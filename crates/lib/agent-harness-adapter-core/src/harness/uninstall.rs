//! `uninstall`: a tool's integration taken back out of a set of harnesses
//! (KIT-22), with install's run, states and plan.
// @zen-component: KIT-Uninstall

use crate::{
    Result,
    harness::{
        Action, InstallResult, PartResult, Scope, State, Tool,
        file::render_files,
        install::{Examined, Run, existing_text, installed, run},
        merge::render_unmerge,
        part::Kind,
        record::render_record,
        write::{Plan, apply_plan},
    },
    integration::Item,
};

/// The options of [`uninstall`], built with [`UninstallOptions::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UninstallOptions {
    /// The harness ids, e.g. `claude`, or `all` for the installed ones.
    pub harnesses: Vec<String>,
    /// `--scope`.
    pub scope: Scope,
    /// `--force`: also remove the parts the user edited.
    pub force: bool,
}

impl UninstallOptions {
    /// Uninstall `harnesses` at `scope`, without `--force`.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(harnesses: I, scope: Scope) -> Self {
        UninstallOptions {
            harnesses: harnesses.into_iter().map(Into::into).collect(),
            scope,
            force: false,
        }
    }

    /// `--force`: also remove the parts the user edited.
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }
}

/// The installed, unnamed harnesses that read `e`'s item at its location.
// @zen-impl: KIT-22_AC-4
fn users(run: &Run, e: &Examined<'_>) -> Vec<String> {
    let Ok(item) = e.part.name().parse::<Item>() else {
        return Vec::new();
    };
    run.members
        .iter()
        .filter(|m| !m.named)
        .filter(|m| {
            m.reads
                .get(&item)
                .is_some_and(|r| r.always.iter().chain(&r.maybe).any(|l| *l == e.path))
        })
        .map(|m| m.harness.id().to_string())
        .collect()
}

/// Plan the removal of one part; `false` when it cannot be removed (an
/// external part its tool cannot remove).
// @zen-impl: KIT-22_AC-1
// @zen-impl: KIT-22_AC-2
fn plan_removal(run: &Run, plan: &mut Plan, e: &Examined<'_>) -> Result<bool> {
    let fs_path = run.cx.path(&e.path);
    match &e.part.kind {
        Kind::Files { .. } => {
            for (rel, _) in render_files(e.part) {
                let file = fs_path.join(rel);
                if file.exists() || plan.pending(&file).is_some() {
                    plan.delete(file);
                }
            }
        }
        Kind::Region { .. } => {
            let Some(text) = existing_text(plan, &fs_path)? else {
                return Ok(true);
            };
            match run.markers.remove(&text) {
                Some(out) if out.trim().is_empty() => plan.delete(fs_path),
                Some(out) => plan.set(fs_path, out),
                None => {}
            }
        }
        Kind::Merge { ops, .. } => {
            let Some(text) = existing_text(plan, &fs_path)? else {
                return Ok(true);
            };
            match render_unmerge(&e.path, &text, ops)? {
                Some(out) if out.is_empty() => plan.delete(fs_path),
                Some(out) => plan.set(fs_path, out),
                None => {}
            }
        }
        Kind::External(ext) => {
            if !ext.removable() {
                return Ok(false);
            }
            plan.removals.push(ext.clone());
        }
    }
    Ok(true)
}

/// Take the tool's integration out of each named harness: a current or
/// stale part is removed (an edited one only with `force`), unless an
/// installed harness that is not named reads it; a declined, shared or
/// absent part is left. The record loses the named harnesses, and their
/// declined parts are cleared. Every refusal comes before any write.
// @zen-impl: KIT-22_AC-3
// @zen-impl: KIT-22_AC-6
// @zen-impl: KIT-22_AC-7
// @zen-impl: KIT-22_AC-8
// @zen-impl: KIT-22_AC-9
pub fn uninstall<T: Tool + ?Sized>(tool: &T, opts: &UninstallOptions) -> Result<InstallResult> {
    let named = if opts.harnesses.iter().any(|n| n == "all") {
        installed(tool, opts.scope)?
    } else {
        opts.harnesses.clone()
    };
    let mut run = run(tool, &named, opts.scope, None)?;
    let shared = run.share()?;
    let examined = run.examine(&shared)?;
    let mut plan = Plan {
        prune_root: Some(run.cx.root.clone()),
        ..Plan::default()
    };
    let mut parts = Vec::new();
    let mut warnings = Vec::new();
    for e in &examined {
        let m = &run.members[e.member];
        if !m.named {
            continue;
        }
        let wanted = match e.state {
            State::Current | State::Stale => true,
            State::Edited => opts.force,
            _ => false,
        };
        let users = if wanted { users(&run, e) } else { Vec::new() };
        let mut action = None;
        let mut by = e.by.clone();
        if wanted && !users.is_empty() {
            action = Some(Action::Kept);
            by = Some(users.join(", "));
        } else if wanted {
            if plan_removal(&run, &mut plan, e)? {
                action = Some(Action::Removed);
            } else {
                warnings.push(format!(
                    "{}: {} ({}) is left: the tool cannot remove it",
                    m.harness.id(),
                    e.path,
                    e.part.name()
                ));
            }
        }
        parts.push((
            e.member,
            PartResult {
                part: e.part.name().to_string(),
                state: e.state,
                action,
                path: e.path.clone(),
                by,
            },
        ));
    }
    let mut record = run.record.clone();
    for m in run.members.iter().filter(|m| m.named) {
        record.harnesses.remove(m.harness.id());
    }
    if record.harnesses.is_empty() {
        if run.record_path.is_file() {
            plan.record = Some((run.record_path.clone(), None));
        }
    } else if record != run.record {
        let text = render_record(&record, &tool.record_header());
        plan.record = Some((run.record_path.clone(), Some(text)));
    }
    apply_plan(&plan)?;
    let store = tool.declined_store(opts.scope)?;
    for m in run
        .members
        .iter()
        .filter(|m| m.named && !m.declined.is_empty())
    {
        store.set_declined(m.harness.id(), &[])?;
    }
    // Install's warnings are about loading twice, which an uninstall does
    // not cause: keep only the uninstall's own.
    run.warnings = warnings;
    Ok(run.result(parts))
}
