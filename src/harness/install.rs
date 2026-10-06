//! `<tool> harness install` and `status`, generic over a [`Tool`].
// @zen-component: KIT-Harness

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::{
    Error, Result,
    fs::read_text,
    harness::{
        Markers, Profile, Scope, State, Tool,
        file::render_files,
        merge::render_merge,
        part::{Kind, Part},
        record::{Record, read_record, render_record},
        state::{Observed, expected, hash, observe, state},
        target::target_path,
        write::{Plan, apply_plan},
    },
};

/// The options of [`install`], built with [`InstallOptions::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstallOptions {
    /// The harness (profile) name, e.g. `claude`.
    pub harness: String,
    /// `--scope`.
    pub scope: Scope,
    /// `--without`, when given at all: replaces the declined parts.
    pub without: Option<Vec<String>>,
    /// `--force`: write a part whose state is `edited`.
    pub force: bool,
}

impl InstallOptions {
    /// Install `harness` at `scope`, with the stored declined parts and
    /// without `--force`.
    pub fn new(harness: impl Into<String>, scope: Scope) -> Self {
        InstallOptions {
            harness: harness.into(),
            scope,
            without: None,
            force: false,
        }
    }

    /// `--without`: decline these parts, replacing the stored list.
    #[must_use]
    pub fn without<I: IntoIterator<Item = S>, S: Into<String>>(mut self, parts: I) -> Self {
        self.without = Some(parts.into_iter().map(Into::into).collect());
        self
    }

    /// `--force`: also write the parts the user edited.
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }
}

// @zen-component: KIT-Results
/// What [`install`] did to a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Action {
    /// Written where nothing was.
    Created,
    /// A `file` part's files written again.
    Rewrote,
    /// A region or merge part written into the existing file, or an
    /// external part written again.
    Updated,
}

impl Action {
    /// The action's word in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Created => "created",
            Action::Rewrote => "rewrote",
            Action::Updated => "updated",
        }
    }
}

/// One line of the report.
// @zen-impl: KIT-4_AC-3
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PartResult {
    /// The part's name.
    pub part: String,
    /// The part's state before any write.
    pub state: State,
    /// What `install` did; `None` when the part was left as found, and
    /// always for `status`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// The part's path relative to the root, `/`-separated; an external
    /// part's location.
    pub path: String,
}

impl PartResult {
    /// The report word: the action, else the state.
    pub fn verb(&self) -> &'static str {
        self.action
            .map_or_else(|| self.state.as_str(), Action::as_str)
    }
}

/// The outcome of [`install`] or [`status`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct HarnessResult {
    /// The harness name.
    pub harness: String,
    /// The scope.
    pub scope: Scope,
    /// The root directory the paths are relative to.
    pub root: PathBuf,
    /// One entry per part, in profile order.
    pub parts: Vec<PartResult>,
}

impl HarnessResult {
    /// The text report: one line per part, `<verb> <path> (<part>)`, the
    /// verb padded to seven columns.
    // @zen-impl: KIT-4_AC-4
    pub fn to_text(&self) -> String {
        self.parts
            .iter()
            .map(|p| format!("{:<7} {} ({})\n", p.verb(), p.path, p.part))
            .collect()
    }
}

/// A part's state and what the tree held, computed before any write.
struct Examined<'a> {
    part: &'a Part,
    path: String,
    state: State,
    existed: bool,
}

/// Everything `install` and `status` read.
struct Context {
    profile: Profile,
    root: PathBuf,
    markers: Markers,
    record_path: PathBuf,
    record: Record,
}

/// `path` relative to `root` when it lies under it, `/`-separated.
fn display(root: &Path, path: &Path) -> String {
    let shown = path.strip_prefix(root).unwrap_or(path);
    shown.to_string_lossy().replace('\\', "/")
}

// @zen-impl: KIT-1_AC-1
// @zen-impl: KIT-1_AC-2
fn context<T: Tool + ?Sized>(tool: &T, harness: &str, scope: Scope) -> Result<Context> {
    let profile = tool
        .profile(harness, scope)
        .ok_or_else(|| Error::UnknownProfile {
            harness: harness.to_string(),
        })?;
    let root = tool.root(scope)?;
    let record_path = tool.record_path(scope)?;
    let record = read_record(&record_path, &display(&root, &record_path))?;
    Ok(Context {
        profile,
        markers: Markers::new(tool.name()),
        root,
        record_path,
        record,
    })
}

/// Examine every part: its target, what the tree holds, its state.
// @zen-impl: KIT-8_AC-2
fn examine<'a>(cx: &'a Context, declined: &[String]) -> Result<Vec<Examined<'a>>> {
    let recorded = cx.record.harnesses.get(cx.profile.harness());
    let mut out = Vec::new();
    for part in cx.profile.parts() {
        let path = target_path(&cx.root, part)?;
        // A declined part is never read: a broken file it would merge into
        // must not block the rest.
        if declined.iter().any(|d| d == part.name()) {
            out.push(Examined {
                part,
                path,
                state: State::Skipped,
                existed: false,
            });
            continue;
        }
        let observed = observe(&cx.root, part, &path, &cx.markers)?;
        let hash = recorded
            .and_then(|m| m.get(part.name()))
            .map(String::as_str);
        let state = state(&observed, &expected(part), hash, false);
        let present = matches!(observed, Observed::Present(_));
        let existed =
            present || (!matches!(part.kind, Kind::External(_)) && cx.root.join(&path).exists());
        out.push(Examined {
            part,
            path,
            state,
            existed,
        });
    }
    Ok(out)
}

/// The existing text of `path`: planned already, or on disk.
fn existing_text(plan: &Plan, path: &Path) -> Result<Option<String>> {
    if let Some(text) = plan.pending(path) {
        return Ok(Some(text.to_string()));
    }
    read_text(path)
}

/// Plan the write of one part, from the pending or the on-disk text.
// @zen-impl: KIT-7_AC-1
fn plan_part(cx: &Context, plan: &mut Plan, e: &Examined<'_>) -> Result<()> {
    let fs_path = cx.root.join(&e.path);
    match &e.part.kind {
        Kind::Files { .. } => {
            for (rel, text) in render_files(e.part) {
                plan.set(fs_path.join(rel), text);
            }
        }
        Kind::Region { block, .. } => {
            let existing = existing_text(plan, &fs_path)?;
            if let Some(text) = cx.markers.render(existing.as_deref(), block) {
                plan.set(fs_path, text);
            }
        }
        Kind::Merge { ops, .. } => {
            let existing = existing_text(plan, &fs_path)?;
            if let Some(text) = render_merge(existing.as_deref(), ops, &e.path)? {
                plan.set(fs_path, text);
            }
        }
        Kind::External(ext) => plan.externals.push(ext.clone()),
    }
    Ok(())
}

/// Make the tree match the profile: a declined part is skipped, an absent
/// part created, a stale part rewritten, an edited part left unless
/// `force`, a current part left. Every refusal comes before any write.
// @zen-impl: KIT-4_AC-1
// @zen-impl: KIT-4_AC-2
// @zen-impl: KIT-6_AC-1
// @zen-impl: KIT-8_AC-1
pub fn install<T: Tool + ?Sized>(tool: &T, opts: &InstallOptions) -> Result<HarnessResult> {
    let cx = context(tool, &opts.harness, opts.scope)?;
    if let Some(part) = opts
        .without
        .iter()
        .flatten()
        .find(|p| cx.profile.part(p).is_none())
    {
        return Err(Error::UnknownPart {
            harness: cx.profile.harness().to_string(),
            part: part.clone(),
        });
    }
    let store = tool.declined_store(opts.scope)?;
    // Read even when `--without` replaces it: a store that cannot be read
    // refuses here, before any write, not when it is set afterwards.
    let stored = store.declined(cx.profile.harness())?;
    let declined = opts.without.clone().unwrap_or(stored);
    let examined = examine(&cx, &declined)?;

    let mut plan = Plan::default();
    let mut recorded = cx
        .record
        .harnesses
        .get(cx.profile.harness())
        .cloned()
        .unwrap_or_default();
    let mut parts = Vec::new();
    for e in &examined {
        let written = match e.state {
            State::Absent | State::Stale => true,
            State::Edited => opts.force,
            _ => false,
        };
        let action = if !written {
            None
        } else if !e.existed {
            Some(Action::Created)
        } else if matches!(e.part.kind, Kind::Files { .. }) {
            Some(Action::Rewrote)
        } else {
            Some(Action::Updated)
        };
        if written {
            plan_part(&cx, &mut plan, e)?;
        }
        match e.state {
            State::Skipped => {
                recorded.remove(e.part.name());
            }
            State::Edited if !written => {}
            _ => {
                recorded.insert(e.part.name().to_string(), hash(&expected(e.part)));
            }
        }
        parts.push(PartResult {
            part: e.part.name().to_string(),
            state: e.state,
            action,
            path: e.path.clone(),
        });
    }
    let mut new_record = cx.record.clone();
    new_record
        .harnesses
        .insert(cx.profile.harness().to_string(), recorded);
    if new_record != cx.record || !cx.record_path.is_file() {
        plan.record = Some((
            cx.record_path.clone(),
            render_record(&new_record, &tool.record_header()),
        ));
    }
    apply_plan(&plan)?;
    if let Some(without) = &opts.without {
        store.set_declined(cx.profile.harness(), without)?;
    }
    Ok(HarnessResult {
        harness: cx.profile.harness().to_string(),
        scope: opts.scope,
        root: cx.root.clone(),
        parts,
    })
}

/// The state of every part; nothing is written.
// @zen-impl: KIT-5_AC-1
pub fn status<T: Tool + ?Sized>(tool: &T, harness: &str, scope: Scope) -> Result<HarnessResult> {
    let cx = context(tool, harness, scope)?;
    let declined = tool.declined_store(scope)?.declined(cx.profile.harness())?;
    let parts = examine(&cx, &declined)?
        .into_iter()
        .map(|e| PartResult {
            part: e.part.name().to_string(),
            state: e.state,
            action: None,
            path: e.path,
        })
        .collect();
    Ok(HarnessResult {
        harness: cx.profile.harness().to_string(),
        scope,
        root: cx.root.clone(),
        parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_relative_under_the_root() {
        assert_eq!(
            display(Path::new("/r"), Path::new("/r/.tool/h.toml")),
            ".tool/h.toml"
        );
        assert_eq!(
            display(Path::new("/r"), Path::new("/x/h.toml")),
            "/x/h.toml"
        );
    }

    // @zen-test: KIT-4_AC-3
    // @zen-test: KIT-4_AC-4
    #[test]
    fn text_pads_the_verb_and_json_carries_state_and_action() {
        let r = HarnessResult {
            harness: "claude".into(),
            scope: Scope::Project,
            root: "/r".into(),
            parts: vec![
                PartResult {
                    part: "hooks".into(),
                    state: State::Absent,
                    action: Some(Action::Created),
                    path: ".claude/settings.json".into(),
                },
                PartResult {
                    part: "instructions".into(),
                    state: State::Edited,
                    action: None,
                    path: "CLAUDE.md".into(),
                },
            ],
        };
        assert_eq!(
            r.to_text(),
            "created .claude/settings.json (hooks)\nedited  CLAUDE.md (instructions)\n"
        );
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            serde_json::json!({
                "harness": "claude",
                "scope": "project",
                "root": "/r",
                "parts": [
                    {"part": "hooks", "state": "absent", "action": "created", "path": ".claude/settings.json"},
                    {"part": "instructions", "state": "edited", "path": "CLAUDE.md"}
                ]
            })
        );
    }

    #[test]
    fn options_build() {
        let o = InstallOptions::new("claude", Scope::User)
            .without(["mcp"])
            .force(true);
        assert_eq!(o.harness, "claude");
        assert_eq!(o.scope, Scope::User);
        assert_eq!(o.without, Some(vec!["mcp".to_string()]));
        assert!(o.force);
        assert_eq!(InstallOptions::new("c", Scope::Project).without, None);
    }
}
