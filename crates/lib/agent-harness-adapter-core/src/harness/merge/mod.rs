//! The `merge` kind: the tool owns entries in the user's JSON or TOML file,
//! named by data-driven operations; the file's own style is kept.

mod json;
mod op;
mod remove;
mod toml;

use serde_json::Value;

use json::parse_json;
pub use op::{EntryMatch, MergeOp};
pub(crate) use remove::render_unmerge;
pub(crate) use toml::parse_toml;

use crate::Result;

/// Whether `file` is merged as TOML (by its extension), else JSON.
fn is_toml(file: &str) -> bool {
    file.ends_with(".toml")
}

/// The file to write with `ops` merged into `existing`; `None` when nothing
/// changes. `file` (the part's path) picks the format and names the file in
/// a refusal.
pub(crate) fn render_merge(
    file: &str,
    existing: Option<&str>,
    ops: &[MergeOp],
) -> Result<Option<String>> {
    if is_toml(file) {
        toml::render_toml_merge(existing, ops, file)
    } else {
        json::render_merge(existing, ops, file)
    }
}

/// The tool's entries of each op found in `text`, skipping ops with none.
/// A file that does not parse is a refusal naming `file`.
pub(crate) fn observe_entries(file: &str, text: &str, ops: &[MergeOp]) -> Result<Vec<Value>> {
    if is_toml(file) {
        let doc = parse_toml(file, text)?;
        let mut out = Vec::new();
        for op in ops {
            if let Some(v) = toml::extract_toml(&doc, op)? {
                out.push(v);
            }
        }
        Ok(out)
    } else {
        let doc = parse_json(file, text)?;
        Ok(ops
            .iter()
            .filter_map(|op| json::extract(&doc, op))
            .collect())
    }
}
