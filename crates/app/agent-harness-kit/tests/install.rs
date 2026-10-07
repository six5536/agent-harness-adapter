//! `ahk install` and `ahk status` from a manifest.

mod common;

use common::{Tree, ahk, output};

const MANIFEST: &str = r#"
version = 1
name = "mytool"
instructions = "Use mytool.\n"
allow_commands = ["mytool"]

[[hooks]]
event = "stop"
run = "mytool decide"
"#;

fn tree() -> Tree {
    let t = Tree::new();
    t.write("mytool.harness.toml", MANIFEST);
    t.mkdir("proj");
    t
}

fn run(t: &Tree, args: &[&str]) -> (i32, String, String) {
    output(
        ahk()
            .current_dir(t.path("proj"))
            .env("HOME", t.path("home"))
            .args(args)
            .args(["--manifest", "../mytool.harness.toml"]),
    )
}

// @zen-test: AHK-3_AC-1
// @zen-test: AHK-3_AC-3
#[test]
fn install_writes_each_harness_and_reports_it() {
    let t = tree();
    let (code, out, err) = run(&t, &["install", "--harness", "claude,codex"]);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert!(
        out.starts_with("claude:\n  created CLAUDE.md (instructions)\n"),
        "{out}"
    );
    assert!(
        out.contains("codex:\n  created AGENTS.md (instructions)\n"),
        "{out}"
    );
    let settings = t.read("proj/.claude/settings.json");
    assert!(
        settings.contains("ahk hook --tool mytool claude stop -- mytool decide"),
        "{settings}"
    );
    assert!(t.read("proj/.mytool/harness.toml").contains("[claude]"));
    // Again: nothing to do; as JSON.
    let (code, out, _) = run(&t, &["install", "--harness", "claude", "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["harnesses"][0]["parts"][0]["state"], "current");
    assert_eq!(out.lines().count(), 1);
}

// @zen-test: AHK-3_AC-1
#[test]
fn all_scope_root_and_without() {
    let t = tree();
    let (code, out, _) = run(
        &t,
        &[
            "install",
            "--harness",
            "all",
            "--scope",
            "local",
            "--root",
            ".",
        ],
    );
    assert_eq!(code, 0, "{out}");
    let names: Vec<&str> = out.lines().filter(|l| !l.starts_with(' ')).collect();
    assert_eq!(names, ["claude:", "copilot:"]);
    assert!(t.exists("proj/.mytool/harness.local.toml"));
    let (code, out, _) = run(&t, &["install", "--harness", "all", "--scope", "user"]);
    assert_eq!(code, 0, "{out}");
    assert!(t.exists("home/.claude/CLAUDE.md"));
    assert!(t.exists("home/.mytool/harness.toml"));
    let (code, out, _) = run(
        &t,
        &[
            "install",
            "--harness",
            "gemini",
            "--without",
            "hooks,permissions",
        ],
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("skipped"), "{out}");
    assert!(t.read("proj/.mytool/config.toml").contains("without"));
}

// @zen-test: AHK-3_AC-2
#[test]
fn status_reports_the_installed_harnesses_by_default() {
    let t = tree();
    let (_, out, _) = run(&t, &["status"]);
    assert_eq!(out, "");
    run(&t, &["install", "--harness", "pi,claude"]);
    let (code, out, _) = run(&t, &["status"]);
    assert_eq!(code, 0);
    let names: Vec<&str> = out.lines().filter(|l| !l.starts_with(' ')).collect();
    assert_eq!(names, ["claude:", "pi:"]);
    assert!(out.contains("current"), "{out}");
    let (_, out, _) = run(&t, &["status", "--harness", "codex"]);
    // Pi wrote the AGENTS.md that Codex reads too.
    assert!(out.contains("current AGENTS.md (instructions)"), "{out}");
    assert!(out.contains("absent  .codex/hooks.json (hooks)"), "{out}");
}

// @zen-test: AHK-3_AC-4
#[test]
fn an_edited_part_is_left_with_a_note() {
    let t = tree();
    run(&t, &["install", "--harness", "claude"]);
    t.write(
        "proj/CLAUDE.md",
        "<!-- mytool:harness -->\nMine.\n<!-- /mytool:harness -->\n",
    );
    let (code, out, err) = run(&t, &["install", "--harness", "claude"]);
    assert_eq!(code, 0);
    assert!(out.contains("edited"), "{out}");
    assert!(
        err.contains("`ahk install --force` overwrites them"),
        "{err}"
    );
    let (_, _, err) = run(&t, &["install", "--harness", "claude", "--json"]);
    assert_eq!(err, "");
    let (_, out, err) = run(&t, &["install", "--harness", "claude", "--force"]);
    assert_eq!(err, "");
    assert!(out.contains("updated CLAUDE.md"), "{out}");
}

// @zen-test: AHK-3_AC-3
#[test]
fn errors_exit_2_with_one_line() {
    let t = tree();
    let (code, _, err) = run(&t, &["install", "--harness", "vim"]);
    assert_eq!((code, err.as_str()), (2, "error: no harness named `vim`\n"));
    t.write(
        "mytool.harness.toml",
        "version = 1\nname = \"mytool\"\nnmae = 1\n",
    );
    let (code, _, err) = run(&t, &["status"]);
    assert_eq!(code, 2);
    assert!(
        err.starts_with("error: ../mytool.harness.toml: TOML parse error at line 3"),
        "{err}"
    );
    let (code, _, err) = run(&t, &["install", "--harness", "claude", "--scope", "global"]);
    assert_eq!(code, 2);
    assert!(err.contains("no scope named `global`"), "{err}");
    let (code, _, err) = output(
        ahk()
            .current_dir(t.path("proj"))
            .env_remove("HOME")
            .env_remove("USERPROFILE")
            .args(["status", "--manifest", "../none.toml"]),
    );
    assert_eq!(code, 2);
    assert!(err.contains("none.toml: no such file"), "{err}");
}
