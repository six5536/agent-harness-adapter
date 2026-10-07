//! Taking a merge back out (KIT-22_AC-1, KIT-22_AC-2) and its property
//! (KIT_P-13).
use proptest::prelude::*;
use serde_json::json;

use super::*;
use crate::harness::{EntryMatch, merge::render_merge};

const P: &str = ".claude/settings.json";
const T: &str = ".codex/config.toml";

fn ops() -> Vec<MergeOp> {
    vec![
        MergeOp::group_entries(
            ["hooks", "Stop"],
            "hooks",
            "command",
            EntryMatch::Prefix("tool ".into()),
            vec![json!({ "hooks": [{ "type": "command", "command": "tool stop" }] })],
        ),
        MergeOp::array_entry(["permissions", "allow"], "Bash(tool *)"),
        MergeOp::object_member(["mcpServers"], "tool", json!({ "command": "tool" })),
        MergeOp::owned_entries(
            ["list"],
            "id",
            EntryMatch::Prefix("tool".into()),
            vec![json!({ "id": "tool-1" })],
        ),
    ]
}

/// Install `ops` into `before`, then take them out again.
fn round_trip(file: &str, before: Option<&str>, ops: &[MergeOp]) -> Option<String> {
    let merged = render_merge(file, before, ops).unwrap().unwrap();
    render_unmerge(file, &merged, ops).unwrap()
}

// @zen-test: KIT-22_AC-1
// @zen-test: KIT-22_AC-2
#[test]
fn a_file_the_merge_created_is_left_empty() {
    assert_eq!(round_trip(P, None, &ops()), Some(String::new()));
    let toml = [MergeOp::object_member(
        ["mcp_servers"],
        "tool",
        json!({ "command": "tool" }),
    )];
    assert_eq!(round_trip(T, None, &toml), Some(String::new()));
}

// @zen-test: KIT-22_AC-1
#[test]
fn the_users_entries_stay_in_their_place() {
    let before = r#"{
    "hooks": {
        "Stop": [
            { "matcher": "x", "hooks": [{ "type": "command", "command": "mine" }] }
        ]
    },
    "permissions": { "allow": ["Bash(ls)"] },
    "theme": "dark"
}
"#;
    // In the library's style first, so the round trip can be exact.
    let styled = json_text(&parse_json(P, before).unwrap(), "    ", true);
    assert_eq!(round_trip(P, Some(&styled), &ops()), Some(styled.clone()));
    // A group the user shares with the tool keeps the user's entry.
    let shared = r#"{
  "hooks": {
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "mine" },
          { "type": "command", "command": "tool stop" }
        ]
      }
    ]
  }
}
"#;
    let out = render_unmerge(P, shared, &ops()).unwrap().unwrap();
    let v: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v,
        json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "mine"}]}]}})
    );
}

#[test]
fn nothing_of_the_tools_is_no_change() {
    assert_eq!(
        render_unmerge(P, "{\n  \"a\": 1\n}\n", &ops()).unwrap(),
        None
    );
    assert_eq!(
        render_unmerge(T, "a = 1\n", &[MergeOp::object_member(["x"], "y", 1)]).unwrap(),
        None
    );
    let e = render_unmerge(P, "{ not json", &ops())
        .unwrap_err()
        .to_string();
    assert!(e.contains(P), "{e}");
}

// @zen-test: KIT-22_AC-1
#[test]
fn toml_tables_left_empty_go_and_comments_stay() {
    let before = "# mine\nmodel = \"x\"\n\n[mcp_servers.other]\ncommand = \"o\"\n";
    let op = [MergeOp::object_member(
        ["mcp_servers"],
        "tool",
        json!({ "command": "tool" }),
    )];
    assert_eq!(round_trip(T, Some(before), &op), Some(before.to_string()));
    let alone = "# mine\nmodel = \"x\"\n";
    assert_eq!(round_trip(T, Some(alone), &op), Some(alone.to_string()));
}

fn key() -> impl Strategy<Value = String> {
    "[a-s]{1,6}"
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    // @zen-test: KIT_P-13
    #[test]
    fn unmerge_undoes_merge(
        user in proptest::collection::btree_map(key(), any::<i32>(), 0..6),
        allow in proptest::collection::vec("[a-z]{1,5}", 0..4),
    ) {
        // The user's members never use the tool's keys (`t*` .. `z*`).
        let mut doc = serde_json::Map::new();
        for (k, v) in &user {
            doc.insert(k.clone(), json!(v));
        }
        if !allow.is_empty() {
            doc.insert("permissions".into(), json!({ "allow": allow }));
        }
        let before = if doc.is_empty() {
            None
        } else {
            Some(json_text(&Value::Object(doc), "  ", true))
        };
        let out = round_trip(P, before.as_deref(), &ops());
        prop_assert_eq!(out, Some(before.unwrap_or_default()));
    }
}
