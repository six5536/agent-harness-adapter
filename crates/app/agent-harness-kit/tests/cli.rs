//! The `ahk` binary, run as a user runs it.

use std::path::Path;

use assert_cmd::Command;

fn ahk() -> Command {
    Command::cargo_bin("ahk").unwrap()
}

fn stdout(cmd: &mut Command) -> String {
    let out = cmd.assert().success();
    String::from_utf8(out.get_output().stdout.clone()).unwrap()
}

// @zen-test: AHK-6_AC-1
#[test]
fn version_names_the_program() {
    let text = stdout(ahk().arg("--version"));
    assert_eq!(text.trim(), format!("ahk {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn an_unknown_flag_is_a_usage_error() {
    ahk().arg("--definitely-not-a-flag").assert().code(2);
    ahk().args(["schema", "nope"]).assert().code(2);
}

// @zen-test: AHK-5_AC-1
// @zen-test: AHK-5_AC-2
#[test]
fn schemas_match_the_checked_in_files() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../schema");
    for contract in ["manifest", "hook-input", "hook-answer", "result"] {
        let printed = stdout(ahk().args(["schema", contract]));
        let file = dir.join(format!("{contract}.v1.json"));
        let kept = std::fs::read_to_string(&file).unwrap_or_default();
        assert!(
            printed == kept,
            "{} is stale: run `cargo run -p agent-harness-kit -- schema {contract} > schema/{contract}.v1.json`",
            file.display()
        );
        let v: serde_json::Value = serde_json::from_str(&printed).unwrap();
        assert!(v["title"].as_str().unwrap().starts_with("AHK "));
    }
}
