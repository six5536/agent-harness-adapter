use std::{fs, path::PathBuf};

use super::*;

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let d = std::env::temp_dir().join(format!("aha-bind-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(d.join("proj")).unwrap();
        fs::create_dir_all(d.join("home")).unwrap();
        Dir(d)
    }

    fn options(&self, harnesses: &[&str]) -> Options {
        Options {
            harnesses: harnesses.iter().map(|s| s.to_string()).collect(),
            root: Some(self.0.join("proj")),
            home: Some(self.0.join("home")),
            ..Options::default()
        }
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn manifest() -> Value {
    json!({"version": 1, "name": "t", "instructions": "Use t.\n", "allow_commands": ["t"]})
}

// @zen-test: BND-1_AC-1
// @zen-test: BND-1_AC-2
#[test]
fn install_then_status_from_a_value_or_a_file() {
    let d = Dir::new("install");
    let out = install(Source::Json(manifest()), &d.options(&["claude"])).unwrap();
    assert_eq!(out["harnesses"][0]["harness"], "claude");
    assert_eq!(out["harnesses"][0]["parts"][0]["action"], "created");
    let file = d.0.join("t.harness.json");
    fs::write(&file, manifest().to_string()).unwrap();
    let st = status(Source::Path(file.clone()), &d.options(&[])).unwrap();
    assert_eq!(st["harnesses"][0]["parts"][0]["state"], "current");
    let all = status(Source::Path(file), &d.options(&["all"])).unwrap();
    assert_eq!(all["harnesses"].as_array().unwrap().len(), 9);
    let mut o = d.options(&["all"]);
    o.scope = Some("local".into());
    o.without = Some(vec!["permissions".into()]);
    o.force = true;
    let local = install(Source::Json(manifest()), &o).unwrap();
    assert_eq!(local["scope"], "local");
    assert_eq!(local["harnesses"].as_array().unwrap().len(), 2);
}

// @zen-test: BND-1_AC-3
#[test]
fn refusals_are_the_kits_messages() {
    let d = Dir::new("errors");
    let e = install(Source::Json(manifest()), &d.options(&["vim"])).unwrap_err();
    assert_eq!(e, "no harness named `vim`");
    let e = install(Source::Json(json!({"version": 2})), &d.options(&[])).unwrap_err();
    assert!(e.starts_with("manifest: version 2"), "{e}");
    let mut o = d.options(&["claude"]);
    o.scope = Some("global".into());
    let e = status(Source::Json(manifest()), &o).unwrap_err();
    assert_eq!(e, "no scope named `global`");
    let e = status(Source::Path(d.0.join("none.toml")), &o).unwrap_err();
    assert!(e.contains("no such file"), "{e}");
    assert!(
        Options::parse(r#"{"harness": []}"#)
            .unwrap_err()
            .contains("unknown field")
    );
    let o = Options::parse(r#"{"harnesses": ["pi"], "force": true}"#).unwrap();
    assert_eq!((o.harnesses.len(), o.force), (1, true));
}

// @zen-test: BND-2_AC-1
// @zen-test: BND-2_AC-2
// @zen-test: BND-2_AC-3
#[test]
fn hooks_parse_and_answer_through_the_harness() {
    let input = parse_hook(
        "claude",
        "stop",
        r#"{"session_id":"s","stop_hook_active":true}"#,
    )
    .unwrap();
    assert_eq!(
        (&input["v"], &input["session_id"], &input["continuing"]),
        (&json!(1), &json!("s"), &json!(true))
    );
    assert_eq!(
        parse_hook("codex", "stop", "nope").unwrap(),
        json!({"v": 1, "harness": "codex", "event": "stop"})
    );
    let out = answer_hook("claude", "stop", r#"{"answer":"continue","reason":"go"}"#).unwrap();
    assert_eq!(
        out,
        json!({"stdout": "{\"decision\":\"block\",\"reason\":\"go\"}\n", "stderr": null, "exit": 0})
    );
    let out = answer_hook("claude", "stop", r#"{"answer":"allow","stderr":"note"}"#).unwrap();
    assert_eq!(out["stderr"], "note");
    for (h, e, a, says) in [
        ("vim", "stop", "{}", "no harness named `vim`"),
        ("claude", "later", "{}", "no hook event named `later`"),
        ("claude", "stop", r#"{"answer":"maybe"}"#, "hook answer"),
        (
            "claude",
            "stop",
            r#"{"answer":"deny","reason":"r"}"#,
            "cannot answer deny at stop",
        ),
    ] {
        let err = answer_hook(h, e, a).unwrap_err();
        assert!(err.contains(says), "{h} {e} {a}: {err}");
    }
    assert!(parse_hook("claude", "later", "{}").is_err());
}

// @zen-test: BND-3_AC-1
// @zen-test: BND-3_AC-2
#[test]
fn schemas_and_versions() {
    for c in ["manifest", "hook-input", "hook-answer", "result"] {
        assert!(
            schema(c).unwrap()["title"]
                .as_str()
                .unwrap()
                .starts_with("AHA ")
        );
    }
    assert!(schema("nope").unwrap_err().contains("no contract `nope`"));
    assert_eq!((MANIFEST_VERSION, HOOK_VERSION), (1, 1));
}

// @zen-test: BND-1_AC-4
#[test]
fn uninstall_takes_it_back_out() {
    let d = Dir::new("uninstall");
    install(Source::Json(manifest()), &d.options(&["claude", "codex"])).unwrap();
    let out = uninstall(Source::Json(manifest()), &d.options(&["all"])).unwrap();
    let names: Vec<_> = out["harnesses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["harness"].clone())
        .collect();
    assert_eq!(names, [json!("claude"), json!("codex")]);
    assert_eq!(out["harnesses"][0]["parts"][0]["action"], "removed");
    assert_eq!(fs::read_dir(d.0.join("proj")).unwrap().count(), 0);
}
