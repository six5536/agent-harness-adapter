//! `install` and `status` on temporary trees: each scenario through the
//! test tool, the tree, the record and the result checked, and every
//! refusal leaving the tree byte-identical.

mod common;

use agent_harness_kit::{
    Action, Error, InstallOptions, InstallResult, Result, Scope, State, install, status,
};
use common::{INSTRUCTIONS, SKILL, TempTree, parts, verbs};

fn run(
    tree: &TempTree,
    scope: Scope,
    without: Option<&[&str]>,
    force: bool,
) -> Result<InstallResult> {
    let mut opts = InstallOptions::new(["claude"], scope).force(force);
    if let Some(w) = without {
        opts = opts.without(w.iter().copied());
    }
    install(&tree.tool(), &opts)
}

fn project(tree: &TempTree, without: Option<&[&str]>, force: bool) -> InstallResult {
    run(tree, Scope::Project, without, force).unwrap()
}

fn st(tree: &TempTree) -> Vec<String> {
    verbs(
        &status(&tree.tool(), ["claude"], Scope::Project).unwrap(),
        "claude",
    )
}

// @zen-test: KIT-4_AC-1
// @zen-test: KIT-4_AC-3
// @zen-test: KIT-5_AC-1
// @zen-test: KIT-7_AC-2
#[test]
fn an_empty_project_gets_every_part_and_the_record() {
    let tree = TempTree::empty("empty");
    // Status writes nothing.
    assert!(st(&tree).iter().all(|s| s == "absent"));
    assert!(tree.files().is_empty());
    let out = project(&tree, None, false);
    let rows: Vec<_> = parts(&out, "claude")
        .iter()
        .map(|p| (p.part.as_str(), p.verb(), p.path.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("instructions", "created", "CLAUDE.md"),
            ("skills", "created", ".claude/skills"),
            ("hooks", "created", ".claude/settings.json"),
            ("mcp", "created", ".mcp.json"),
            ("permissions", "created", ".claude/settings.json"),
        ]
    );
    assert_eq!(out.scope, Scope::Project);
    assert_eq!(out.root, tree.dir());
    assert!(out.warnings.is_empty());
    assert_eq!(tree.read(".claude/skills/tool/SKILL.md"), SKILL);
    assert_eq!(
        tree.read("CLAUDE.md"),
        format!("<!-- tool:harness -->\n\n{INSTRUCTIONS}<!-- /tool:harness -->\n")
    );
    let settings: serde_json::Value =
        serde_json::from_str(&tree.read(".claude/settings.json")).unwrap();
    assert_eq!(settings["permissions"]["allow"][0], "Bash(tool *)");
    let keys: Vec<_> = settings["hooks"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    assert_eq!(keys, ["SessionStart", "UserPromptSubmit", "Stop"]);
    assert_eq!(
        settings["hooks"]["Stop"][0]["hooks"][0]["command"],
        "tool harness hook claude stop"
    );
    assert_eq!(
        settings["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"],
        "tool harness hook claude prompt-submit"
    );
    assert_eq!(
        tree.read(".mcp.json"),
        "{\n  \"mcpServers\": {\n    \"tool\": {\n      \"command\": \"tool\",\n      \"args\": [\n        \"mcp\"\n      ]\n    }\n  }\n}\n"
    );
    let record = tree.read(".tool/harness.toml");
    assert!(
        record.starts_with("# Written by tool harness install. Do not edit.\n\n[claude]\n"),
        "{record}"
    );
    for part in ["hooks", "instructions", "mcp", "permissions", "skills"] {
        assert!(record.contains(&format!("{part} = \"fnv1a64:")), "{record}");
    }
    assert!(!tree.exists(".tool/config.toml"));
    // Idempotent: the second run writes nothing and reports current.
    let before = tree.files();
    let again = project(&tree, None, false);
    assert!(
        verbs(&again, "claude").iter().all(|v| v == "current"),
        "{again:?}"
    );
    assert_eq!(tree.files(), before);
    assert!(st(&tree).iter().all(|s| s == "current"));
}

// @zen-test: KIT-2_AC-2
// @zen-test: HAR-1_AC-2
#[test]
fn the_instructions_target_follows_the_tree() {
    let tree = TempTree::empty("agents");
    tree.write("AGENTS.md", "# Agents\n");
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[0].path, "AGENTS.md");
    assert_eq!(parts(&out, "claude")[0].verb(), "updated");
    assert!(
        tree.read("AGENTS.md")
            .starts_with("# Agents\n\n<!-- tool:harness -->\n\n")
    );
    assert!(!tree.exists("CLAUDE.md"));
    let tree = TempTree::empty("import");
    tree.write("AGENTS.md", "# Agents\n");
    tree.write("CLAUDE.md", "@AGENTS.md\n");
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[0].path, "AGENTS.md");
    assert_eq!(tree.read("CLAUDE.md"), "@AGENTS.md\n");
    let tree = TempTree::empty("both");
    tree.write("AGENTS.md", "# Agents\n");
    tree.write("CLAUDE.md", "# Claude\n");
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[0].path, "CLAUDE.md");
    assert_eq!(tree.read("AGENTS.md"), "# Agents\n");
    assert!(
        tree.read("CLAUDE.md")
            .starts_with("# Claude\n\n<!-- tool:harness -->\n")
    );
}

// @zen-test: KIT-3_AC-2
#[test]
fn a_reflowed_block_is_current_and_left_alone() {
    let tree = TempTree::empty("reflow");
    project(&tree, None, false);
    let reflowed = tree
        .read("CLAUDE.md")
        .replace("uses tool.\nRead", "uses   tool.  Read")
        .replace('\n', "\r\n");
    tree.write("CLAUDE.md", &reflowed);
    let before = tree.files();
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[0].verb(), "current");
    assert_eq!(tree.files(), before);
}

// @zen-test: KIT-10_AC-1
#[test]
fn a_settings_file_of_another_style_keeps_its_entries_order_and_indent() {
    let tree = TempTree::empty("style");
    tree.write(
        ".claude/settings.json",
        "{\n    \"permissions\": {\n        \"allow\": [\"Bash(npm run *)\"]\n    },\n    \"hooks\": {\n        \"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"echo done\"}]}]\n    },\n    \"model\": \"opus\"\n}",
    );
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[2].verb(), "updated");
    assert_eq!(parts(&out, "claude")[4].verb(), "updated");
    let after = tree.read(".claude/settings.json");
    let doc: serde_json::Value = serde_json::from_str(&after).unwrap();
    let keys: Vec<_> = doc.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["permissions", "hooks", "model"]);
    assert_eq!(
        doc["permissions"]["allow"],
        serde_json::json!(["Bash(npm run *)", "Bash(tool *)"])
    );
    assert_eq!(doc["hooks"]["Stop"][0]["hooks"][0]["command"], "echo done");
    assert_eq!(
        doc["hooks"]["Stop"][1]["hooks"][0]["command"],
        "tool harness hook claude stop"
    );
    let keys: Vec<_> = doc["hooks"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["Stop", "SessionStart", "UserPromptSubmit"]);
    assert!(
        after.starts_with("{\n    \"permissions\": {\n        \"allow\": ["),
        "{after}"
    );
    assert!(!after.ends_with('\n'), "no trailing newline was there");
    assert!(st(&tree).iter().all(|s| s == "current"));
}

// @zen-test: KIT-3_AC-1
// @zen-test: KIT-4_AC-1
#[test]
fn a_part_the_tool_never_wrote_is_edited_until_forced() {
    let tree = TempTree::empty("foreign");
    tree.write(".claude/skills/tool/SKILL.md", "mine\n");
    tree.write(
        ".mcp.json",
        "{\"mcpServers\": {\"tool\": {\"command\": \"old\"}}}\n",
    );
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[1].verb(), "edited");
    assert_eq!(parts(&out, "claude")[3].verb(), "edited");
    assert_eq!(tree.read(".claude/skills/tool/SKILL.md"), "mine\n");
    let record = tree.read(".tool/harness.toml");
    assert!(
        !record.contains("skills") && !record.contains("mcp"),
        "{record}"
    );
    assert_eq!(st(&tree)[1], "edited");
    let out = project(&tree, None, true);
    assert_eq!(parts(&out, "claude")[1].verb(), "rewrote");
    assert_eq!(parts(&out, "claude")[3].verb(), "updated");
    assert_eq!(tree.read(".claude/skills/tool/SKILL.md"), SKILL);
    assert!(
        tree.read(".tool/harness.toml")
            .contains("skills = \"fnv1a64:")
    );
}

// @zen-test: KIT-4_AC-1
// @zen-test: KIT-4_AC-2
// @zen-test: KIT-7_AC-1
#[test]
fn every_part_edited_then_install_then_force() {
    let tree = TempTree::empty("edited");
    project(&tree, None, false);
    tree.write(".claude/skills/tool/SKILL.md", "changed\n");
    tree.write(
        "CLAUDE.md",
        &tree.read("CLAUDE.md").replace("Read the", "Skip the"),
    );
    let settings = tree
        .read(".claude/settings.json")
        .replace(
            "tool harness hook claude stop",
            "tool harness hook claude stop --quiet",
        )
        .replace("Bash(tool *)", "Bash(other *)");
    tree.write(".claude/settings.json", &settings);
    tree.write(
        ".mcp.json",
        &tree.read(".mcp.json").replace("\"mcp\"", "\"serve\""),
    );
    // The permission entry is gone, so absent; the hook is still the tool's
    // by its prefix, so edited.
    assert_eq!(
        st(&tree),
        ["edited", "edited", "edited", "edited", "absent"]
    );
    let out = project(&tree, None, false);
    assert_eq!(
        verbs(&out, "claude"),
        ["edited", "edited", "edited", "edited", "updated"]
    );
    assert_eq!(tree.read(".claude/skills/tool/SKILL.md"), "changed\n");
    assert!(tree.read("CLAUDE.md").contains("Skip the"));
    assert!(tree.read(".claude/settings.json").contains("--quiet"));
    let out = project(&tree, None, true);
    assert_eq!(
        verbs(&out, "claude"),
        ["updated", "rewrote", "updated", "updated", "current"]
    );
    assert!(st(&tree).iter().all(|s| s == "current"));
}

// @zen-test: KIT-3_AC-1
// @zen-test: KIT-4_AC-1
// @zen-test: KIT-10_AC-6
#[test]
fn a_changed_integration_makes_written_parts_stale() {
    let tree = TempTree::empty("stale");
    // An older version wrote only the stop hook and another block.
    install(
        &tree.old_tool(),
        &InstallOptions::new(["claude"], Scope::Project),
    )
    .unwrap();
    assert_eq!(
        st(&tree),
        ["stale", "current", "stale", "current", "current"]
    );
    let out = project(&tree, None, false);
    assert_eq!(
        verbs(&out, "claude"),
        ["updated", "current", "updated", "current", "current"]
    );
    assert_eq!(parts(&out, "claude")[0].state, State::Stale);
    assert_eq!(parts(&out, "claude")[0].action, Some(Action::Updated));
    assert!(st(&tree).iter().all(|s| s == "current"));
    // Back to the old version: the hooks it no longer has are removed.
    install(
        &tree.old_tool(),
        &InstallOptions::new(["claude"], Scope::Project),
    )
    .unwrap();
    let settings: serde_json::Value =
        serde_json::from_str(&tree.read(".claude/settings.json")).unwrap();
    assert_eq!(settings["hooks"]["SessionStart"], serde_json::json!([]));
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);
}

// @zen-test: KIT-8_AC-1
// @zen-test: KIT-8_AC-2
#[test]
fn a_declined_part_stays_declined_until_the_config_line_goes() {
    let tree = TempTree::empty("declined");
    let out = project(&tree, Some(&["hooks"]), false);
    assert_eq!(parts(&out, "claude")[2].verb(), "skipped");
    assert_eq!(
        tree.read(".tool/config.toml"),
        "[harness.claude]\nwithout = [\"hooks\"]\n"
    );
    assert!(!tree.read(".claude/settings.json").contains("hooks"));
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[2].verb(), "skipped");
    assert_eq!(st(&tree)[2], "skipped");
    let out = project(&tree, Some(&["skills"]), false);
    assert_eq!(
        verbs(&out, "claude"),
        ["current", "skipped", "updated", "current", "current"]
    );
    assert_eq!(
        tree.read(".tool/config.toml"),
        "[harness.claude]\nwithout = [\"skills\"]\n"
    );
    assert!(!tree.read(".tool/harness.toml").contains("skills = "));
    tree.write(".tool/config.toml", "# nothing declined\n");
    let out = project(&tree, None, false);
    assert!(
        verbs(&out, "claude").iter().all(|v| v == "current"),
        "{out:?}"
    );
    assert!(tree.read(".tool/harness.toml").contains("skills = "));
}

// @zen-test: KIT-1_AC-1
// @zen-test: KIT-2_AC-4
// @zen-test: KIT-17_AC-4
// @zen-test: KIT-21_AC-1
#[test]
fn the_user_scope_has_its_own_root_record_and_external_part() {
    let tree = TempTree::empty("user");
    let out = run(&tree, Scope::User, Some(&["skills"]), false).unwrap();
    let rows: Vec<_> = parts(&out, "claude")
        .iter()
        .map(|p| (p.part.as_str(), p.verb(), p.path.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("instructions", "created", ".claude/CLAUDE.md"),
            ("skills", "skipped", ".claude/skills"),
            ("hooks", "created", ".claude/settings.json"),
            ("permissions", "created", ".claude/settings.json"),
            ("claude-mcp", "created", "claude mcp (user)"),
        ]
    );
    // Claude Code keeps the user's MCP servers in its own state file.
    assert_eq!(
        out.harness("claude").unwrap().unsupported,
        [agent_harness_kit::Item::Mcp]
    );
    assert!(tree.exists("home/.claude/CLAUDE.md"));
    assert!(tree.exists("home/claude-mcp-user.json"));
    assert!(tree.exists("home/.config/tool/harness.toml"));
    assert_eq!(
        tree.read("home/.config/tool/config.toml"),
        "[harness.claude]\nwithout = [\"skills\"]\n"
    );
    assert!(!tree.exists("CLAUDE.md") && !tree.exists(".tool"));
    let again = run(&tree, Scope::User, None, false).unwrap();
    assert_eq!(
        verbs(&again, "claude"),
        ["current", "skipped", "current", "current", "current"]
    );
    // The external part edited: left, then forced.
    tree.write("home/claude-mcp-user.json", "{}");
    let s = status(&tree.tool(), ["claude"], Scope::User).unwrap();
    assert_eq!(parts(&s, "claude")[4].verb(), "edited");
    assert_eq!(
        parts(&run(&tree, Scope::User, None, false).unwrap(), "claude")[4].verb(),
        "edited"
    );
    assert_eq!(
        parts(&run(&tree, Scope::User, None, true).unwrap(), "claude")[4].verb(),
        "updated"
    );
    assert_eq!(
        parts(
            &status(&tree.tool(), ["claude"], Scope::User).unwrap(),
            "claude"
        )[4]
        .verb(),
        "current"
    );
}

// @zen-test: KIT-21_AC-1
#[test]
fn the_local_scope_takes_hooks_and_permissions() {
    let tree = TempTree::empty("local");
    let out = run(&tree, Scope::Local, None, false).unwrap();
    let rows: Vec<_> = parts(&out, "claude")
        .iter()
        .map(|p| (p.part.as_str(), p.path.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("hooks", ".claude/settings.local.json"),
            ("permissions", ".claude/settings.local.json")
        ]
    );
    let mut unsupported = out.harness("claude").unwrap().unsupported.clone();
    unsupported.sort();
    assert_eq!(unsupported.len(), 3);
    assert!(tree.exists(".tool/harness.local.toml"));
    assert!(!tree.exists(".claude/settings.json"));
}

// @zen-test: KIT-4_AC-4
#[test]
fn json_and_text_carry_the_same_parts() {
    let tree = TempTree::empty("json");
    let out = project(&tree, Some(&["permissions"]), false);
    let json = serde_json::to_value(&out).unwrap();
    assert_eq!(json["scope"], "project");
    let h = &json["harnesses"][0];
    assert_eq!(h["harness"], "claude");
    assert_eq!(h["parts"].as_array().unwrap().len(), 5);
    assert_eq!(
        h["parts"][4],
        serde_json::json!({ "part": "permissions", "state": "skipped", "path": ".claude/settings.json" })
    );
    let keys: Vec<_> = h["parts"][0].as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["part", "state", "action", "path"]);
    assert_eq!(h["parts"][0]["state"], "absent");
    assert_eq!(h["parts"][0]["action"], "created");
    let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, ["scope", "root", "harnesses", "warnings"]);
    insta::assert_snapshot!(out.to_text(), @r"
    claude:
      created CLAUDE.md (instructions)
      created .claude/skills (skills)
      created .claude/settings.json (hooks)
      created .mcp.json (mcp)
      skipped .claude/settings.json (permissions)
    ");
}

// @zen-test: KIT-1_AC-2
// @zen-test: KIT-6_AC-1
#[test]
fn every_refusal_leaves_the_tree_byte_identical() {
    let tree = TempTree::empty("refusals");
    tree.write("CLAUDE.md", "# Mine\n");
    let pristine = tree.files();
    let refused = |tree: &TempTree, name: &str, without: Option<&[&str]>| -> String {
        let mut opts = InstallOptions::new([name], Scope::Project);
        if let Some(w) = without {
            opts = opts.without(w.iter().copied());
        }
        let e = install(&tree.tool(), &opts).unwrap_err();
        assert!(
            matches!(
                e,
                Error::UnknownHarness { .. } | Error::UnknownPart { .. } | Error::File { .. }
            ),
            "{e}"
        );
        e.to_string()
    };
    assert_eq!(refused(&tree, "cursor", None), "no harness named `cursor`");
    assert_eq!(tree.files(), pristine);
    assert_eq!(
        refused(&tree, "claude", Some(&["hooks", "mcp2"])),
        "harness `claude` has no part named `mcp2`"
    );
    assert_eq!(tree.files(), pristine);
    for (file, text, message) in [
        (
            ".claude/settings.json",
            "{ \"permissions\": [ }",
            ".claude/settings.json: does not parse",
        ),
        (".mcp.json", "[]", ".mcp.json: is not a JSON object"),
        (
            ".claude/settings.json",
            "{\"hooks\": []}",
            "`hooks` is not an object",
        ),
        (".tool/harness.toml", "[claude\n", ".tool/harness.toml: "),
        (
            ".tool/config.toml",
            "[harness.claude]\nwithout = 1\n",
            ".tool/config.toml: ",
        ),
    ] {
        tree.write(file, text);
        let before = tree.files();
        let e = refused(&tree, "claude", None);
        assert!(e.contains(message), "{e}");
        assert_eq!(tree.files(), before, "{e}");
        std::fs::remove_file(tree.dir().join(file)).unwrap();
    }
    tree.write(".tool/config.toml", "harness = 1\n");
    assert!(status(&tree.tool(), ["claude"], Scope::Project).is_err());
    assert!(status(&tree.tool(), ["cursor"], Scope::Project).is_err());
    std::fs::remove_file(tree.dir().join(".tool/config.toml")).unwrap();
    std::fs::remove_dir(tree.dir().join(".tool")).unwrap();
    std::fs::remove_dir(tree.dir().join(".claude")).unwrap();
    assert_eq!(tree.files(), pristine);
}

// A skills part with one file missing (the tool added a skill) is not
// absent: an edited skill beside it is kept until forced; a skill added to
// a directory holding others' is created, not rewritten.
// @zen-test: KIT-3_AC-1
#[test]
fn a_new_skill_beside_an_edited_one_is_not_written_without_force() {
    let tree = TempTree::empty("skills-added");
    tree.write(".claude/skills/mine/SKILL.md", "theirs\n");
    let out = project(&tree, None, false);
    assert_eq!(parts(&out, "claude")[1].verb(), "created");
    tree.write(".claude/skills/tool/SKILL.md", "edited\n");
    // A second skill in the tool's integration is missing on disk.
    struct Two(common::TestTool);
    impl agent_harness_kit::Tool for Two {
        fn name(&self) -> &str {
            self.0.name()
        }
        fn harnesses(&self) -> Vec<std::sync::Arc<dyn agent_harness_kit::Harness>> {
            self.0.harnesses()
        }
        fn integration(&self, scope: Scope) -> agent_harness_kit::Integration {
            self.0
                .integration(scope)
                .skill(agent_harness_kit::integration::Skill::new(
                    "two", "Second.", "# Two\n",
                ))
        }
        fn root(&self, scope: Scope) -> Result<std::path::PathBuf> {
            self.0.root(scope)
        }
        fn record_path(&self, scope: Scope) -> Result<std::path::PathBuf> {
            self.0.record_path(scope)
        }
        fn declined_store(
            &self,
            scope: Scope,
        ) -> Result<Box<dyn agent_harness_kit::DeclinedStore + '_>> {
            self.0.declined_store(scope)
        }
    }
    let two = Two(tree.tool());
    let out = install(&two, &InstallOptions::new(["claude"], Scope::Project)).unwrap();
    assert_eq!(parts(&out, "claude")[1].verb(), "edited");
    assert_eq!(tree.read(".claude/skills/tool/SKILL.md"), "edited\n");
    assert!(!tree.exists(".claude/skills/two/SKILL.md"));
    // Unedited, the added skill makes the part stale and it is written.
    project(&tree, None, true);
    let out = install(&two, &InstallOptions::new(["claude"], Scope::Project)).unwrap();
    assert_eq!(parts(&out, "claude")[1].verb(), "rewrote");
    assert_eq!(parts(&out, "claude")[1].state, State::Stale);
    assert!(tree.exists(".claude/skills/two/SKILL.md"));
    assert_eq!(tree.read(".claude/skills/mine/SKILL.md"), "theirs\n");
}
