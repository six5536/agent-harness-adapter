//! `<tool> harness install` and `status` over a set of harnesses, generic
//! over a [`Tool`].
// @zen-component: KIT-Harness

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    Error, Result,
    fs::read_text,
    harness::{
        Context, Harness, Markers, Part, Profile, Reads, Scope, State, Tool,
        file::render_files,
        merge::render_merge,
        part::Kind,
        record::{Record, read_record, render_record},
        result::{Action, HarnessResult, InstallResult, PartResult},
        shared::{Candidate, Role, choose},
        state::{Observed, expected, hash, observe, state},
        write::{Plan, apply_plan},
    },
    integration::{Integration, Item},
};

/// The options of [`install`], built with [`InstallOptions::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstallOptions {
    /// The harness ids, e.g. `claude`.
    pub harnesses: Vec<String>,
    /// `--scope`.
    pub scope: Scope,
    /// `--without`, when given at all: replaces the declined parts of every
    /// harness named.
    pub without: Option<Vec<String>>,
    /// `--force`: write a part whose state is `edited`.
    pub force: bool,
}

impl InstallOptions {
    /// Install `harnesses` at `scope`, with the stored declined parts and
    /// without `--force`.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(harnesses: I, scope: Scope) -> Self {
        InstallOptions {
            harnesses: harnesses.into_iter().map(Into::into).collect(),
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

/// One harness of the set: its profile, where it loads each item, and what
/// it declined.
struct Member {
    harness: Arc<dyn Harness>,
    named: bool,
    profile: Profile,
    reads: BTreeMap<Item, Reads>,
    unsupported: Vec<Item>,
    declined: Vec<String>,
}

/// A part's role, state and what the tree held, computed before any write.
struct Examined<'a> {
    member: usize,
    part: &'a Part,
    path: String,
    state: State,
    existed: bool,
    by: Option<String>,
}

/// Everything `install` and `status` read.
struct Run {
    cx: Context,
    markers: Markers,
    record_path: PathBuf,
    record: Record,
    members: Vec<Member>,
    warnings: Vec<String>,
}

/// `path` relative to `root` when it lies under it, `/`-separated.
fn display(root: &Path, path: &Path) -> String {
    let shown = path.strip_prefix(root).unwrap_or(path);
    shown.to_string_lossy().replace('\\', "/")
}

/// The profile `harness` renders for `integration`, then the raw parts.
// @zen-impl: KIT-17_AC-3
// @zen-impl: KIT-17_AC-4
fn profile(
    harness: &dyn Harness,
    integration: &Integration,
    cx: &Context,
) -> Result<(Profile, Vec<Item>)> {
    let mut parts = harness.render(integration, cx)?;
    let mut rendered = Vec::new();
    for p in &parts {
        let item = p.name().parse::<Item>().map_err(|()| {
            Error::Internal(format!(
                "harness `{}` rendered a part named `{}`, not an item",
                harness.id(),
                p.name()
            ))
        })?;
        if rendered.contains(&item) {
            return Err(Error::Internal(format!(
                "harness `{}` rendered `{item}` twice",
                harness.id()
            )));
        }
        rendered.push(item);
    }
    // A raw part may take an item's name when the harness renders nothing
    // for that item, e.g. a tool's own way to install its MCP server there.
    for raw in integration.parts_for(harness.id()) {
        if parts.iter().any(|p| p.name() == raw.name()) {
            return Err(Error::Internal(format!(
                "the part `{}` of `{}` clashes with another part's name",
                raw.name(),
                harness.id()
            )));
        }
        if let Ok(item) = raw.name().parse::<Item>() {
            rendered.push(item);
        }
        parts.push(raw.clone());
    }
    let unsupported = integration
        .items()
        .into_iter()
        .filter(|i| !rendered.contains(i))
        .collect();
    Ok((Profile::new(parts), unsupported))
}

/// Resolve the set, render each profile, read the declined parts.
// @zen-impl: KIT-1_AC-1
// @zen-impl: KIT-1_AC-2
// @zen-impl: KIT-19_AC-1
// @zen-impl: KIT-8_AC-1
fn run<T: Tool + ?Sized>(
    tool: &T,
    named: &[String],
    scope: Scope,
    without: Option<&[String]>,
) -> Result<Run> {
    let supported = tool.harnesses();
    for id in named {
        let h = supported
            .iter()
            .find(|h| h.id() == id)
            .ok_or_else(|| Error::UnknownHarness {
                harness: id.clone(),
            })?;
        if !h.scopes().contains(&scope) {
            return Err(Error::UnsupportedScope {
                harness: id.clone(),
                scope,
            });
        }
    }
    let root = tool.root(scope)?;
    let record_path = tool.record_path(scope)?;
    let record = read_record(&record_path, &display(&root, &record_path))?;
    let user_root = match scope {
        Scope::User => Some(root.clone()),
        _ => tool.root(Scope::User).ok(),
    };
    let cx = Context::new(tool.name(), scope, root, user_root);
    let integration = tool.integration(scope);
    let store = tool.declined_store(scope)?;
    let mut members = Vec::new();
    for h in supported {
        let is_named = named.iter().any(|n| n == h.id());
        let recorded = record.harnesses.contains_key(h.id());
        if !(is_named || recorded) || !h.scopes().contains(&scope) {
            continue;
        }
        let (profile, unsupported) = profile(h.as_ref(), &integration, &cx)?;
        // Read even when `--without` replaces it: a store that cannot be
        // read refuses here, before any write.
        let stored = store.declined(h.id())?;
        let declined = match without {
            Some(w) if is_named => w
                .iter()
                .filter(|p| profile.part(p).is_some())
                .cloned()
                .collect(),
            _ => stored,
        };
        let mut reads = BTreeMap::new();
        for p in profile.parts() {
            if let Ok(item) = p.name().parse::<Item>() {
                reads.insert(item, h.reads(item, &cx)?);
            }
        }
        members.push(Member {
            harness: h,
            named: is_named,
            profile,
            reads,
            unsupported,
            declined,
        });
    }
    if let Some(part) = without.into_iter().flatten().find(|p| {
        !members
            .iter()
            .any(|m| m.named && m.profile.part(p).is_some())
    }) {
        return Err(Error::UnknownPart {
            harness: named.join(", "),
            part: part.clone(),
        });
    }
    Ok(Run {
        markers: Markers::new(tool.name()),
        cx,
        record_path,
        record,
        members,
        warnings: Vec::new(),
    })
}

impl Run {
    /// The recorded hash of `member`'s part `name`; for an item, else that
    /// of another harness whose part for it has the same location.
    fn recorded(&self, member: usize, part: &Part) -> Option<&str> {
        let own = |m: &Member| {
            self.record
                .harnesses
                .get(m.harness.id())
                .and_then(|t| t.get(part.name()))
                .map(String::as_str)
        };
        own(&self.members[member]).or_else(|| {
            part.name().parse::<Item>().ok()?;
            self.members.iter().find_map(|m| {
                let p = m.profile.part(part.name())?;
                (p.location() == part.location()).then(|| own(m)).flatten()
            })
        })
    }

    /// The shared parts: per item, the choice of KIT-Shared.
    // @zen-impl: KIT-19_AC-6
    fn share(&mut self) -> Result<BTreeMap<(usize, String), (String, String)>> {
        let mut shared = BTreeMap::new();
        for item in Item::ALL {
            let mut idx = Vec::new();
            let mut cands = Vec::new();
            for (i, m) in self.members.iter().enumerate() {
                let Some(part) = m.profile.part(item.as_str()) else {
                    continue;
                };
                if m.declined.iter().any(|d| d == item.as_str()) {
                    continue;
                }
                idx.push(i);
                cands.push(Candidate {
                    harness: m.harness.id(),
                    named: m.named,
                    part,
                    reads: &m.reads[&item],
                });
            }
            let choice = choose(item.as_str(), &cands)?;
            self.warnings.extend(choice.warnings);
            for (k, role) in choice.roles.into_iter().enumerate() {
                if let Role::Shared { location, by } = role {
                    let m = &self.members[idx[k]];
                    let own = cands[k].part.location();
                    let has_hash = self
                        .record
                        .harnesses
                        .get(m.harness.id())
                        .is_some_and(|t| t.contains_key(item.as_str()));
                    if has_hash && own != location {
                        self.warnings.push(format!(
                            "{}: the {item} now come from {location} ({by}); the earlier copy in {own} is left in place",
                            m.harness.id()
                        ));
                    }
                    shared.insert((idx[k], item.as_str().to_string()), (location, by));
                }
            }
        }
        Ok(shared)
    }

    /// Examine every part of every member: declined, shared, or its state.
    // @zen-impl: KIT-8_AC-2
    fn examine(
        &self,
        shared: &BTreeMap<(usize, String), (String, String)>,
    ) -> Result<Vec<Examined<'_>>> {
        let mut out = Vec::new();
        for (i, m) in self.members.iter().enumerate() {
            for part in m.profile.parts() {
                let path = part.location();
                // A declined part is never read: a broken file it would merge
                // into must not block the rest.
                if m.declined.iter().any(|d| d == part.name()) {
                    out.push(Examined {
                        member: i,
                        part,
                        path,
                        state: State::Skipped,
                        existed: false,
                        by: None,
                    });
                    continue;
                }
                if let Some((location, by)) = shared.get(&(i, part.name().to_string())) {
                    out.push(Examined {
                        member: i,
                        part,
                        path: location.clone(),
                        state: State::Shared,
                        existed: false,
                        by: Some(by.clone()),
                    });
                    continue;
                }
                let observed = observe(&self.cx.root, part, &path, &self.markers)?;
                let st = state(&observed, &expected(part), self.recorded(i, part), false);
                let present = matches!(observed, Observed::Present(_));
                // A file part existed when any of its files did (its directory
                // may hold others'); a region or merge when its file did.
                let existed = present
                    || match &part.kind {
                        Kind::Files { files, .. } => files
                            .iter()
                            .any(|(rel, _)| self.cx.path(&path).join(rel).exists()),
                        Kind::Region { .. } | Kind::Merge { .. } => self.cx.path(&path).exists(),
                        Kind::External(_) => false,
                    };
                out.push(Examined {
                    member: i,
                    part,
                    path,
                    state: st,
                    existed,
                    by: None,
                });
            }
        }
        Ok(out)
    }

    /// The result of the named members, from their parts' results.
    fn result(&self, parts: Vec<(usize, PartResult)>) -> InstallResult {
        let harnesses = self
            .members
            .iter()
            .enumerate()
            .filter(|(_, m)| m.named)
            .map(|(i, m)| {
                let parts: Vec<PartResult> = parts
                    .iter()
                    .filter(|(j, _)| *j == i)
                    .map(|(_, p)| p.clone())
                    .collect();
                HarnessResult {
                    harness: m.harness.id().to_string(),
                    notes: m.harness.notes(&self.cx, &parts),
                    parts,
                    unsupported: m.unsupported.clone(),
                }
            })
            .collect();
        InstallResult {
            scope: self.cx.scope,
            root: self.cx.root.clone(),
            harnesses,
            warnings: self.warnings.clone(),
        }
    }
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
fn plan_part(cx: &Context, markers: &Markers, plan: &mut Plan, e: &Examined<'_>) -> Result<()> {
    let fs_path = cx.path(&e.path);
    match &e.part.kind {
        Kind::Files { .. } => {
            for (rel, text) in render_files(e.part) {
                plan.set(fs_path.join(rel), text);
            }
        }
        Kind::Region { block, .. } => {
            let existing = existing_text(plan, &fs_path)?;
            if let Some(text) = markers.render(existing.as_deref(), block) {
                plan.set(fs_path, text);
            }
        }
        Kind::Merge { ops, .. } => {
            let existing = existing_text(plan, &fs_path)?;
            if let Some(text) = render_merge(&e.path, existing.as_deref(), ops)? {
                plan.set(fs_path, text);
            }
        }
        Kind::External(ext) => plan.externals.push(ext.clone()),
    }
    Ok(())
}

/// Make the tree match each named harness's profile: a declined part is
/// skipped, a shared part left to its writer, an absent part created, a
/// stale part rewritten, an edited part left unless `force`, a current part
/// left. Every refusal comes before any write.
// @zen-impl: KIT-4_AC-1
// @zen-impl: KIT-4_AC-2
// @zen-impl: KIT-6_AC-1
pub fn install<T: Tool + ?Sized>(tool: &T, opts: &InstallOptions) -> Result<InstallResult> {
    let mut run = run(tool, &opts.harnesses, opts.scope, opts.without.as_deref())?;
    let shared = run.share()?;
    let examined = run.examine(&shared)?;
    let mut plan = Plan::default();
    let mut record = run.record.clone();
    let mut parts = Vec::new();
    for e in &examined {
        let m = &run.members[e.member];
        if !m.named {
            continue;
        }
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
            plan_part(&run.cx, &run.markers, &mut plan, e)?;
        }
        let table = record
            .harnesses
            .entry(m.harness.id().to_string())
            .or_default();
        match e.state {
            State::Skipped => {
                table.remove(e.part.name());
            }
            // A shared part keeps any hash; an edited part left keeps its own.
            State::Shared => {}
            State::Edited if !written => {}
            _ => {
                table.insert(e.part.name().to_string(), hash(&expected(e.part)));
            }
        }
        parts.push((
            e.member,
            PartResult {
                part: e.part.name().to_string(),
                state: e.state,
                action,
                path: e.path.clone(),
                by: e.by.clone(),
            },
        ));
    }
    if record != run.record || !run.record_path.is_file() {
        plan.record = Some((
            run.record_path.clone(),
            render_record(&record, &tool.record_header()),
        ));
    }
    apply_plan(&plan)?;
    if opts.without.is_some() {
        let store = tool.declined_store(opts.scope)?;
        for m in run.members.iter().filter(|m| m.named) {
            store.set_declined(m.harness.id(), &m.declined)?;
        }
    }
    Ok(run.result(parts))
}

/// The ids of the harnesses installed at `scope` (those in the record), in
/// the tool's order; nothing is written.
// @zen-impl: KIT-5_AC-2
pub fn installed<T: Tool + ?Sized>(tool: &T, scope: Scope) -> Result<Vec<String>> {
    let root = tool.root(scope)?;
    let path = tool.record_path(scope)?;
    let record = read_record(&path, &display(&root, &path))?;
    Ok(tool
        .harnesses()
        .iter()
        .map(|h| h.id().to_string())
        .filter(|id| record.harnesses.contains_key(id))
        .collect())
}

/// The state of every part of `harnesses` at `scope`; nothing is written.
// @zen-impl: KIT-5_AC-1
pub fn status<T: Tool + ?Sized, I: IntoIterator<Item = S>, S: AsRef<str>>(
    tool: &T,
    harnesses: I,
    scope: Scope,
) -> Result<InstallResult> {
    let named: Vec<String> = harnesses
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect();
    let mut run = run(tool, &named, scope, None)?;
    let shared = run.share()?;
    let parts: Vec<(usize, PartResult)> = run
        .examine(&shared)?
        .into_iter()
        .map(|e| {
            (
                e.member,
                PartResult {
                    part: e.part.name().to_string(),
                    state: e.state,
                    action: None,
                    path: e.path,
                    by: e.by,
                },
            )
        })
        .collect();
    let parts = parts
        .into_iter()
        .filter(|(i, _)| run.members[*i].named)
        .collect();
    Ok(run.result(parts))
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

    #[test]
    fn options_build() {
        let o = InstallOptions::new(["claude", "codex"], Scope::User)
            .without(["mcp"])
            .force(true);
        assert_eq!(o.harnesses, ["claude", "codex"]);
        assert_eq!(o.scope, Scope::User);
        assert_eq!(o.without, Some(vec!["mcp".to_string()]));
        assert!(o.force);
        assert_eq!(InstallOptions::new(["c"], Scope::Project).without, None);
    }
}
