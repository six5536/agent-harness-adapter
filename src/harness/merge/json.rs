//! The JSON merge: the tool's entries applied to the user's JSON file,
//! named by data-driven operations. Key order, the file's indent and its
//! trailing newline are kept.
// @zen-component: KIT-Merge

use serde::Serialize;
use serde_json::{Map, Value, ser::PrettyFormatter};

use crate::{
    Error, Result,
    harness::merge::{EntryMatch, MergeOp, op::Op},
};

/// Parse a target file. A file that does not parse, or is not an object, is
/// a refusal naming `display`.
pub(crate) fn parse_json(display: &str, text: &str) -> Result<Value> {
    let doc: Value = serde_json::from_str(text)
        .map_err(|e| Error::file(display, format!("does not parse as JSON: {e}")))?;
    if !doc.is_object() {
        return Err(Error::file(display, "is not a JSON object"));
    }
    Ok(doc)
}

/// The indent of a file: the leading whitespace of its first indented line,
/// or two spaces.
fn indent_of(text: &str) -> String {
    text.lines()
        .find(|l| l.starts_with(' ') || l.starts_with('\t'))
        .map(|l| l[..l.len() - l.trim_start().len()].to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "  ".to_string())
}

/// The value at `path` in `doc`, when every step is an object member.
fn get<'a>(doc: &'a Value, path: &[String]) -> Option<&'a Value> {
    path.iter().try_fold(doc, |v, k| v.as_object()?.get(k))
}

/// Whether an entry is the tool's: its `field` matches.
fn is_tool_entry(entry: &Value, field: &str, owned: &EntryMatch) -> bool {
    entry[field].as_str().is_some_and(|c| owned.matches(c))
}

/// A group's keys other than its entries: what tells groups apart (e.g. a
/// `matcher`).
fn other_keys(group: &Value, entries: &str) -> Map<String, Value> {
    group
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| *k != entries)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The tool's part of `op` as found in `doc`, in the form of
/// [`MergeOp::value`]; `None` when none of it is there.
pub(crate) fn extract(doc: &Value, op: &MergeOp) -> Option<Value> {
    match &op.0 {
        Op::ArrayEntry { path, value } => get(doc, path)?
            .as_array()?
            .iter()
            .find(|v| *v == value)
            .cloned(),
        Op::ObjectMember { path, key, .. } => get(doc, path)?.as_object()?.get(key).cloned(),
        Op::OwnedEntries {
            path, field, owned, ..
        } => {
            let ours: Vec<Value> = get(doc, path)?
                .as_array()?
                .iter()
                .filter(|e| is_tool_entry(e, field, owned))
                .cloned()
                .collect();
            (!ours.is_empty()).then_some(Value::Array(ours))
        }
        Op::GroupEntries {
            path,
            entries,
            field,
            owned,
            ..
        } => {
            // Only the tool's entries, grouped as found: the user's entries
            // and keys beside them are theirs, not an edit of the tool's.
            let mut groups = Vec::new();
            for g in get(doc, path)?.as_array()? {
                let ours: Vec<Value> = g[entries.as_str()]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| is_tool_entry(e, field, owned))
                    .cloned()
                    .collect();
                if !ours.is_empty() {
                    let mut out = other_keys(g, entries);
                    out.insert(entries.clone(), Value::Array(ours));
                    groups.push(Value::Object(out));
                }
            }
            (!groups.is_empty()).then_some(Value::Array(groups))
        }
    }
}

/// The object at `path` in `doc`, created along the way when absent.
fn object_at<'a>(
    doc: &'a mut Value,
    path: &[String],
    display: &str,
) -> Result<&'a mut Map<String, Value>> {
    let mut cur = doc;
    for (i, key) in path.iter().enumerate() {
        let map = cur
            .as_object_mut()
            .ok_or_else(|| not_object(display, &path[..i]))?;
        cur = map
            .entry(key.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    cur.as_object_mut().ok_or_else(|| not_object(display, path))
}

fn not_object(display: &str, path: &[String]) -> Error {
    Error::file(display, format!("`{}` is not an object", path.join(".")))
}

/// The array at `path` (non-empty) in `doc`, created along the way when
/// absent.
fn array_at<'a>(doc: &'a mut Value, path: &[String], display: &str) -> Result<&'a mut Vec<Value>> {
    let (last, parents) = path
        .split_last()
        .ok_or_else(|| Error::Internal("an array entry needs a path".into()))?;
    object_at(doc, parents, display)?
        .entry(last.clone())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| Error::file(display, format!("`{}` is not an array", path.join("."))))
}

/// Set the tool's part of `op` in `doc`. Containers are created when
/// absent; a container of another type is a refusal naming `display`.
// @zen-impl: KIT-10_AC-2
// @zen-impl: KIT-10_AC-3
// @zen-impl: KIT-10_AC-4
fn apply(doc: &mut Value, op: &MergeOp, display: &str) -> Result<()> {
    // An op that only removes the tool's old entries creates nothing.
    // @zen-impl: KIT-10_AC-6
    if op.expects_nothing() && get(doc, op.path()).is_none() {
        return Ok(());
    }
    match &op.0 {
        Op::ArrayEntry { path, value } => {
            let array = array_at(doc, path, display)?;
            if !array.contains(value) {
                array.push(value.clone());
            }
        }
        Op::ObjectMember { path, key, value } => {
            object_at(doc, path, display)?.insert(key.clone(), value.clone());
        }
        Op::OwnedEntries {
            path,
            field,
            owned,
            entries,
        } => {
            let array = array_at(doc, path, display)?;
            // The tool's entries go where its first one was.
            let at = array
                .iter()
                .position(|e| is_tool_entry(e, field, owned))
                .unwrap_or(array.len());
            array.retain(|e| !is_tool_entry(e, field, owned));
            let ours = entries.as_array().cloned().unwrap_or_default();
            array.splice(at..at, ours);
        }
        Op::GroupEntries {
            path,
            entries,
            field,
            owned,
            groups: wanted,
        } => {
            let groups = array_at(doc, path, display)?;
            // Where each group held the tool's first entry, before removing
            // the tool's entries everywhere.
            let mut held: Vec<Option<usize>> = Vec::with_capacity(groups.len());
            for g in groups.iter_mut() {
                let es = g[entries.as_str()].as_array_mut();
                let at = es
                    .as_ref()
                    .and_then(|es| es.iter().position(|e| is_tool_entry(e, field, owned)));
                if let (Some(es), Some(_)) = (es, at) {
                    es.retain(|e| !is_tool_entry(e, field, owned));
                }
                held.push(at);
            }
            let mut used = vec![false; groups.len()];
            let mut appended = Vec::new();
            for w in wanted.as_array().into_iter().flatten() {
                let keys = other_keys(w, entries);
                let ours = w[entries.as_str()].as_array().cloned().unwrap_or_default();
                let slot = (0..held.len()).find(|&i| {
                    held[i].is_some() && !used[i] && other_keys(&groups[i], entries) == keys
                });
                match slot {
                    Some(i) => {
                        used[i] = true;
                        let at = held[i].expect("held");
                        if let Some(es) = groups[i][entries.as_str()].as_array_mut() {
                            es.splice(at..at, ours);
                        }
                    }
                    None => appended.push(w.clone()),
                }
            }
            // A group that held only the tool's entries and got none back goes.
            let mut i = 0;
            groups.retain(|g| {
                let keep = !(held[i].is_some()
                    && !used[i]
                    && g[entries.as_str()].as_array().is_some_and(Vec::is_empty));
                i += 1;
                keep
            });
            groups.extend(appended);
        }
    }
    Ok(())
}

/// Serialise with an indent and, when asked, a trailing newline.
fn json_text(doc: &Value, indent: &str, trailing_newline: bool) -> String {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        PrettyFormatter::with_indent(indent.as_bytes()),
    );
    doc.serialize(&mut ser).expect("a JSON value serialises");
    let mut text = String::from_utf8(buf).expect("serde_json writes UTF-8");
    if trailing_newline {
        text.push('\n');
    }
    text
}

/// The file to write with `ops` merged into `existing`, re-serialised in its
/// style: its indent (two spaces for a new file) and its trailing newline
/// (one for a new file). `None` when nothing changes. `display` names the
/// file in a refusal.
// @zen-impl: KIT-10_AC-1
pub(crate) fn render_merge(
    existing: Option<&str>,
    ops: &[MergeOp],
    display: &str,
) -> Result<Option<String>> {
    let mut doc = match existing {
        Some(text) => parse_json(display, text)?,
        None => Value::Object(Map::new()),
    };
    for op in ops {
        apply(&mut doc, op, display)?;
    }
    let indent = existing.map_or_else(|| "  ".to_string(), indent_of);
    let newline = existing.is_none_or(|t| t.ends_with('\n'));
    let out = json_text(&doc, &indent, newline);
    Ok((Some(out.as_str()) != existing).then_some(out))
}

#[cfg(test)]
mod tests;
