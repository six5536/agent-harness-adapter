//! The correctness properties of `install` through the public API: a second
//! install writes nothing, and a refusal writes nothing.

mod common;

use agent_harness_adapter_core::{Error, InstallOptions, Scope, State, install, status};
use common::{TempTree, parts};
use proptest::prelude::*;
use serde::Serialize as _;

/// `value` pretty-printed with `indent`, with or without a final newline.
fn json_text(value: &serde_json::Value, indent: &str, newline: bool) -> String {
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(
        &mut buf,
        serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes()),
    );
    value.serialize(&mut ser).unwrap();
    let mut text = String::from_utf8(buf).unwrap();
    if newline {
        text.push('\n');
    }
    text
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

fn arb_scalar() -> impl Strategy<Value = serde_json::Value> {
    prop_oneof![
        any::<bool>().prop_map(serde_json::Value::from),
        (0i64..1000).prop_map(serde_json::Value::from),
        "[a-z]{1,8}".prop_map(serde_json::Value::from),
    ]
}

/// A settings object: other keys before and after the tool's, an `allow`
/// list with or without the entry, hook lists with or without the tool's
/// groups and with other groups, an `mcpServers` object, and a nested object
/// whose order must survive.
fn arb_settings_object() -> impl Strategy<Value = serde_json::Value> {
    (
        prop::collection::vec((arb_key(), arb_scalar()), 0..3),
        prop::option::of(prop::collection::vec(
            prop::sample::select(vec!["Bash(npm run *)", "Bash(tool *)", "Read(./src/**)"]),
            0..3,
        )),
        prop::option::of(prop::collection::vec(
            prop::sample::select(vec!["echo hi", "tool harness hook claude stop", "make test"]),
            0..3,
        )),
        prop::option::of(prop::sample::select(vec!["other", "tool"])),
        prop::collection::vec((arb_key(), arb_scalar()), 0..3),
    )
        .prop_map(|(before, allow, stop, server, after)| {
            let mut map = serde_json::Map::new();
            for (k, v) in before {
                map.insert(k, v);
            }
            if let Some(allow) = allow {
                map.insert(
                    "permissions".into(),
                    serde_json::json!({ "allow": allow, "deny": ["Bash(rm *)"] }),
                );
            }
            if let Some(stop) = stop {
                let groups: Vec<_> = stop
                    .into_iter()
                    .map(|c| serde_json::json!({ "hooks": [{ "type": "command", "command": c }] }))
                    .collect();
                map.insert(
                    "hooks".into(),
                    serde_json::json!({ "PreToolUse": [], "Stop": groups.clone(), "SessionStart": groups }),
                );
            }
            if let Some(name) = server {
                map.insert("mcpServers".into(), serde_json::json!({ name: { "command": "x" }, "z": {} }));
            }
            for (k, v) in after {
                map.insert(k, v);
            }
            map.insert("nested".into(), serde_json::json!({ "z": 1, "a": { "y": 2, "b": 3 } }));
            serde_json::Value::Object(map)
        })
}

fn arb_indent() -> impl Strategy<Value = String> {
    prop::sample::select(vec!["  ", "    ", "\t"]).prop_map(str::to_string)
}

fn arb_text() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z #@-]{0,12}", 0..6).prop_map(|lines| lines.join("\n"))
}

/// A tree with an instructions file of either kind, a settings file of any
/// style, and a declined list; `install` twice.
#[derive(Debug, Clone)]
struct Scenario {
    agents: Option<String>,
    claude: Option<String>,
    settings: Option<(serde_json::Value, String, bool)>,
    without: Vec<&'static str>,
}

fn arb_scenario() -> impl Strategy<Value = Scenario> {
    (
        prop::option::of(arb_text()),
        prop::option::of(prop_oneof![Just("@AGENTS.md\n".to_string()), arb_text()]),
        prop::option::of((arb_settings_object(), arb_indent(), any::<bool>())),
        prop::collection::btree_set(
            prop::sample::select(vec![
                "skills",
                "instructions",
                "mcp",
                "hooks",
                "permissions",
            ]),
            0..3,
        ),
    )
        .prop_map(|(agents, claude, settings, without)| Scenario {
            agents,
            claude,
            settings,
            without: without.into_iter().collect(),
        })
}

fn build(s: &Scenario) -> TempTree {
    let tree = TempTree::empty("scenario");
    if let Some(t) = &s.agents {
        tree.write("AGENTS.md", t);
    }
    if let Some(t) = &s.claude {
        tree.write("CLAUDE.md", t);
    }
    if let Some((object, indent, newline)) = &s.settings {
        tree.write(
            ".claude/settings.json",
            &json_text(object, indent, *newline),
        );
    }
    tree
}

fn opts(without: Vec<String>, name: &str) -> InstallOptions {
    let o = InstallOptions::new([name], Scope::Project);
    if without.is_empty() {
        o
    } else {
        o.without(without)
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    // @zen-test: KIT_P-5
    #[test]
    fn install_is_idempotent(s in arb_scenario()) {
        let tree = build(&s);
        let o = opts(s.without.iter().map(|w| w.to_string()).collect(), "claude");
        // An existing tool group or member the tool never wrote is edited;
        // force takes it over so the second run can be checked.
        install(&tree.tool(), &o.clone().force(true)).unwrap();
        let after_first = tree.files();
        let second = install(&tree.tool(), &o).unwrap();
        for p in parts(&second, "claude") {
            prop_assert!(p.action.is_none(), "{second:?}");
            prop_assert!(p.state == State::Current || p.state == State::Skipped, "{second:?}");
            prop_assert_eq!(p.state == State::Skipped, s.without.contains(&p.part.as_str()));
        }
        prop_assert_eq!(tree.files(), after_first);
        let st = status(&tree.tool(), ["claude"], Scope::Project).unwrap();
        prop_assert_eq!(parts(&st, "claude"), parts(&second, "claude"));
    }

    // @zen-test: KIT_P-6
    #[test]
    fn nothing_on_refusal(s in arb_scenario(), which in 0u8..5) {
        let tree = build(&s);
        let mut without: Vec<String> = s.without.iter().map(|w| w.to_string()).collect();
        let mut name = "claude";
        match which {
            0 => name = "cursor",
            1 => without.push("nope".into()),
            2 => {
                // A broken file refuses when a part that writes it is kept;
                // a declined part never reads it.
                without.retain(|w| w != "hooks");
                tree.write(".claude/settings.json", "{ not json");
            }
            3 => {
                // A container of another type refuses when the part is written.
                without.retain(|w| w != "mcp");
                tree.write(".mcp.json", "{\"mcpServers\": 1}");
            }
            _ => tree.write(".tool/harness.toml", "[claude\nx = "),
        }
        let before = tree.files();
        let o = InstallOptions::new([name], Scope::Project).without(without);
        let e = install(&tree.tool(), &o).unwrap_err();
        prop_assert!(
            matches!(e, Error::UnknownHarness { .. } | Error::UnknownPart { .. } | Error::File { .. }),
            "{e}"
        );
        prop_assert_eq!(tree.files(), before);
    }
}
