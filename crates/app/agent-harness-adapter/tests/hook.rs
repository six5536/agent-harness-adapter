//! `agent-harness-adapter hook`: every harness and event, the command's answers translated
//! as the library renders them.

mod common;

use std::path::PathBuf;

use agent_harness_adapter_core::{
    harness,
    hook::{Answer, Event},
};
use common::{adapter, output};

/// The fake tool (`examples/fake_tool.rs`), which cargo builds with the
/// tests: `examples/` in the nearest directory above this test's binary
/// that has one (its place differs between cargo's build-dir layouts).
fn fake_tool() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let name = format!("fake_tool{}", std::env::consts::EXE_SUFFIX);
    exe.ancestors()
        .map(|d| d.join("examples").join(&name))
        .find(|p| p.is_file())
        .expect("the fake_tool example is built")
}

fn hook(harness: &str, event: &str, stdin: &str, mode: &str) -> (i32, String, String) {
    output(
        adapter()
            .args(["hook", harness, event, "--"])
            .arg(fake_tool())
            .arg(mode)
            .write_stdin(stdin),
    )
}

fn answer(mode: &str) -> Answer {
    match mode {
        "allow" => Answer::Allow { stderr: None },
        "deny" => Answer::Deny {
            reason: "no".into(),
        },
        "continue" => Answer::Continue {
            reason: "go on".into(),
        },
        _ => unreachable!(),
    }
}

// @zen-test: AHA-4_AC-1
#[test]
fn every_harness_and_event_answers_as_the_library_renders() {
    for h in harness::builtin() {
        for &event in h.hook_events() {
            for mode in ["allow", "deny", "continue"] {
                let label = format!("{} {event} {mode}", h.id());
                let (code, out, err) = hook(h.id(), event.as_str(), "{}", mode);
                assert!(err.starts_with("fake-tool ran "), "{label}: {err}");
                match h.answer(event, &answer(mode)) {
                    Ok(want) => {
                        assert_eq!(out, format!("{}\n", want.stdout), "{label}");
                        assert_eq!(code, i32::from(want.exit), "{label}");
                    }
                    Err(e) => {
                        assert_eq!(out, "", "{label}");
                        assert_eq!(code, 1, "{label}");
                        assert!(err.ends_with(&format!("error: {e}\n")), "{label}: {err}");
                    }
                }
            }
        }
    }
}

// @zen-test: AHA-4_AC-1
// @zen-test: AHA-2_AC-1
#[test]
fn the_command_reads_the_contracts_input() {
    let stdin = r#"{"session_id":"s1","cwd":"/w","hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":"ls"}}"#;
    let (code, out, _) = hook("claude", "pre-tool", stdin, "context");
    // Claude cannot add context before a tool: the library says so.
    assert_eq!((code, out.as_str()), (1, ""));
    let (code, out, _) = hook("claude", "prompt-submit", stdin, "context");
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let text = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    let input: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(input["v"], 1);
    assert_eq!(input["harness"], "claude");
    assert_eq!(input["event"], "prompt-submit");
    assert_eq!(input["session_id"], "s1");
    assert_eq!(input["raw"]["tool_input"]["command"], "ls");
}

// @zen-test: AHA-4_AC-3
#[test]
fn input_that_is_not_json_sends_only_harness_and_event() {
    let (code, out, _) = hook("claude", "session-start", "not json", "context");
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let text = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert_eq!(
        text,
        r#"{"v":1,"harness":"claude","event":"session-start"}"#
    );
}

// @zen-test: AHA-4_AC-2
#[test]
fn a_failing_command_allows_and_says_why() {
    let claude = harness::find("claude").unwrap();
    let allow = claude
        .answer(Event::Stop, &Answer::Allow { stderr: None })
        .unwrap()
        .stdout;
    for (mode, why) in [
        ("fail", "fake_tool: exited with 3\n"),
        ("garbage", "fake_tool: hook answer: expected"),
    ] {
        let (code, out, err) = hook("claude", "stop", "{}", mode);
        assert_eq!((code, out.clone()), (0, format!("{allow}\n")), "{mode}");
        assert!(err.contains(why), "{mode}: {err}");
    }
    let (code, out, err) = output(
        adapter()
            .args([
                "hook",
                "--tool",
                "mine",
                "claude",
                "stop",
                "--",
                "no-such-command-x",
            ])
            .write_stdin("{}"),
    );
    assert_eq!((code, out), (0, format!("{allow}\n")));
    assert!(
        err.starts_with("mine: could not run `no-such-command-x`"),
        "{err}"
    );
}

// @zen-test: AHA-4_AC-4
#[test]
fn an_unknown_harness_or_event_runs_nothing() {
    for (h, e, says) in [
        ("vim", "stop", "error: no harness named `vim`\n"),
        ("claude", "later", "error: no hook event named `later`\n"),
    ] {
        let (code, out, err) = hook(h, e, "{}", "allow");
        assert_eq!((code, out.as_str(), err.as_str()), (1, "", says));
    }
    let (code, _, _) = output(adapter().args(["hook", "claude", "stop"]));
    assert_eq!(code, 2);
}

// @zen-test: AHA-4_AC-5
#[test]
fn tools_runs_the_command_only_for_its_kind() {
    let run = |tool: &str| {
        output(
            adapter()
                .args(["hook", "--tools", "shell", "claude", "pre-tool", "--"])
                .arg(fake_tool())
                .arg("deny")
                .write_stdin(format!(r#"{{"tool_name":"{tool}","tool_input":{{}}}}"#)),
        )
    };
    // Another kind: allowed, the command never runs.
    let (code, out, err) = run("Read");
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let claude = harness::find("claude").unwrap();
    let allow = claude
        .answer(Event::PreTool, &Answer::Allow { stderr: None })
        .unwrap()
        .stdout;
    assert_eq!(out, format!("{allow}\n"));
    // Its kind: the command runs and denies.
    let (code, out, err) = run("Bash");
    assert_eq!(code, 0);
    assert!(err.starts_with("fake-tool ran"), "{err}");
    assert!(out.contains("deny"), "{out}");
    // A bad kind is a usage error.
    let (code, _, err) =
        output(adapter().args(["hook", "--tools", "net", "claude", "stop", "--", "x"]));
    assert_eq!(code, 2);
    assert!(err.contains("no tool kind `net`"), "{err}");
}
