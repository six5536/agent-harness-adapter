//! The built-in harnesses installed together on temporary trees: shared
//! locations written once, each harness's own files, and a second install
//! that writes nothing.

mod common;

use agent_harness_kit_core::{
    InstallOptions, InstallResult, Scope, State, harness, install, status,
};
use common::{TempTree, parts, verbs};

fn install_all(tree: &TempTree, ids: &[&str]) -> InstallResult {
    install(
        &tree.tool().with(harness::builtin()),
        &InstallOptions::new(ids.iter().copied(), Scope::Project),
    )
    .unwrap()
}

#[test]
fn claude_and_codex_each_get_their_files() {
    let tree = TempTree::empty("claude-codex");
    let out = install_all(&tree, &["codex", "claude"]);
    insta::assert_snapshot!(out.to_text(), @r"
    claude:
      created CLAUDE.md (instructions)
      created .claude/skills (skills)
      created .claude/settings.json (hooks)
      created .mcp.json (mcp)
      created .claude/settings.json (permissions)
    codex:
      created AGENTS.md (instructions)
      created .agents/skills (skills)
      created .codex/hooks.json (hooks)
      created .codex/config.toml (mcp)
      unsupported: permissions
      note: Codex reads the project's .codex files only once you trust the project
      note: new hooks run only once approved in Codex's /hooks
      note: restart Codex to load the changes
    ");
    let hooks: serde_json::Value = serde_json::from_str(&tree.read(".codex/hooks.json")).unwrap();
    assert_eq!(
        hooks["hooks"]["Stop"][0]["hooks"][0]["command"],
        "tool harness hook codex stop"
    );
    assert_eq!(
        tree.read(".codex/config.toml"),
        "[mcp_servers.tool]\ncommand = \"tool\"\nargs = [\"mcp\"]\n"
    );
    let before = tree.files();
    let again = install_all(&tree, &["claude", "codex"]);
    assert_eq!(tree.files(), before);
    for id in ["claude", "codex"] {
        assert!(
            verbs(&again, id).iter().all(|v| v == "current"),
            "{again:?}"
        );
        assert!(again.harness(id).unwrap().notes.is_empty());
    }
}

// @zen-test: HAR-9_AC-1
#[test]
fn agents_md_and_skills_are_written_once() {
    let tree = TempTree::empty("agents-shared");
    tree.write(
        ".gemini/settings.json",
        "{\"context\": {\"fileName\": [\"AGENTS.md\"]}}\n",
    );
    let out = install_all(&tree, &["codex", "factory", "gemini", "agents"]);
    for id in ["factory", "gemini", "agents"] {
        let p = parts(&out, id);
        assert_eq!(p[0].part, "instructions");
        assert_eq!(
            (p[0].state, p[0].path.as_str(), p[0].by.as_deref()),
            (State::Shared, "AGENTS.md", Some("codex")),
            "{id}"
        );
    }
    // Factory keeps its user skills apart; the project ones are shared.
    assert_eq!(parts(&out, "gemini")[1].state, State::Shared);
    assert_eq!(parts(&out, "factory")[1].by.as_deref(), Some("codex"));
    assert!(out.warnings.is_empty(), "{:?}", out.warnings);
    let agents = tree.read("AGENTS.md");
    assert_eq!(agents.matches("<!-- tool:harness -->").count(), 1);
    assert!(!tree.exists("GEMINI.md"));
    // Gemini's settings get its hooks, MCP server and allowed command beside
    // the user's own setting.
    let settings: serde_json::Value =
        serde_json::from_str(&tree.read(".gemini/settings.json")).unwrap();
    assert_eq!(settings["context"]["fileName"][0], "AGENTS.md");
    assert_eq!(
        settings["hooks"]["AfterAgent"][0]["hooks"][0]["command"],
        "tool harness hook gemini stop"
    );
    assert_eq!(settings["tools"]["allowed"][0], "run_shell_command(tool)");
    let record = tree.read(".tool/harness.toml");
    assert!(record.contains("[codex]\n"), "{record}");
    // Every part of `agents` is shared: an empty table keeps it installed.
    assert!(record.contains("\n[agents]\n\n"), "{record}");
    let before = tree.files();
    install_all(&tree, &["agents", "gemini", "factory", "codex"]);
    assert_eq!(tree.files(), before);
}

#[test]
fn claude_importing_agents_md_shares_it_with_codex() {
    let tree = TempTree::empty("import-shared");
    tree.write("AGENTS.md", "# Agents\n");
    tree.write("CLAUDE.md", "@AGENTS.md\n");
    let out = install_all(&tree, &["claude", "codex"]);
    assert_eq!(parts(&out, "claude")[0].path, "AGENTS.md");
    assert_eq!(parts(&out, "codex")[0].state, State::Shared);
    assert_eq!(parts(&out, "codex")[0].by.as_deref(), Some("claude"));
    assert_eq!(tree.read("CLAUDE.md"), "@AGENTS.md\n");
    // Named alone, codex is the writer of AGENTS.md, and finds it current.
    let st = status(
        &tree.tool().with(harness::builtin()),
        ["codex"],
        Scope::Project,
    )
    .unwrap();
    assert_eq!(parts(&st, "codex")[0].state, State::Current);
    let st = status(
        &tree.tool().with(harness::builtin()),
        ["claude", "codex"],
        Scope::Project,
    )
    .unwrap();
    assert_eq!(parts(&st, "codex")[0].state, State::Shared);
}

// @zen-test: KIT-10_AC-5
#[test]
fn codex_config_keeps_the_users_toml() {
    let tree = TempTree::empty("codex-toml");
    let config = "# mine\nmodel = \"o3\" # pinned\n\n[mcp_servers.other]\ncommand = \"x\"\n";
    tree.write(".codex/config.toml", config);
    let out = install_all(&tree, &["codex"]);
    assert_eq!(parts(&out, "codex")[3].verb(), "updated");
    let after = tree.read(".codex/config.toml");
    assert!(after.starts_with(config), "{after}");
    assert!(
        after.ends_with("[mcp_servers.tool]\ncommand = \"tool\"\nargs = [\"mcp\"]\n"),
        "{after}"
    );
    let st = status(
        &tree.tool().with(harness::builtin()),
        ["codex"],
        Scope::Project,
    )
    .unwrap();
    assert_eq!(parts(&st, "codex")[3].verb(), "current");
    // An edit by the user is left until forced.
    tree.write(
        ".codex/config.toml",
        &after.replace("args = [\"mcp\"]", "args = [\"serve\"]"),
    );
    let st = status(
        &tree.tool().with(harness::builtin()),
        ["codex"],
        Scope::Project,
    )
    .unwrap();
    assert_eq!(parts(&st, "codex")[3].verb(), "edited");
}

// @zen-test: KIT-19_AC-4
#[test]
fn cursor_reads_claudes_skills_and_agents_and_is_warned_about_hooks() {
    let tree = TempTree::empty("claude-cursor");
    let tool = tree.tool().with(harness::builtin());
    let out = install(
        &tool,
        &InstallOptions::new(["claude", "cursor"], Scope::Project),
    )
    .unwrap();
    let cursor = parts(&out, "cursor");
    let by: Vec<_> = cursor
        .iter()
        .map(|p| (p.part.as_str(), p.state, p.by.as_deref()))
        .collect();
    assert_eq!(
        by,
        [
            ("instructions", State::Absent, None),
            ("skills", State::Shared, Some("claude")),
            ("hooks", State::Absent, None),
            ("mcp", State::Absent, None),
            ("permissions", State::Absent, None),
        ]
    );
    assert!(tree.exists(".cursor/hooks.json"));
    assert!(!tree.exists(".agents/skills"));
    assert_eq!(
        out.warnings,
        [
            "cursor may load the instructions twice: CLAUDE.md, AGENTS.md",
            "cursor may load the hooks twice: .claude/settings.json, .cursor/hooks.json"
        ]
    );
    let hooks: serde_json::Value = serde_json::from_str(&tree.read(".cursor/hooks.json")).unwrap();
    assert_eq!(hooks["version"], 1);
    assert_eq!(
        hooks["hooks"]["stop"][0]["command"],
        "tool harness hook cursor stop"
    );
    assert_eq!(
        tree.read(".cursor/cli.json"),
        "{\n  \"permissions\": {\n    \"allow\": [\n      \"Shell(tool)\"\n    ]\n  }\n}\n"
    );
}

#[test]
fn copilot_shares_claudes_files() {
    let tree = TempTree::empty("claude-copilot");
    let out = install_all(&tree, &["claude", "copilot"]);
    let copilot: Vec<_> = parts(&out, "copilot")
        .iter()
        .map(|p| (p.part.as_str(), p.verb(), p.path.as_str()))
        .collect();
    assert_eq!(
        copilot,
        [
            ("instructions", "shared", "CLAUDE.md"),
            ("skills", "shared", ".claude/skills"),
            ("hooks", "created", ".github/hooks"),
            ("mcp", "shared", ".mcp.json"),
        ]
    );
    assert!(tree.exists(".github/hooks/tool.json"));
    assert!(!tree.exists("AGENTS.md"));
    assert_eq!(
        out.warnings,
        ["copilot may load the hooks twice: .claude/settings.json, .github/hooks"]
    );
    // With codex as well, AGENTS.md is needed and Copilot reads both.
    let out = install_all(&tree, &["claude", "codex", "copilot"]);
    assert!(
        out.warnings
            .contains(&"copilot may load the instructions twice: CLAUDE.md, AGENTS.md".to_string()),
        "{:?}",
        out.warnings
    );
    let before = tree.files();
    install_all(&tree, &["claude", "codex", "copilot"]);
    assert_eq!(tree.files(), before);
}

#[test]
fn every_harness_at_once_installs_and_settles() {
    let tree = TempTree::empty("all");
    let ids: Vec<String> = harness::builtin()
        .iter()
        .map(|h| h.id().to_string())
        .collect();
    let tool = tree.tool().with(harness::builtin());
    let out = install(&tool, &InstallOptions::new(ids.clone(), Scope::Project)).unwrap();
    assert_eq!(out.harnesses.len(), ids.len());
    let before = tree.files();
    let again = install(&tool, &InstallOptions::new(ids.clone(), Scope::Project)).unwrap();
    assert_eq!(tree.files(), before);
    for h in &again.harnesses {
        for p in &h.parts {
            assert!(
                matches!(p.state, State::Current | State::Shared),
                "{}: {p:?}",
                h.harness
            );
        }
    }
    // Every harness has a user scope.
    let user = install(&tool, &InstallOptions::new(ids, Scope::User));
    assert!(user.is_ok(), "{user:?}");
}
