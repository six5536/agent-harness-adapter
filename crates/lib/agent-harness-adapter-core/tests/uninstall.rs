//! `uninstall` through the public API (KIT-22): install then uninstall
//! leaves the tree as it was, the user's content and edits stay, and what
//! another installed harness reads stays.

mod common;

use agent_harness_adapter_core::{
    Action, InstallOptions, InstallResult, Scope, State, UninstallOptions, harness, install,
    installed, uninstall,
};
use common::{TempTree, TestTool, parts};
use proptest::prelude::*;

fn tool(tree: &TempTree) -> TestTool {
    tree.tool().with(harness::builtin())
}

fn ids(scope: Scope) -> Vec<String> {
    harness::builtin()
        .iter()
        .filter(|h| h.scopes().contains(&scope))
        .map(|h| h.id().to_string())
        .collect()
}

fn put(tree: &TempTree, ids: &[String], scope: Scope) -> InstallResult {
    install(
        &tool(tree),
        &InstallOptions::new(ids.iter().cloned(), scope),
    )
    .unwrap()
}

fn take(tree: &TempTree, ids: &[&str], scope: Scope) -> InstallResult {
    uninstall(
        &tool(tree),
        &UninstallOptions::new(ids.iter().copied(), scope),
    )
    .unwrap()
}

/// The user's own files, in the library's styles, beside the tool's.
fn users_files(tree: &TempTree) {
    tree.write("CLAUDE.md", "# Mine\n\nMy notes.\n");
    tree.write("AGENTS.md", "# Agents\n");
    tree.write(
        ".claude/settings.json",
        "{\n  \"theme\": \"dark\",\n  \"permissions\": {\n    \"allow\": [\n      \"Bash(ls)\"\n    ]\n  }\n}\n",
    );
    tree.write(".codex/config.toml", "# mine\nmodel = \"x\"\n");
    tree.write(".claude/skills/mine/SKILL.md", "mine\n");
}

// @zen-test: KIT_P-12
// @zen-test: KIT-22_AC-1
// @zen-test: KIT-22_AC-2
// @zen-test: KIT-22_AC-6
#[test]
fn uninstall_undoes_install_for_every_harness_and_scope() {
    for scope in [Scope::Project, Scope::User, Scope::Local] {
        let all = ids(scope);
        for set in all.iter().map(|i| vec![i.clone()]).chain([all.clone()]) {
            for with_users in [false, true] {
                let tree = TempTree::empty("uninstall-roundtrip");
                if with_users {
                    users_files(&tree);
                }
                let before = tree.files();
                put(&tree, &set, scope);
                assert_ne!(tree.files(), before, "{scope} {set:?}: installed something");
                let names: Vec<&str> = set.iter().map(String::as_str).collect();
                let out = take(&tree, &names, scope);
                assert_eq!(tree.files(), before, "{scope} {set:?} users={with_users}");
                assert!(out.warnings.is_empty(), "{:?}", out.warnings);
                // Twice is once.
                let again = take(&tree, &names, scope);
                assert!(
                    again
                        .harnesses
                        .iter()
                        .flat_map(|h| &h.parts)
                        .all(|p| p.action.is_none())
                );
                assert_eq!(tree.files(), before);
            }
        }
    }
}

// @zen-test: KIT-22_AC-3
#[test]
fn an_edited_part_stays_unless_forced() {
    let tree = TempTree::empty("uninstall-edited");
    put(&tree, &["claude".into()], Scope::Project);
    let edited = "<!-- tool:harness -->\nMine now.\n<!-- /tool:harness -->\n";
    tree.write("CLAUDE.md", edited);
    let out = take(&tree, &["claude"], Scope::Project);
    let instructions = &parts(&out, "claude")[0];
    assert_eq!(
        (instructions.state, instructions.action),
        (State::Edited, None)
    );
    assert_eq!(tree.read("CLAUDE.md"), edited);
    assert!(!tree.exists(".claude/settings.json"), "the rest went");
    // The record no longer lists claude, so a second run sees the region as
    // the user's: with --force it goes.
    put(&tree, &["claude".into()], Scope::Project);
    tree.write("CLAUDE.md", edited);
    let out = uninstall(
        &tool(&tree),
        &UninstallOptions::new(["claude"], Scope::Project).force(true),
    )
    .unwrap();
    assert_eq!(parts(&out, "claude")[0].action, Some(Action::Removed));
    assert!(!tree.exists("CLAUDE.md"));
}

// @zen-test: KIT-22_AC-4
// @zen-test: KIT-22_AC-9
#[test]
fn what_another_installed_harness_reads_stays() {
    let tree = TempTree::empty("uninstall-shared");
    put(&tree, &["codex".into(), "pi".into()], Scope::Project);
    let out = take(&tree, &["codex"], Scope::Project);
    let kept: Vec<_> = parts(&out, "codex")
        .iter()
        .filter(|p| p.action == Some(Action::Kept))
        .map(|p| (p.part.as_str(), p.by.as_deref(), p.to_line()))
        .collect();
    assert!(
        kept.iter().any(|(part, by, line)| *part == "instructions"
            && *by == Some("pi")
            && line == "kept    AGENTS.md (instructions, used by pi)"),
        "{kept:?}"
    );
    assert!(tree.exists("AGENTS.md") && tree.exists(".agents/skills/tool/SKILL.md"));
    assert!(!tree.exists(".codex/hooks.json"), "codex's own files went");
    let tool = tool(&tree);
    assert_eq!(installed(&tool, Scope::Project).unwrap(), ["pi"]);
    // Then the last one: everything goes.
    take(&tree, &["pi"], Scope::Project);
    assert!(tree.files().is_empty(), "{:?}", tree.files().keys());
}

// @zen-test: KIT-22_AC-7
#[test]
fn all_is_the_installed_harnesses() {
    let tree = TempTree::empty("uninstall-all");
    put(&tree, &["gemini".into(), "cursor".into()], Scope::Project);
    let out = take(&tree, &["all"], Scope::Project);
    let named: Vec<&str> = out.harnesses.iter().map(|h| h.harness.as_str()).collect();
    assert_eq!(named, ["gemini", "cursor"]);
    assert!(tree.files().is_empty(), "{:?}", tree.files().keys());
    // Not installed: an entry with nothing removed. Unknown: refused.
    let out = take(&tree, &["codex"], Scope::Project);
    assert!(parts(&out, "codex").iter().all(|p| p.action.is_none()));
    assert!(tree.files().is_empty());
    let e = uninstall(
        &tool(&tree),
        &UninstallOptions::new(["vim"], Scope::Project),
    )
    .unwrap_err();
    assert_eq!(e.to_string(), "no harness named `vim`");
}

// @zen-test: KIT-22_AC-8
#[test]
fn a_refusal_writes_nothing() {
    let tree = TempTree::empty("uninstall-refusal");
    put(&tree, &["claude".into(), "codex".into()], Scope::Project);
    tree.write(".claude/settings.json", "{ broken");
    let before = tree.files();
    let e = uninstall(
        &tool(&tree),
        &UninstallOptions::new(["claude", "codex"], Scope::Project),
    )
    .unwrap_err();
    assert!(e.to_string().contains(".claude/settings.json"), "{e}");
    assert_eq!(tree.files(), before);
}

// @zen-test: KIT-22_AC-6
#[test]
fn declined_parts_are_cleared() {
    let tree = TempTree::empty("uninstall-declined");
    install(
        &tool(&tree),
        &InstallOptions::new(["claude"], Scope::Project).without(["hooks"]),
    )
    .unwrap();
    assert!(tree.read(".tool/config.toml").contains("hooks"));
    take(&tree, &["claude"], Scope::Project);
    assert!(!tree.read(".tool/config.toml").contains("hooks"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    // @zen-test: KIT_P-12
    #[test]
    fn any_install_then_uninstall_is_undone(
        picks in proptest::collection::vec(any::<bool>(), 8),
        users in any::<bool>(),
        scope in prop_oneof![Just(Scope::Project), Just(Scope::User), Just(Scope::Local)],
    ) {
        let all = ids(scope);
        let set: Vec<String> = all.iter().zip(&picks).filter(|(_, p)| **p).map(|(i, _)| i.clone()).collect();
        let tree = TempTree::empty("uninstall-prop");
        if users {
            users_files(&tree);
        }
        let before = tree.files();
        put(&tree, &set, scope);
        take(&tree, &["all"], scope);
        prop_assert_eq!(tree.files(), before);
    }
}

/// An external part its tool cannot take back out.
#[derive(Debug)]
struct Stuck(std::path::PathBuf);

impl agent_harness_adapter_core::ExternalPart for Stuck {
    fn location(&self) -> String {
        "stuck (user)".into()
    }
    fn expected(&self) -> String {
        "on".into()
    }
    fn observe(&self) -> agent_harness_adapter_core::Result<Option<String>> {
        Ok(std::fs::read_to_string(&self.0).ok())
    }
    fn write(&self) -> agent_harness_adapter_core::Result<()> {
        std::fs::write(&self.0, "on").map_err(|e| agent_harness_adapter_core::Error::io(&self.0, e))
    }
}

/// The test tool with the stuck part for Claude Code.
struct WithStuck(TestTool);

impl agent_harness_adapter_core::Tool for WithStuck {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn harnesses(&self) -> Vec<std::sync::Arc<dyn agent_harness_adapter_core::Harness>> {
        self.0.harnesses()
    }
    fn integration(&self, scope: Scope) -> agent_harness_adapter_core::Integration {
        self.0.integration(scope).part(
            "claude",
            agent_harness_adapter_core::Part::external(
                "stuck",
                std::sync::Arc::new(Stuck(self.0.dir.join("stuck"))),
            ),
        )
    }
    fn root(&self, scope: Scope) -> agent_harness_adapter_core::Result<std::path::PathBuf> {
        self.0.root(scope)
    }
    fn record_path(&self, scope: Scope) -> agent_harness_adapter_core::Result<std::path::PathBuf> {
        self.0.record_path(scope)
    }
    fn declined_store(
        &self,
        scope: Scope,
    ) -> agent_harness_adapter_core::Result<Box<dyn agent_harness_adapter_core::DeclinedStore + '_>>
    {
        self.0.declined_store(scope)
    }
}

// @zen-test: KIT-22_AC-5
#[test]
fn an_external_part_the_tool_cannot_remove_stays_with_a_warning() {
    let tree = TempTree::empty("uninstall-stuck");
    let t = WithStuck(tool(&tree));
    install(&t, &InstallOptions::new(["claude"], Scope::Project)).unwrap();
    assert_eq!(tree.read("stuck"), "on");
    let out = uninstall(&t, &UninstallOptions::new(["claude"], Scope::Project)).unwrap();
    assert_eq!(tree.read("stuck"), "on");
    assert_eq!(
        out.warnings,
        ["claude: stuck (user) (stuck) is left: the tool cannot remove it"]
    );
    let stuck = parts(&out, "claude")
        .iter()
        .find(|p| p.part == "stuck")
        .unwrap();
    assert_eq!(stuck.action, None);
}
