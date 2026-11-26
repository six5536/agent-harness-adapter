//! `keep_instructions`: a tool's text written as the project's own after
//! its block, in each file of an installed harness's instructions.

use std::{collections::BTreeMap, fmt::Write as _, path::PathBuf, sync::Arc};

use crate::{
    Error, Result, fs,
    harness::{
        DeclinedStore, Force, Harness, Markers, Scope, Tool,
        install::{self, Covered, Member},
        part::Kind,
        write,
    },
    integration::{Integration, Item},
};

/// What [`keep_instructions`] did, as a report the tool prints as it
/// stands ([`Kept::to_text`]); it names the files only for the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Kept {
    /// Each file written, relative to the root, in harness order.
    files: Vec<String>,
}

impl Kept {
    /// The report: one `kept    <file>` line per file written, or
    /// `kept    nothing new` when the text stood in each already.
    // sokf-implements: KIT-27_AC-5
    pub fn to_text(&self) -> String {
        if self.files.is_empty() {
            return "kept    nothing new\n".to_string();
        }
        self.files.iter().fold(String::new(), |mut out, f| {
            let _ = writeln!(out, "kept    {f}");
            out
        })
    }

    /// Whether the call wrote a file.
    pub fn wrote(&self) -> bool {
        !self.files.is_empty()
    }
}

/// `tool` with its instructions alone ([`Integration::instructions_only`]):
/// with an empty block when it has none now (e.g. its last pack left), so
/// each harness still renders the part that tells where its block is
/// (`empty`), and with its raw `instructions` parts (`raw`).
struct Instructions<'a, T: ?Sized> {
    tool: &'a T,
    empty: bool,
    raw: bool,
}

impl<T: Tool + ?Sized> Tool for Instructions<'_, T> {
    fn name(&self) -> &str {
        self.tool.name()
    }

    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        self.tool.harnesses()
    }

    fn integration(&self, scope: Scope) -> Integration {
        self.tool
            .integration(scope)
            .instructions_only(self.empty, self.raw)
    }

    fn root(&self, scope: Scope) -> Result<PathBuf> {
        self.tool.root(scope)
    }

    fn record_path(&self, scope: Scope) -> Result<PathBuf> {
        self.tool.record_path(scope)
    }

    fn declined_store(&self, scope: Scope) -> Result<Box<dyn DeclinedStore + '_>> {
        self.tool.declined_store(scope)
    }

    fn record_header(&self) -> String {
        self.tool.record_header()
    }
}

/// The file of `m`'s instructions, the member `i` of its run: its region's
/// own location, the location that covers a shared one, or the SOURCE that
/// an IN PLACE part loads or a REFERENCE points at (`true`: the text goes
/// at its end); `None` for a declined part, or one of another kind.
fn instructions_file(
    m: &Member,
    i: usize,
    shared: &BTreeMap<(usize, String), Covered>,
    source: Option<&str>,
) -> Option<(String, bool)> {
    let name = Item::Instructions.as_str();
    let part = m.profile.part(name)?;
    if m.declined.iter().any(|d| d == name) {
        return None;
    }
    if part.is_reference() {
        return source.map(|s| (s.to_string(), true));
    }
    match shared.get(&(i, name.to_string())) {
        Some(c) => Some((c.location.clone(), c.by.is_none())),
        None if matches!(part.kind, Kind::Region { .. }) => Some((part.location(), false)),
        None => None,
    }
}

/// Write `text` as plain text, the project's own, right after the tool's
/// block in each file that holds the block and the instructions of a
/// harness installed at `scope`, and at the end of the SOURCE a harness
/// loads in place, once each; `AGENTS.md` at project scope when no file
/// holds the block. A file that already holds the text is not written. The
/// record and the declined store are left as found.
///
/// # Errors
///
/// As [`status`](crate::harness::status) refuses while it renders, before
/// any write; [`Error::MarkedText`] for a text holding a block's marker;
/// [`Error::NoBlock`] when no file holds the block outside project scope.
// sokf-implements: KIT-27_AC-1
// sokf-implements: KIT-27_AC-2
// sokf-implements: KIT-27_AC-3
// sokf-implements: KIT-27_AC-5
// sokf-implements: KIT-27_AC-6
// sokf-implements: KIT-27_AC-7
pub fn keep_instructions<T: Tool + ?Sized>(tool: &T, text: &str, scope: Scope) -> Result<Kept> {
    keep(tool, text, scope, true, None).map(|k| k.unwrap_or(Kept { files: Vec::new() }))
}

/// [`keep_instructions`] only beside the tool's block in the files of the
/// harnesses `only`: `None` when no such file holds it, so nothing is
/// written to a file of no block, or of a harness that leaves (`follow`,
/// which would then count that file as showing use).
pub(crate) fn keep_beside<T: Tool + ?Sized>(
    tool: &T,
    text: &str,
    scope: Scope,
    only: &[String],
) -> Result<Option<Kept>> {
    keep(tool, text, scope, false, Some(only))
}

/// Keep `text`; with `fallback`, in `AGENTS.md` at project scope (or a
/// refusal at another) when no file holds the block, else `None` there.
#[expect(
    clippy::too_many_lines,
    reason = "one plan of every file, whose steps share the files found and the refusals"
)]
fn keep<T: Tool + ?Sized>(
    tool: &T,
    text: &str,
    scope: Scope,
    fallback: bool,
    only: Option<&[String]>,
) -> Result<Option<Kept>> {
    if let Some(line) = Markers::marker_line(text) {
        return Err(Error::MarkedText { line });
    }
    if text.trim().is_empty() {
        return Ok(Some(Kept { files: Vec::new() }));
    }
    let named: Vec<String> = install::installed(tool, scope)?
        .into_iter()
        .filter(|id| {
            tool.harnesses()
                .iter()
                .any(|h| h.id() == id && h.scopes().contains(&scope))
        })
        .collect();
    // A harness with a raw `instructions` part renders through it; when the
    // integration has no instructions now, every other harness renders an
    // empty block, so each says where its block is.
    let integration = tool.integration(scope);
    let has_item = integration.items().contains(&Item::Instructions);
    let raw_ids: Vec<String> = tool
        .harnesses()
        .iter()
        .map(|h| h.id().to_string())
        .filter(|id| {
            integration
                .parts_for(id)
                .any(|p| p.name() == Item::Instructions.as_str())
        })
        .collect();
    // Whether each run renders the raw parts; the other renders an empty
    // block.
    let runs = if has_item || raw_ids.is_empty() {
        vec![has_item]
    } else {
        vec![true, false]
    };
    let order: Vec<String> = tool
        .harnesses()
        .iter()
        .map(|h| h.id().to_string())
        .collect();
    let mut found: Vec<(usize, String, bool)> = Vec::new();
    let mut cx = None;
    let mut source = None;
    for raw in runs {
        let mut run = install::run(
            &Instructions {
                tool,
                empty: !raw,
                raw,
            },
            &named,
            scope,
            &Force::None,
        )?;
        let shared = run.share()?;
        source = source.or_else(|| run.sources.get(&Item::Instructions).cloned());
        for (i, m) in run.members.iter().enumerate() {
            // With no instructions now, the two runs split the harnesses:
            // with a raw part, or not.
            if !has_item && raw_ids.iter().any(|id| id == m.harness.id()) != raw {
                continue;
            }
            if only.is_some_and(|o| !o.iter().any(|id| id == m.harness.id())) {
                continue;
            }
            if let Some((path, in_place)) = instructions_file(m, i, &shared, source.as_deref()) {
                let at = order
                    .iter()
                    .position(|id| id == m.harness.id())
                    .unwrap_or(usize::MAX);
                found.push((at, path, in_place));
            }
        }
        cx.get_or_insert(run.cx);
    }
    found.sort_by_key(|f| f.0);
    let mut candidates: Vec<(String, bool)> = Vec::new();
    for (_, path, in_place) in found {
        if !candidates.iter().any(|(p, _)| *p == path) {
            candidates.push((path, in_place));
        }
    }
    // Instructions read from a SOURCE: every harness reads it, by itself, by
    // a REFERENCE, or through a copy that install refreshes from it, so the
    // text goes there alone; after a copy's block it would come back inside
    // the block at the next install.
    if let Some(src) = &source {
        candidates = vec![(src.clone(), true)];
    }
    let cx = cx.expect("a run");
    let markers = Markers::new(tool.name()).order(integration.block_order_value());
    let mut holding: Vec<(String, Option<String>)> = Vec::new();
    for (path, in_place) in candidates {
        let found = fs::read_text(&cx.path(&path))?;
        if in_place || found.as_deref().is_some_and(|t| markers.find(t).is_some()) {
            holding.push((path, found));
        }
    }
    if holding.is_empty() {
        if !fallback {
            return Ok(None);
        }
        if scope != Scope::Project {
            return Err(Error::NoBlock {
                tool: tool.name().to_string(),
                scope,
            });
        }
        let found = fs::read_text(&cx.path("AGENTS.md"))?;
        holding.push(("AGENTS.md".to_string(), found));
    }
    let mut plan = write::Plan::default();
    let mut files = Vec::new();
    for (path, found) in holding {
        if let Some(out) = markers.keep(&path, found.as_deref(), text) {
            plan.set(cx.path(&path), out);
            files.push(path);
        }
    }
    write::apply_plan(&plan)?;
    Ok(Some(Kept { files }))
}
