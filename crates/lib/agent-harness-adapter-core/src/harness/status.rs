//! `<tool> harness status`: the state of every part of a set of harnesses,
//! from install's run and examination, with nothing written.

use crate::{
    Result,
    fs::read_text,
    harness::{
        InstallResult, PartResult, Scope, Tool,
        install::{Force, is_current_region, placing, run, unusable_warning},
    },
};

/// The state of every part of `harnesses` at `scope`; nothing is written.
// sokf-implements: KIT-5_AC-1
pub fn status<T: Tool + ?Sized, I: IntoIterator<Item = S>, S: AsRef<str>>(
    tool: &T,
    harnesses: I,
    scope: Scope,
) -> Result<InstallResult> {
    let named: Vec<String> = harnesses
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect();
    let mut run = run(tool, &named, scope, &Force::None)?;
    let shared = run.share()?;
    let examined = run.examine(&shared)?;
    // A current region that install would move is current, with a warning.
    // sokf-implements: KIT-26_AC-4
    let mut moves = Vec::new();
    for e in examined.iter().filter(|e| run.members[e.member].named) {
        moves.extend(unusable_warning(&run, e, false));
        let text = if is_current_region(e) {
            read_text(&run.cx.path(&e.path))?
        } else {
            None
        };
        if let Some((_, moved)) = placing(&run.markers, text.as_deref()) {
            let (h, part, path) = (run.members[e.member].harness.id(), e.part.name(), &e.path);
            moves.push(if moved {
                format!("{h}: the tools' blocks in {path} are out of their order; install moves them ({part})")
            } else {
                format!("{h}: the {part} block in {path} opens with the other form of its marker; install rewrites it")
            });
        }
    }
    let parts: Vec<(usize, PartResult)> = examined
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
                    in_place: e.in_place,
                },
            )
        })
        .collect();
    let parts: Vec<_> = parts
        .into_iter()
        .filter(|(i, _)| run.members[*i].named)
        .collect();
    run.warnings.extend(moves);
    Ok(run.result(&parts))
}
