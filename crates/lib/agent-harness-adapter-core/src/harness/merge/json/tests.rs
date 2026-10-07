//! The JSON merge's tests and its property (KIT_P-2).
use proptest::prelude::*;
use serde_json::json;

use super::*;

const P: &str = ".claude/settings.json";
const PREFIX: &str = "tool harness hook ";

/// A Claude Code style hook group op, built here so the core's tests do
/// not depend on the `claude` module.
fn hook(event: &str, command: &str) -> MergeOp {
    MergeOp::group_entries(
        ["hooks", event],
        "hooks",
        "command",
        EntryMatch::Prefix(PREFIX.into()),
        vec![json!({ "hooks": [{ "type": "command", "command": command }] })],
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
        Op::OwnedEntries {
            path, field, owned, ..
        } => {
            if let Some(array) = get_mut(doc, path).and_then(Value::as_array_mut) {
                array.retain(|e| !is_tool_entry(e, field, owned));
            }
        }
        Op::GroupEntries {
            path,
            entries,
            field,
            owned,
            ..
        } => {
            let Some(groups) = get_mut(doc, path).and_then(Value::as_array_mut) else {
                return;
            };
            for g in groups.iter_mut() {
                if let Some(es) = g[entries.as_str()].as_array_mut() {
                    es.retain(|e| !is_tool_entry(e, field, owned));
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
    // The tool's group has the same other keys: its entries go in place.
    let op = MergeOp::group_entries(
        ["hooks", "Stop"],
        "hooks",
        "command",
        EntryMatch::Prefix(PREFIX.into()),
        vec![
            json!({ "matcher": "*", "hooks": [{ "type": "command", "command": "tool harness hook claude stop" }] }),
        ],
    );
    let mut doc = shared.clone();
    apply(&mut doc, &op, P).unwrap();
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
    assert_eq!(extract(&doc, &op).as_ref(), Some(op.value()));
    // Other keys differ: the user's group keeps its own entry, the tool's
    // group is appended.
    let op = &hooks()[0];
    let mut doc = shared.clone();
    apply(&mut doc, op, P).unwrap();
    assert_eq!(
        doc["hooks"]["Stop"],
        json!([
            { "matcher": "*", "hooks": [{ "type": "command", "command": "~/bin/notify.sh" }] },
            { "hooks": [{ "type": "command", "command": "tool harness hook claude stop" }] }
        ])
    );
    assert_eq!(extract(&doc, op).as_ref(), Some(op.value()));
    // Found in the user's group, the tool's entry reads as that group.
    assert_eq!(
        extract(&shared, op),
        Some(
            json!([{ "matcher": "*", "hooks": [{ "type": "command", "command": "tool harness hook claude old" }] }])
        )
    );
}

// Two of the tool's groups for one event (e.g. two matchers), and a group
// the tool no longer has, which goes.
// @zen-test: KIT-10_AC-3
#[test]
fn several_groups_per_event() {
    let owned = EntryMatch::Prefix(PREFIX.into());
    let entry = |c: &str| json!({ "type": "command", "command": c });
    let op = MergeOp::group_entries(
        ["hooks", "PreToolUse"],
        "hooks",
        "command",
        owned,
        vec![
            json!({ "matcher": "Bash", "hooks": [entry("tool harness hook shell")] }),
            json!({ "matcher": "Edit", "hooks": [entry("tool harness hook edit")] }),
        ],
    );
    let doc = json!({"hooks": {"PreToolUse": [
        { "matcher": "Read", "hooks": [entry("tool harness hook read")] },
        { "matcher": "Edit", "hooks": [entry("echo mine"), entry("tool harness hook old")] },
    ]}});
    let mut out = doc.clone();
    apply(&mut out, &op, P).unwrap();
    assert_eq!(
        out["hooks"]["PreToolUse"],
        json!([
            { "matcher": "Edit", "hooks": [entry("echo mine"), entry("tool harness hook edit")] },
            { "matcher": "Bash", "hooks": [entry("tool harness hook shell")] },
        ])
    );
    let found = extract(&out, &op).unwrap();
    assert_eq!(found.as_array().unwrap().len(), 2);
    let mut again = out.clone();
    apply(&mut again, &op, P).unwrap();
    assert_eq!(again, out);
}

// @zen-test: KIT-10_AC-4
// @zen-test: KIT-10_AC-6
#[test]
fn owned_entries_replace_the_tools_and_keep_the_users() {
    let op = MergeOp::owned_entries(
        ["hooks", "stop"],
        "command",
        EntryMatch::Prefix(PREFIX.into()),
        vec![json!({ "command": "tool harness hook cursor stop" })],
    );
    let doc = json!({"version": 1, "hooks": {"stop": [
        { "command": "echo a" },
        { "command": "tool harness hook cursor old" },
        { "command": "echo b" },
        { "command": "tool harness hook cursor older" },
    ]}});
    assert_eq!(
        extract(&doc, &op),
        Some(
            json!([{ "command": "tool harness hook cursor old" }, { "command": "tool harness hook cursor older" }])
        )
    );
    let mut out = doc.clone();
    apply(&mut out, &op, P).unwrap();
    assert_eq!(
        out["hooks"]["stop"],
        json!([{ "command": "echo a" }, { "command": "tool harness hook cursor stop" }, { "command": "echo b" }])
    );
    assert_eq!(extract(&out, &op).as_ref(), Some(op.value()));
    // An op with nothing to write creates no container.
    let none = MergeOp::owned_entries(
        ["hooks", "stop"],
        "command",
        EntryMatch::Prefix(PREFIX.into()),
        vec![],
    );
    let mut empty = json!({});
    apply(&mut empty, &none, P).unwrap();
    assert_eq!(empty, json!({}));
    let mut old = doc.clone();
    apply(&mut old, &none, P).unwrap();
    assert_eq!(
        old["hooks"]["stop"],
        json!([{ "command": "echo a" }, { "command": "echo b" }])
    );
    let mut fresh = json!({});
    apply(&mut fresh, &op, P).unwrap();
    assert_eq!(
        fresh,
        json!({"hooks": {"stop": [{ "command": "tool harness hook cursor stop" }]}})
    );
    assert_eq!(extract(&json!({"hooks": {"stop": []}}), &op), None);
}

// A tool whose program users may rename owns its entries by a text they
// contain: a hook edited to another binary is the tool's, and edited.
// @zen-test: KIT-2_AC-3
#[test]
fn entries_owned_by_contained_text() {
    let op = MergeOp::group_entries(
        ["hooks", "Stop"],
        "hooks",
        "command",
        EntryMatch::Contains(" harness hook ".into()),
        vec![
            json!({ "hooks": [{ "type": "command", "command": "tool harness hook claude stop" }] }),
        ],
    );
    let doc = json!({"hooks": {"Stop": [{"hooks": [
        { "type": "command", "command": "/opt/bin/tool2 harness hook claude stop" },
        { "type": "command", "command": "echo hi" }
    ]}]}});
    let found = extract(&doc, &op).unwrap();
    assert_eq!(
        found,
        json!([{ "hooks": [{ "type": "command", "command": "/opt/bin/tool2 harness hook claude stop" }] }])
    );
    assert_ne!(&found, op.value());
    let mut out = doc.clone();
    apply(&mut out, &op, P).unwrap();
    assert_eq!(
        out["hooks"]["Stop"][0]["hooks"],
        json!([
            { "type": "command", "command": "tool harness hook claude stop" },
            { "type": "command", "command": "echo hi" }
        ])
    );
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
