//! `<tool> harness install` and `status` over a set of harnesses, generic
//! over a [`Tool`].

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    Error, Result,
    fs::{read_text, shown},
    harness::{
        Context, Harness, Markers, Part, Profile, Reads, Scope, State, Tool,
        file::render_files,
        meaning::Unusable,
        merge::render_merge,
        part::Kind,
        record::{Record, Recorded, read_record, render_record},
        result::{Action, HarnessResult, InstallResult, MissingSource, PartResult},
        shared::{Candidate, Role, choose},
        state::{Observed, expected, hash, observe, state},
        write::{Plan, apply_plan},
    },
    integration::{Hook, Integration, Item},
};

// sokf:begin install-options
/// The parts `install` writes whatever their state (`--force`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Force {
    /// No part: an edited part is left as found.
    #[default]
    None,
    /// Every part that is not declined (`--force all`).
    All,
    /// The parts named (`--force <part>`).
    Parts(Vec<String>),
}

impl Force {
    /// Whether the part `name` is forced.
    pub fn covers(&self, name: &str) -> bool {
        match self {
            Force::None => false,
            Force::All => true,
            Force::Parts(parts) => parts.iter().any(|p| p == name),
        }
    }
}

/// The options of [`install`], built with [`InstallOptions::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstallOptions {
    /// The harness ids, e.g. `claude`.
    pub harnesses: Vec<String>,
    /// `--scope`.
    pub scope: Scope,
    /// `--force`: the parts written whatever their state.
    pub force: Force,
}

impl InstallOptions {
    /// Install `harnesses` at `scope`, with the stored declined parts and
    /// no forced part.
    pub fn new<I: IntoIterator<Item = S>, S: Into<String>>(harnesses: I, scope: Scope) -> Self {
        InstallOptions {
            harnesses: harnesses.into_iter().map(Into::into).collect(),
            scope,
            force: Force::None,
        }
    }

    /// `--force` alone: every part that is not declined, or none.
    #[must_use]
    pub fn force(mut self, force: bool) -> Self {
        self.force = if force { Force::All } else { Force::None };
        self
    }

    /// `--force <part>`: these parts; `all` among them forces every part
    /// that is not declined, and no part forces none.
    #[must_use]
    pub fn force_parts<I: IntoIterator<Item = S>, S: Into<String>>(mut self, parts: I) -> Self {
        let parts: Vec<String> = parts.into_iter().map(Into::into).collect();
        self.force = if parts.iter().any(|p| p == "all") {
            Force::All
        } else if parts.is_empty() {
            Force::None
        } else {
            Force::Parts(parts)
        };
        self
    }
}
// sokf:end install-options

/// One harness of the set: its profile, where it loads each item, and what
/// it declined.
pub(super) struct Member {
    pub(super) harness: Arc<dyn Harness>,
    pub(super) named: bool,
    pub(super) profile: Profile,
    pub(super) reads: BTreeMap<Item, Reads>,
    pub(super) unsupported: Vec<Item>,
    pub(super) declined: Vec<String>,
    /// The hook events of the integration the harness does not run.
    pub(super) missed: Vec<crate::hook::Event>,
}

/// A part's role, state and what the tree held, computed before any write.
pub(super) struct Examined<'a> {
    pub(super) member: usize,
    pub(super) part: &'a Part,
    pub(super) path: String,
    pub(super) state: State,
    pub(super) existed: bool,
    pub(super) by: Option<String>,
    pub(super) in_place: bool,
    /// Why the record could not vouch for an edited part.
    pub(super) unusable: Option<Unusable>,
}

/// Where a shared part's content is: `location`, written by `by`, or the
/// item's SOURCE (`by` none: IN PLACE).
#[derive(Debug, Clone)]
pub(super) struct Covered {
    pub(super) location: String,
    pub(super) by: Option<String>,
}

/// Everything `install` and `status` read.
pub(super) struct Run {
    pub(super) cx: Context,
    pub(super) markers: Markers,
    pub(super) record_path: PathBuf,
    pub(super) record: Record,
    pub(super) members: Vec<Member>,
    pub(super) warnings: Vec<String>,
    /// Per item read from a SOURCE under the root, its location.
    pub(super) sources: BTreeMap<Item, String>,
    pub(super) missing: Vec<MissingSource>,
}

/// The profile `harness` renders for `integration`, then the raw parts.
// sokf-implements: KIT-17_AC-3
// sokf-implements: KIT-17_AC-4
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
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the set, whose steps share the run's state"
)]
// sokf-implements: KIT-1_AC-1
// sokf-implements: KIT-1_AC-2
// sokf-implements: KIT-19_AC-1
// sokf-implements: KIT-8_AC-1
// sokf-implements: KIT-6_AC-1
pub(super) fn run<T: Tool + ?Sized>(
    tool: &T,
    named: &[String],
    scope: Scope,
    force: &Force,
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
    let record = read_record(&record_path, &shown(&root, &record_path))?;
    let user_root = match scope {
        Scope::User => Some(root.clone()),
        _ => tool.root(Scope::User).ok(),
    };
    let cx = Context::new(tool.name(), scope, root, user_root);
    let declared = tool.integration(scope);
    let integration = declared.resolve(&cx);
    let sources: BTreeMap<Item, String> = Item::ALL
        .into_iter()
        .filter_map(|i| Some((i, cx.relative(&declared.source(i)?.path)?)))
        .collect();
    let missing = declared
        .missing()
        .iter()
        .map(|m| MissingSource {
            item: m.item,
            path: cx
                .relative(&m.path)
                .unwrap_or_else(|| m.path.display().to_string()),
        })
        .collect();
    let store = tool.declined_store(scope)?;
    let set: Vec<(&Arc<dyn Harness>, bool)> = supported
        .iter()
        .filter(|h| h.scopes().contains(&scope))
        .map(|h| (h, named.iter().any(|n| n == h.id())))
        .filter(|(h, is_named)| *is_named || record.harnesses.contains_key(h.id()))
        .collect();
    // A first render gives where each harness puts the parts it does not
    // decline; each then renders again seeing where the others do, so a
    // choice made from the files present sees a file the same run writes.
    // The first render's locations are all a harness sees: none sees where
    // another chose from them.
    // sokf-implements: KIT-19_AC-9
    let mut first: Vec<Vec<String>> = Vec::with_capacity(set.len());
    for (h, _) in &set {
        // A store that cannot be read refuses here, before any write.
        let declined = store.declined(h.id())?;
        first.push(
            profile(h.as_ref(), &integration, &cx)?
                .0
                .parts()
                .iter()
                .filter(|p| !declined.iter().any(|d| d == p.name()))
                .map(Part::location)
                .collect(),
        );
    }
    let mut members = Vec::new();
    for (i, (h, is_named)) in set.into_iter().enumerate() {
        let mut cx = cx.clone();
        cx.rendered = first
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .flat_map(|(_, l)| l.iter().cloned())
            .collect();
        let (profile, unsupported) = profile(h.as_ref(), &integration, &cx)?;
        let mut missed: Vec<crate::hook::Event> = Vec::new();
        if profile.part(Item::Hooks.as_str()).is_some() {
            for e in integration.hooks().iter().map(Hook::event) {
                if !h.hook_events().contains(&e) && !missed.contains(&e) {
                    missed.push(e);
                }
            }
        }
        let declined: Vec<String> = store
            .declined(h.id())?
            .into_iter()
            .filter(|p| profile.part(p).is_some())
            .collect();
        let mut reads = BTreeMap::new();
        for p in profile.parts() {
            if let Ok(item) = p.name().parse::<Item>() {
                reads.insert(item, h.reads(item, &cx)?);
            }
        }
        members.push(Member {
            harness: Arc::clone(h),
            named: is_named,
            profile,
            reads,
            unsupported,
            declined,
            missed,
        });
    }
    if let Force::Parts(parts) = force {
        for part in parts {
            if !members
                .iter()
                .any(|m| m.named && m.profile.part(part).is_some())
            {
                return Err(Error::UnknownPart {
                    harness: named.join(", "),
                    part: part.clone(),
                });
            }
            if let Some(m) = members
                .iter()
                .find(|m| m.named && m.declined.contains(part))
            {
                return Err(Error::DeclinedPart {
                    harness: m.harness.id().to_string(),
                    part: part.clone(),
                    store: store.location(),
                });
            }
        }
    }
    Ok(Run {
        markers: Markers::new(tool.name()).order(declared.block_order_value()),
        cx,
        record_path,
        record,
        members,
        warnings: integration.warnings().to_vec(),
        sources,
        missing,
    })
}

impl Run {
    /// The recorded value of `member`'s part `name`; for an item, else that
    /// of another harness whose part for it has the same location.
    fn recorded(&self, member: usize, part: &Part) -> Option<&Recorded> {
        let own = |m: &Member| {
            self.record
                .harnesses
                .get(m.harness.id())
                .and_then(|t| t.get(part.name()))
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
    // sokf-implements: KIT-19_AC-6
    pub(super) fn share(&mut self) -> Result<BTreeMap<(usize, String), Covered>> {
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
            let choice = choose(
                item.as_str(),
                &cands,
                self.sources.get(&item).map(String::as_str),
            )?;
            self.warnings.extend(choice.warnings);
            for (k, role) in choice.roles.into_iter().enumerate() {
                if let Role::InPlace { location } = role {
                    shared.insert(
                        (idx[k], item.as_str().to_string()),
                        Covered { location, by: None },
                    );
                } else if let Role::Shared { location, by } = role {
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
                    shared.insert(
                        (idx[k], item.as_str().to_string()),
                        Covered {
                            location,
                            by: Some(by),
                        },
                    );
                }
            }
        }
        Ok(shared)
    }

    /// Examine every part of every member: declined, shared, or its state.
    // sokf-implements: KIT-8_AC-2
    pub(super) fn examine(
        &self,
        shared: &BTreeMap<(usize, String), Covered>,
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
                        in_place: false,
                        unusable: None,
                    });
                    continue;
                }
                if let Some(c) = shared.get(&(i, part.name().to_string())) {
                    out.push(Examined {
                        member: i,
                        part,
                        path: c.location.clone(),
                        state: State::Shared,
                        existed: false,
                        by: c.by.clone(),
                        in_place: c.by.is_none(),
                        unusable: None,
                    });
                    continue;
                }
                // A copy of a SOURCE differs from its recorded hash once the
                // SOURCE changes: stale.
                // sokf-implements: KIT-23_AC-4
                let observed = observe(&self.cx.root, part, &path, &self.markers)?;
                let (st, unusable) =
                    state(&observed, &expected(part), self.recorded(i, part), false);
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
                    in_place: false,
                    unusable,
                });
            }
        }
        Ok(out)
    }

    /// The result of the named members, from their parts' results.
    pub(super) fn result(&self, parts: &[(usize, PartResult)]) -> InstallResult {
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
                    notes: missed_note(m)
                        .into_iter()
                        .chain(m.harness.notes(&self.cx, &parts))
                        .collect(),
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
            missing: self.missing.clone(),
        }
    }
}

/// The note for hooks whose event the harness does not run.
// sokf-implements: KIT-20_AC-2
fn missed_note(m: &Member) -> Option<String> {
    if m.missed.is_empty() {
        return None;
    }
    let events: Vec<&str> = m.missed.iter().map(|e| e.as_str()).collect();
    Some(format!(
        "{} has no {} event: those hooks do not run there",
        m.harness.id(),
        events.join(", ")
    ))
}

/// The existing text of `path`: planned already (`None` for a planned
/// deletion), or on disk.
pub(super) fn existing_text(plan: &Plan, path: &Path) -> Result<Option<String>> {
    if let Some(planned) = plan.pending(path) {
        return Ok(planned.map(str::to_string));
    }
    read_text(path)
}

/// Plan the write of one part, from the pending or the on-disk text.
// sokf-implements: KIT-7_AC-1
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

/// Whether `e` is a region found current, the parts KIT-26_AC-4 places; an
/// edited region stays where it is.
// sokf-implements: KIT-26_AC-5
pub(super) fn is_current_region(e: &Examined<'_>) -> bool {
    e.state == State::Current && matches!(e.part.kind, Kind::Region { .. })
}

/// For a current region ([`is_current_region`]) in the file's `text` that
/// install puts in place (KIT-26_AC-4): the file's new text, and whether
/// the blocks move (else only the opening marker changes). Install and
/// status share it.
// sokf-implements: KIT-26_AC-4
pub(super) fn placing(markers: &Markers, text: Option<&str>) -> Option<(String, bool)> {
    let text = text?;
    let placed = markers.place(text)?;
    Some((placed, markers.out_of_order(text)))
}

/// Make the tree match each named harness's profile: a declined part is
/// skipped, a shared part left to its writer, an absent part created, a
/// stale part rewritten, an edited part left unless `force`, a current part
/// left. Every refusal comes before any write.
// sokf-implements: KIT-4_AC-1
// sokf-implements: KIT-4_AC-2
// sokf-implements: KIT-4_AC-5
pub fn install<T: Tool + ?Sized>(tool: &T, opts: &InstallOptions) -> Result<InstallResult> {
    let mut run = run(tool, &opts.harnesses, opts.scope, &opts.force)?;
    let shared = run.share()?;
    let examined = run.examine(&shared)?;
    let mut plan = Plan::default();
    let mut record = run.record.clone();
    // Every named harness is installed, even one with nothing to write yet
    // (e.g. its sources are all missing), so later runs know it.
    for m in run.members.iter().filter(|m| m.named) {
        record
            .harnesses
            .entry(m.harness.id().to_string())
            .or_default();
    }
    let mut parts = Vec::new();
    let mut warnings = Vec::new();
    for e in &examined {
        let m = &run.members[e.member];
        if !m.named {
            continue;
        }
        let written = match e.state {
            State::Absent | State::Stale => true,
            State::Edited => opts.force.covers(e.part.name()),
            _ => false,
        };
        if !written {
            warnings.extend(unusable_warning(&run, e, false));
        }
        // A current region in the wrong place, or under the other form of
        // its opening marker, moves with its lines as found.
        // sokf-implements: KIT-26_AC-4
        let mut placed = false;
        if !written && is_current_region(e) {
            let fs_path = run.cx.path(&e.path);
            let existing = existing_text(&plan, &fs_path)?;
            if let Some((text, _)) = placing(&run.markers, existing.as_deref()) {
                plan.set(fs_path, text);
                placed = true;
            }
        }
        let action = if placed {
            Some(Action::Updated)
        } else if !written {
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
            // A declined part keeps its hash, so that it is stale, not edited,
            // when the decline ends; a shared part keeps any hash; an edited
            // part left keeps its own.
            State::Skipped | State::Shared => {}
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
                in_place: e.in_place,
            },
        ));
    }
    if record != run.record || !run.record_path.is_file() {
        plan.record = Some((
            run.record_path.clone(),
            Some(render_record(&record, &tool.record_header())),
        ));
    }
    apply_plan(&plan)?;
    run.warnings.extend(warnings);
    Ok(run.result(&parts))
}

/// The warning for an edited part whose record could not vouch for it
/// (KIT-3_AC-10).
pub(super) fn unusable_warning(run: &Run, e: &Examined<'_>, uninstall: bool) -> Option<String> {
    let why = e.unusable.as_ref()?;
    let harness = run.members[e.member].harness.id();
    Some(why.warning(harness, e.part.name(), &e.path, uninstall))
}

/// The harness ids `names` stands for at `scope`: `all` is every harness
/// of the tool with files at the scope, in the tool's order; other names
/// are kept as given.
// sokf-implements: KIT-1_AC-3
pub fn expand<T: Tool + ?Sized, S: AsRef<str>>(tool: &T, names: &[S], scope: Scope) -> Vec<String> {
    if names.iter().any(|n| n.as_ref() == "all") {
        return tool
            .harnesses()
            .iter()
            .filter(|h| h.scopes().contains(&scope))
            .map(|h| h.id().to_string())
            .collect();
    }
    names.iter().map(|n| n.as_ref().to_string()).collect()
}

/// The ids of the harnesses installed at `scope` (those in the record), in
/// the tool's order; nothing is written.
// sokf-implements: KIT-5_AC-2
pub fn installed<T: Tool + ?Sized>(tool: &T, scope: Scope) -> Result<Vec<String>> {
    let root = tool.root(scope)?;
    let path = tool.record_path(scope)?;
    let record = read_record(&path, &shown(&root, &path))?;
    Ok(tool
        .harnesses()
        .iter()
        .map(|h| h.id().to_string())
        .filter(|id| record.harnesses.contains_key(id))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_build() {
        let o = InstallOptions::new(["claude", "codex"], Scope::User).force(true);
        assert_eq!(o.harnesses, ["claude", "codex"]);
        assert_eq!(o.scope, Scope::User);
        assert_eq!(o.force, Force::All);
        assert_eq!(o.clone().force(false).force, Force::None);
        let p = o.clone().force_parts(["skills", "hooks"]);
        assert_eq!(p.force, Force::Parts(vec!["skills".into(), "hooks".into()]));
        assert!(p.force.covers("hooks") && !p.force.covers("mcp"));
        assert_eq!(o.clone().force_parts(["skills", "all"]).force, Force::All);
        assert_eq!(o.force_parts(Vec::<String>::new()).force, Force::None);
        assert_eq!(
            InstallOptions::new(["c"], Scope::Project).force,
            Force::None
        );
    }
}
