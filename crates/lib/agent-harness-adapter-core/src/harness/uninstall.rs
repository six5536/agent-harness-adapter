//! `uninstall`: a tool's integration taken back out of a set of harnesses
//! (KIT-22), with install's run, states and plan.

use std::collections::BTreeMap;

use crate::{
    Result,
    harness::{
        Action, InstallResult, PartResult, Scope, State, Tool,
        file::render_files,
        install::{Covered, Examined, Force, Run, existing_text, installed, run, unusable_warning},
        merge::render_unmerge,
        part::Kind,
        record::render_record,
        write::{Plan, apply_plan},
    },
    integration::Item,
};

// sokf:begin uninstall-options
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
    /// Keep a part for another installed harness only when that harness's
    /// own part is there, not when it is merely covered by it (`follow`,
    /// whose next install writes the covered harness where it reads).
    pub(crate) own_only: bool,
}
// sokf:end uninstall-options

impl UninstallOptions {
    /// Uninstall `harnesses` at `scope`, without `--force`.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(harnesses: I, scope: Scope) -> Self {
        UninstallOptions {
            harnesses: harnesses.into_iter().map(Into::into).collect(),
            scope,
            force: false,
            own_only: false,
        }
    }

    /// `--force`: also remove the parts the user edited.
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.force = force;
        self
    }
}

/// The installed, unnamed harnesses that read `e`'s item at its location
/// and either have their own part for it there or are shared by it, each
/// with whether its own part is there (it then owns what is kept).
// sokf-implements: KIT-22_AC-4
fn users(
    run: &Run,
    shared: &BTreeMap<(usize, String), Covered>,
    e: &Examined<'_>,
) -> Vec<(String, bool)> {
    let Ok(item) = e.part.name().parse::<Item>() else {
        return Vec::new();
    };
    run.members
        .iter()
        .enumerate()
        .filter(|(_, m)| !m.named)
        .filter(|(_, m)| {
            m.reads
                .get(&item)
                .is_some_and(|r| r.always.iter().chain(&r.maybe).any(|l| *l == e.path))
        })
        .filter_map(|(i, m)| {
            let own = m
                .profile
                .part(e.part.name())
                .is_some_and(|p| p.location() == e.path);
            let covered = shared
                .get(&(i, e.part.name().to_string()))
                .is_some_and(|c| c.by.is_some() && c.location == e.path);
            (own || covered).then(|| (m.harness.id().to_string(), own))
        })
        .collect()
}

/// Plan the removal of one part; `false` when it cannot be removed (an
/// external part its tool cannot remove).
// sokf-implements: KIT-22_AC-1
// sokf-implements: KIT-22_AC-2
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
// sokf-implements: KIT-22_AC-3
// sokf-implements: KIT-22_AC-6
// sokf-implements: KIT-22_AC-7
// sokf-implements: KIT-22_AC-8
// sokf-implements: KIT-22_AC-9
pub fn uninstall<T: Tool + ?Sized>(tool: &T, opts: &UninstallOptions) -> Result<InstallResult> {
    let named = if opts.harnesses.iter().any(|n| n == "all") {
        installed(tool, opts.scope)?
    } else {
        opts.harnesses.clone()
    };
    let mut run = run(tool, &named, opts.scope, &Force::None)?;
    let shared = run.share()?;
    let examined = run.examine(&shared)?;
    let mut plan = Plan {
        prune_root: Some(run.cx.root.clone()),
        ..Plan::default()
    };
    let mut parts = Vec::new();
    let mut warnings = Vec::new();
    // Kept parts: (the named harness, the part, the harnesses that own it).
    let mut kept: Vec<(String, String, Vec<String>)> = Vec::new();
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
        if !wanted {
            warnings.extend(unusable_warning(&run, e, true));
        }
        let users = if wanted {
            users(&run, &shared, e)
                .into_iter()
                .filter(|(_, own)| *own || !opts.own_only)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let mut action = None;
        let mut by = e.by.clone();
        if wanted && !users.is_empty() {
            action = Some(Action::Kept);
            let names: Vec<&str> = users.iter().map(|(u, _)| u.as_str()).collect();
            by = Some(names.join(", "));
            kept.push((
                m.harness.id().to_string(),
                e.part.name().to_string(),
                users
                    .iter()
                    .filter(|(_, owns)| *owns)
                    .map(|(u, _)| u.clone())
                    .collect(),
            ));
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
                in_place: e.in_place,
            },
        ));
    }
    let mut record = run.record.clone();
    // A kept part's value goes, whole, to the harnesses whose own part is
    // there, so a later install finds it stale, not edited.
    // sokf-implements: KIT-22_AC-4
    for (from, part, users) in &kept {
        let Some(hash) = run.record.harnesses.get(from).and_then(|t| t.get(part)) else {
            continue;
        };
        for u in users {
            if let Some(table) = record.harnesses.get_mut(u) {
                table.entry(part.clone()).or_insert_with(|| hash.clone());
            }
        }
    }
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
    // The declined store is the user's: uninstall leaves it as found.
    apply_plan(&plan)?;
    // Install's warnings are about loading twice, which an uninstall does
    // not cause: keep only the uninstall's own.
    run.warnings = warnings;
    Ok(run.result(&parts))
}
