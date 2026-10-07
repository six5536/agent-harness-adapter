//! The inverse of a merge (KIT-22_AC-1): the tool's entries taken back out
//! of a JSON or TOML file, then every container on an operation's path the
//! removal left empty. The rest of the file, its order and its style stay.

use serde_json::Value;
use toml_edit::Item;

use super::{
    is_toml,
    json::{indent_of, is_tool_entry, json_text, parse_json},
    op::{MergeOp, Op},
    parse_toml,
    toml::members,
};
use crate::Result;

/// The value at `path` in `doc`, mutable, when every step is an object
/// member.
fn get_mut<'a>(doc: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    path.iter()
        .try_fold(doc, |v, k| v.as_object_mut()?.get_mut(k))
}

/// Whether `v` is an empty array or object.
fn is_empty_container(v: &Value) -> bool {
    match v {
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// Remove the tool's part of `op` from `doc`.
fn remove_json(doc: &mut Value, op: &MergeOp) {
    let Some(target) = get_mut(doc, op.path()) else {
        return;
    };
    match &op.0 {
        Op::ArrayEntry { value, .. } => {
            if let Some(a) = target.as_array_mut() {
                a.retain(|v| v != value);
            }
        }
        Op::ObjectMember { key, .. } => {
            if let Some(o) = target.as_object_mut() {
                o.shift_remove(key);
            }
        }
        Op::OwnedEntries { field, owned, .. } => {
            if let Some(a) = target.as_array_mut() {
                a.retain(|e| !is_tool_entry(e, field, owned));
            }
        }
        Op::GroupEntries {
            entries,
            field,
            owned,
            ..
        } => {
            if let Some(groups) = target.as_array_mut() {
                // A group that held the tool's entries and is left with none
                // goes, as on install.
                groups.retain_mut(|g| {
                    let Some(es) = g[entries.as_str()].as_array_mut() else {
                        return true;
                    };
                    let before = es.len();
                    es.retain(|e| !is_tool_entry(e, field, owned));
                    !(es.len() < before && es.is_empty())
                });
            }
        }
    }
    prune_json(doc, op.path());
}

/// Remove each container on `path` the removal left empty, deepest first.
fn prune_json(doc: &mut Value, path: &[String]) {
    for depth in (1..=path.len()).rev() {
        let Some(parent) = get_mut(doc, &path[..depth - 1]).and_then(Value::as_object_mut) else {
            return;
        };
        if parent.get(&path[depth - 1]).is_some_and(is_empty_container) {
            parent.shift_remove(&path[depth - 1]);
        } else {
            return;
        }
    }
}

/// `existing` with `ops` taken out, in its style. `None` when nothing
/// changes; an empty string when the file is left empty (an empty object).
fn render_json_unmerge(existing: &str, ops: &[MergeOp], display: &str) -> Result<Option<String>> {
    let mut doc = parse_json(display, existing)?;
    let before = doc.clone();
    for op in ops {
        remove_json(&mut doc, op);
    }
    if doc == before {
        return Ok(None);
    }
    if is_empty_container(&doc) {
        return Ok(Some(String::new()));
    }
    let out = json_text(&doc, &indent_of(existing), existing.ends_with('\n'));
    Ok((out != existing).then_some(out))
}

/// `existing` with `ops`' members taken out, comments and order kept.
/// `None` when nothing changes; an empty string when nothing is left but
/// whitespace.
fn render_toml_unmerge(existing: &str, ops: &[MergeOp], display: &str) -> Result<Option<String>> {
    let mut doc = parse_toml(display, existing)?;
    for op in ops {
        let (path, key, _) = members(op)?;
        remove_toml(doc.as_table_mut(), path, key);
    }
    let out = doc.to_string();
    if out == existing {
        return Ok(None);
    }
    Ok(Some(if out.trim().is_empty() {
        String::new()
    } else {
        out
    }))
}

/// Remove `key` from the table at `path` under `table`, then the tables on
/// the path it left empty.
fn remove_toml(table: &mut toml_edit::Table, path: &[String], key: &str) {
    let Some((first, rest)) = path.split_first() else {
        table.remove(key);
        return;
    };
    let Some(Item::Table(child)) = table.get_mut(first) else {
        return;
    };
    remove_toml(child, rest, key);
    if child.is_empty() {
        table.remove(first);
    }
}

/// The file with `ops` taken out of `existing`: `None` when nothing
/// changes, an empty string when the file is left empty. `file` picks the
/// format and names the file in a refusal.
// @zen-impl: KIT-22_AC-1
pub(crate) fn render_unmerge(
    file: &str,
    existing: &str,
    ops: &[MergeOp],
) -> Result<Option<String>> {
    if is_toml(file) {
        render_toml_unmerge(existing, ops, file)
    } else {
        render_json_unmerge(existing, ops, file)
    }
}

#[cfg(test)]
mod tests;
