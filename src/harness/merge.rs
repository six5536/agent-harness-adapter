//! The `merge` kind: the tool owns entries in the user's JSON file, named by
//! data-driven operations. Key order, the file's indent and its trailing
//! newline are kept.
// @zen-component: KIT-Merge

use serde::Serialize;
use serde_json::{Map, Value, ser::PrettyFormatter};

use crate::{Error, Result};

/// One entry the tool owns in a JSON file, built by
/// [`MergeOp::array_entry`], [`MergeOp::object_member`] or
/// [`MergeOp::group_entry`].
// @zen-impl: KIT-2_AC-3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOp(Op);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    ArrayEntry {
        path: Vec<String>,
        value: Value,
    },
    ObjectMember {
        path: Vec<String>,
        key: String,
        value: Value,
    },
    GroupEntry {
        path: Vec<String>,
        entries: String,
        field: String,
        prefix: String,
        group: Value,
    },
}

fn segments<P: IntoIterator<Item = S>, S: Into<String>>(path: P) -> Vec<String> {
    path.into_iter().map(Into::into).collect()
}

impl MergeOp {
    /// `value` is present once in the array at `path` (keys from the
    /// top-level object, e.g. `["permissions", "allow"]`). Found by
    /// equality.
    pub fn array_entry<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        value: impl Into<Value>,
    ) -> Self {
        MergeOp(Op::ArrayEntry {
            path: segments(path),
            value: value.into(),
        })
    }

    /// The member `key` of the object at `path` (empty for the top-level
    /// object) is `value`, e.g. `tool` under `["mcpServers"]`. Found by its
    /// key.
    pub fn object_member<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Self {
        MergeOp(Op::ObjectMember {
            path: segments(path),
            key: key.into(),
            value: value.into(),
        })
    }

    /// The tool's entries in the groups of the array at `path`. Each group is
    /// an object holding its entries as an array under `entries`; an entry is
    /// the tool's when its member `field` is a string starting with `prefix`.
    /// The tool owns those entries, not the group they sit in: they replace
    /// the tool's entries in the first group that has one (the user's entries
    /// and the group's other keys stay), or `group` (an object with the
    /// tool's entries under `entries`) is appended as a new group.
    pub fn group_entry<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        entries: impl Into<String>,
        field: impl Into<String>,
        prefix: impl Into<String>,
        group: impl Into<Value>,
    ) -> Self {
        MergeOp(Op::GroupEntry {
            path: segments(path),
            entries: entries.into(),
            field: field.into(),
            prefix: prefix.into(),
            group: group.into(),
        })
    }

    /// The value the operation writes.
    pub fn value(&self) -> &Value {
        match &self.0 {
            Op::ArrayEntry { value, .. } | Op::ObjectMember { value, .. } => value,
            Op::GroupEntry { group, .. } => group,
        }
    }
}

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

/// Parse TOML, keeping its formatting; `display` names the file in the
/// refusal.
pub(crate) fn parse_toml(display: &str, text: &str) -> Result<toml_edit::DocumentMut> {
    text.parse()
        .map_err(|e: toml_edit::TomlError| Error::file(display, e.message()))
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

/// Whether an entry is the tool's: its `field` starts with the prefix.
fn is_tool_entry(entry: &Value, field: &str, prefix: &str) -> bool {
    entry[field].as_str().is_some_and(|c| c.starts_with(prefix))
}

/// Whether a group holds one of the tool's entries.
fn has_tool_entry(group: &Value, entries: &str, field: &str, prefix: &str) -> bool {
    group[entries]
        .as_array()
        .is_some_and(|es| es.iter().any(|e| is_tool_entry(e, field, prefix)))
}

/// The tool's entry of `op` as found in `doc`; `None` when it is not there.
pub(crate) fn extract(doc: &Value, op: &MergeOp) -> Option<Value> {
    match &op.0 {
        Op::ArrayEntry { path, value } => get(doc, path)?
            .as_array()?
            .iter()
            .find(|v| *v == value)
            .cloned(),
        Op::ObjectMember { path, key, .. } => get(doc, path)?.as_object()?.get(key).cloned(),
        Op::GroupEntry {
            path,
            entries,
            field,
            prefix,
            ..
        } => {
            // Only the tool's entries: the user's entries and keys beside
            // them are theirs, not an edit of the tool's entry.
            let group = get(doc, path)?
                .as_array()?
                .iter()
                .find(|g| has_tool_entry(g, entries, field, prefix))?;
            let ours: Vec<Value> = group[entries.as_str()]
                .as_array()?
                .iter()
                .filter(|e| is_tool_entry(e, field, prefix))
                .cloned()
                .collect();
            let mut out = Map::new();
            out.insert(entries.clone(), Value::Array(ours));
            Some(Value::Object(out))
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

/// Set the tool's entry of `op` in `doc`. Containers are created when
/// absent; a container of another type is a refusal naming `display`.
// @zen-impl: KIT-10_AC-2
// @zen-impl: KIT-10_AC-3
fn apply(doc: &mut Value, op: &MergeOp, display: &str) -> Result<()> {
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
        Op::GroupEntry {
            path,
            entries,
            field,
            prefix,
            group,
        } => {
            let groups = array_at(doc, path, display)?;
            match groups
                .iter_mut()
                .find(|g| has_tool_entry(g, entries, field, prefix))
                .and_then(|g| g[entries.as_str()].as_array_mut())
            {
                Some(es) => {
                    // The tool's entries go where its first one was.
                    let at = es
                        .iter()
                        .position(|e| is_tool_entry(e, field, prefix))
                        .unwrap_or(es.len());
                    es.retain(|e| !is_tool_entry(e, field, prefix));
                    let ours = group[entries.as_str()]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    es.splice(at..at, ours);
                }
                None => groups.push(group.clone()),
            }
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
mod tests {
    use proptest::prelude::*;
    use serde_json::json;

    use super::*;

    const P: &str = ".claude/settings.json";
    const PREFIX: &str = "tool harness hook ";

    /// A Claude Code style hook group op, built here so the core's tests do
    /// not depend on the `claude` module.
    fn hook(event: &str, command: &str) -> MergeOp {
        MergeOp::group_entry(
            ["hooks", event],
            "hooks",
            "command",
            PREFIX,
            json!({ "hooks": [{ "type": "command", "command": command }] }),
        )
    }

    fn permissions() -> Vec<MergeOp> {
        vec![MergeOp::array_entry(
            ["permissions", "allow"],
            "Bash(tool *)",
        )]
    }

    fn hooks() -> Vec<MergeOp> {
        vec![hook("Stop", "tool harness hook claude stop")]
    }

    fn mcp() -> Vec<MergeOp> {
        vec![MergeOp::object_member(
            ["mcpServers"],
            "tool",
            json!({ "command": "tool", "args": ["mcp"] }),
        )]
    }

    /// Remove the tool's entry of `op` from `doc`, leaving the containers; a
    /// group left with no entries goes. Test-only: it states what "the rest"
    /// of a document is.
    fn remove(doc: &mut Value, op: &MergeOp) {
        fn get_mut<'a>(doc: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
            path.iter()
                .try_fold(doc, |v, k| v.as_object_mut()?.get_mut(k))
        }
        match &op.0 {
            Op::ArrayEntry { path, value } => {
                if let Some(array) = get_mut(doc, path).and_then(Value::as_array_mut) {
                    array.retain(|v| v != value);
                }
            }
            Op::ObjectMember { path, key, .. } => {
                if let Some(m) = get_mut(doc, path).and_then(Value::as_object_mut) {
                    m.shift_remove(key);
                }
            }
            Op::GroupEntry {
                path,
                entries,
                field,
                prefix,
                ..
            } => {
                let Some(groups) = get_mut(doc, path).and_then(Value::as_array_mut) else {
                    return;
                };
                for g in groups.iter_mut() {
                    if let Some(es) = g[entries.as_str()].as_array_mut() {
                        es.retain(|e| !is_tool_entry(e, field, prefix));
                    }
                }
                groups.retain(|g| g[entries.as_str()].as_array().is_none_or(|e| !e.is_empty()));
            }
        }
    }

    #[test]
    fn indent_is_detected_or_two_spaces() {
        assert_eq!(indent_of("{\n    \"a\": 1\n}"), "    ");
        assert_eq!(indent_of("{\n\t\"a\": 1\n}"), "\t");
        assert_eq!(indent_of("{}"), "  ");
        assert_eq!(indent_of(""), "  ");
    }

    // @zen-test: KIT-2_AC-3
    #[test]
    fn permissions_are_added_once_and_extracted() {
        let out = render_merge(None, &permissions(), P).unwrap().unwrap();
        assert_eq!(
            out,
            "{\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(tool *)\"\n    ]\n  }\n}\n"
        );
        let doc = parse_json(P, &out).unwrap();
        assert_eq!(
            extract(&doc, &permissions()[0]),
            Some(json!("Bash(tool *)"))
        );
        let existing = "{\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(npm *)\",\n      \"Bash(tool *)\"\n    ]\n  }\n}\n";
        assert_eq!(
            render_merge(Some(existing), &permissions(), P).unwrap(),
            None
        );
        let doc = parse_json(P, "{\"permissions\":{\"allow\":[\"Bash(npm *)\"]}}").unwrap();
        assert_eq!(extract(&doc, &permissions()[0]), None);
        assert_eq!(extract(&json!({"permissions": 1}), &permissions()[0]), None);
    }

    // A path segment may hold a dot: it is one key.
    #[test]
    fn a_key_may_contain_a_dot() {
        let op = MergeOp::object_member(["enabledPlugins"], "x.y@market", true);
        let out = render_merge(Some("{\"x\": {\"y\": 1}}"), std::slice::from_ref(&op), P)
            .unwrap()
            .unwrap();
        let doc = parse_json(P, &out).unwrap();
        assert_eq!(doc["enabledPlugins"]["x.y@market"], true);
        assert_eq!(doc["x"], json!({"y": 1}));
        let op = MergeOp::array_entry(["a.b"], 1);
        let doc = parse_json(
            P,
            &render_merge(None, std::slice::from_ref(&op), P)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(doc["a.b"], json!([1]));
        assert_eq!(extract(&doc, &op), Some(json!(1)));
    }

    // The tool owns its entries, not the group: a user's entry and a key
    // beside it are kept, and do not make the entry "edited".
    // @zen-test: KIT-10_AC-3
    #[test]
    fn a_shared_group_keeps_the_users_entries() {
        let shared = json!({"hooks": {"Stop": [{
            "matcher": "*",
            "hooks": [
                { "type": "command", "command": "tool harness hook claude old" },
                { "type": "command", "command": "~/bin/notify.sh" }
            ]
        }]}});
        let op = &hooks()[0];
        let mut doc = shared.clone();
        apply(&mut doc, op, P).unwrap();
        assert_eq!(
            doc["hooks"]["Stop"],
            json!([{
                "matcher": "*",
                "hooks": [
                    { "type": "command", "command": "tool harness hook claude stop" },
                    { "type": "command", "command": "~/bin/notify.sh" }
                ]
            }])
        );
        assert_eq!(extract(&doc, op).as_ref(), Some(op.value()));
    }

    // @zen-test: KIT-10_AC-1
    // @zen-test: KIT-10_AC-3
    #[test]
    fn the_group_replaces_the_tools_entries_and_keeps_others() {
        let existing = r#"{
    "other": true,
    "hooks": {
        "PreToolUse": [],
        "Stop": [
            { "hooks": [ { "type": "command", "command": "echo hi" } ] },
            { "hooks": [ { "type": "command", "command": "tool harness hook claude old" } ] }
        ]
    },
    "z": [1, 2]
}"#;
        let out = render_merge(Some(existing), &hooks(), P).unwrap().unwrap();
        let doc = parse_json(P, &out).unwrap();
        let keys: Vec<_> = doc.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, ["other", "hooks", "z"]);
        assert_eq!(doc["hooks"]["PreToolUse"], json!([]));
        assert_eq!(doc["hooks"]["Stop"][0]["hooks"][0]["command"], "echo hi");
        assert_eq!(
            doc["hooks"]["Stop"][1]["hooks"][0]["command"],
            "tool harness hook claude stop"
        );
        assert_eq!(doc["hooks"]["Stop"].as_array().unwrap().len(), 2);
        assert!(out.starts_with("{\n    \"other\": true,"), "{out}");
        assert!(!out.ends_with('\n'));
        assert_eq!(extract(&doc, &hooks()[0]), Some(hooks()[0].value().clone()));
        assert_eq!(render_merge(Some(&out), &hooks(), P).unwrap(), None);
        let out = render_merge(Some("{}\n"), &hooks(), P).unwrap().unwrap();
        let doc = parse_json(P, &out).unwrap();
        assert_eq!(doc["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn groups_for_several_events_and_object_members() {
        let ops = vec![
            hook("SessionStart", "tool harness hook claude session-start"),
            hook("Stop", "tool harness hook claude stop"),
            mcp()[0].clone(),
            MergeOp::object_member(Vec::<String>::new(), "top", 1),
        ];
        let out = render_merge(Some("{\"mcpServers\": {\"x\": {}}}"), &ops, P)
            .unwrap()
            .unwrap();
        let doc = parse_json(P, &out).unwrap();
        let keys: Vec<_> = doc["mcpServers"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys, ["x", "tool"]);
        assert_eq!(doc["top"], 1);
        for op in &ops {
            assert_eq!(extract(&doc, op).as_ref(), Some(op.value()));
        }
        // Each event holds its own group.
        assert_eq!(doc["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert_eq!(doc["hooks"]["Stop"].as_array().unwrap().len(), 1);
        // A member of another value is found, so it can be compared.
        let edited = json!({"mcpServers": {"tool": {"command": "other"}}});
        assert_eq!(extract(&edited, &ops[2]), Some(json!({"command": "other"})));
        assert_eq!(render_merge(Some(&out), &ops, P).unwrap(), None);
    }

    // @zen-test: KIT-10_AC-2
    #[test]
    fn a_file_that_does_not_parse_is_refused_and_named() {
        let e = render_merge(Some("{ nope"), &hooks(), P).unwrap_err();
        assert!(matches!(&e, Error::File { file, .. } if file == P), "{e}");
        assert!(
            e.to_string()
                .starts_with(".claude/settings.json: does not parse"),
            "{e}"
        );
        let e = parse_json(P, "[]").unwrap_err();
        assert!(e.to_string().contains("not a JSON object"), "{e}");
        let e = render_merge(Some("{\"permissions\": []}"), &permissions(), P).unwrap_err();
        assert!(
            e.to_string().contains("`permissions` is not an object"),
            "{e}"
        );
        let e = render_merge(
            Some("{\"permissions\": {\"allow\": {}}}"),
            &permissions(),
            P,
        )
        .unwrap_err();
        assert!(
            e.to_string()
                .contains("`permissions.allow` is not an array"),
            "{e}"
        );
        let e = render_merge(Some("{\"hooks\": []}"), &hooks(), P).unwrap_err();
        assert!(e.to_string().contains("`hooks` is not an object"), "{e}");
        let e = render_merge(
            Some("{\"m\": 1}"),
            &[MergeOp::object_member(["m"], "k", 1)],
            P,
        )
        .unwrap_err();
        assert!(e.to_string().contains("`m` is not an object"), "{e}");
        let bad = MergeOp::array_entry(Vec::<String>::new(), 1);
        assert!(matches!(
            render_merge(None, &[bad], P),
            Err(Error::Internal(_))
        ));
        let e = parse_toml("c.toml", "a = ").unwrap_err();
        assert!(
            matches!(&e, Error::File { file, .. } if file == "c.toml"),
            "{e}"
        );
    }

    fn arb_key() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "model",
            "env",
            "theme",
            "cleanupPeriodDays",
            "statusLine",
        ])
        .prop_map(str::to_string)
    }

    fn arb_scalar() -> impl Strategy<Value = Value> {
        prop_oneof![
            any::<bool>().prop_map(Value::from),
            (0i64..1000).prop_map(Value::from),
            "[a-z]{1,8}".prop_map(Value::from),
        ]
    }

    /// A settings object: other keys before and after the tool's, an `allow`
    /// list with or without the entry, hook lists with or without the tool's
    /// groups and with other groups, an `mcpServers` object, and a nested
    /// object whose order must survive.
    fn arb_settings_object() -> impl Strategy<Value = Value> {
        (
            prop::collection::vec((arb_key(), arb_scalar()), 0..3),
            prop::option::of(prop::collection::vec(
                prop::sample::select(vec!["Bash(npm run *)", "Bash(tool *)", "Read(./src/**)"]),
                0..3,
            )),
            prop::option::of(prop::collection::vec(
                prop::sample::select(vec![
                    "echo hi",
                    "tool harness hook claude stop",
                    "make test",
                ]),
                0..3,
            )),
            prop::option::of(prop::sample::select(vec!["other", "tool"])),
            prop::collection::vec((arb_key(), arb_scalar()), 0..3),
        )
            .prop_map(|(before, allow, stop, server, after)| {
                let mut map = Map::new();
                for (k, v) in before {
                    map.insert(k, v);
                }
                if let Some(allow) = allow {
                    map.insert(
                        "permissions".into(),
                        json!({ "allow": allow, "deny": ["Bash(rm *)"] }),
                    );
                }
                if let Some(stop) = stop {
                    let groups: Vec<_> = stop
                        .into_iter()
                        .map(|c| json!({ "hooks": [{ "type": "command", "command": c }] }))
                        .collect();
                    map.insert(
                        "hooks".into(),
                        json!({ "PreToolUse": [], "Stop": groups.clone(), "SessionStart": groups }),
                    );
                }
                if let Some(name) = server {
                    map.insert(
                        "mcpServers".into(),
                        json!({ name: { "command": "x" }, "z": {} }),
                    );
                }
                for (k, v) in after {
                    map.insert(k, v);
                }
                map.insert("nested".into(), json!({ "z": 1, "a": { "y": 2, "b": 3 } }));
                Value::Object(map)
            })
    }

    fn arb_indent() -> impl Strategy<Value = String> {
        prop::sample::select(vec!["  ", "    ", "\t"]).prop_map(str::to_string)
    }

    fn strip_entries(v: &Value, ops: &[MergeOp]) -> Value {
        let mut v = v.clone();
        for op in ops {
            remove(&mut v, op);
        }
        v
    }

    fn keys_in_order(v: &Value) -> Vec<String> {
        v.as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // @zen-test: KIT_P-2
        #[test]
        fn merge_keeps_the_rest(object in arb_settings_object(), indent in arb_indent(), newline in any::<bool>(), which in 0usize..3) {
            let (ops, container) = match which {
                0 => (hooks(), "hooks"),
                1 => (permissions(), "permissions"),
                _ => (mcp(), "mcpServers"),
            };
            let text = json_text(&object, &indent, newline);
            let Some(after) = render_merge(Some(&text), &ops, P).unwrap() else {
                for op in &ops {
                    prop_assert_eq!(extract(&object, op), Some(op.value().clone()));
                }
                return Ok(());
            };
            let parsed: Value = serde_json::from_str(&after).unwrap();
            for op in &ops {
                prop_assert_eq!(extract(&parsed, op), Some(op.value().clone()));
            }
            // Everything but the tool's entries survives; containers the
            // input lacked are created and hold nothing else.
            let mut stripped = strip_entries(&parsed, &ops);
            if object.get(container).is_none() {
                stripped.as_object_mut().unwrap().shift_remove(container);
            } else if container == "hooks" {
                // Events the input lacked are created, empty once stripped.
                let hooks = stripped["hooks"].as_object_mut().unwrap();
                hooks.retain(|k, v| object["hooks"].get(k).is_some() || v != &json!([]));
            }
            prop_assert_eq!(stripped, strip_entries(&object, &ops));
            let mut expected = keys_in_order(&object);
            if !expected.iter().any(|k| k == container) {
                expected.push(container.to_string());
            }
            prop_assert_eq!(keys_in_order(&parsed), expected);
            prop_assert_eq!(indent_of(&after), indent);
            prop_assert_eq!(after.ends_with('\n'), newline);
            prop_assert_eq!(render_merge(Some(&after), &ops, P).unwrap(), None);
        }
    }
}
