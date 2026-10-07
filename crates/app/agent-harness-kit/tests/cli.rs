//! The `ahk` binary, run as a user runs it.

use assert_cmd::Command;

fn ahk() -> Command {
    Command::cargo_bin("ahk").unwrap()
}

#[test]
fn version_names_the_program() {
    let out = ahk().arg("--version").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert_eq!(text.trim(), format!("ahk {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn an_unknown_flag_is_a_usage_error() {
    ahk().arg("--definitely-not-a-flag").assert().code(2);
}
