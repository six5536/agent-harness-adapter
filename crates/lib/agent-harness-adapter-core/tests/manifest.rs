//! A manifest installs exactly what the same integration built in Rust does.

mod common;

use std::{path::PathBuf, sync::Arc, time::Duration};

use agent_harness_adapter_core::{
    DeclinedStore, Harness, InstallOptions, Integration, Part, Result, Scope, TomlDeclined, Tool,
    harness,
    hook::{Event, ToolKind},
    install,
    integration::{Agent, Command, Hook, McpServer, Skill},
    manifest::{Manifest, ManifestTool},
};
use common::TempTree;

const MANIFEST: &str = r##"
version = 1
name = "mytool"
instructions = { file = "instructions.md" }
allow_commands = ["mytool"]
allow_mcp_tools = [{ server = "mytool", tool = "run" }]

[[skills]]
name = "mytool"
description = "Use mytool: always."
body = { file = "skill.md" }
files = { "ref.md" = "ref\n" }

[[hooks]]
event = "session-start"
run = "mytool context"

[[hooks]]
event = "pre-tool"
run = "mytool guard"
tools = "shell"
timeout = 5

[[hooks]]
event = "stop"
command = "mytool hook {harness} {event}"

[[mcp_servers]]
name = "mytool"
command = "mytool"
args = ["mcp"]
env = { MYTOOL = "1" }

[[agents]]
name = "reviewer"
description = "Reviews."
prompt = "You review.\n"

[[commands]]
name = "check"
description = "Run the check."
prompt = "Check $ARGUMENTS.\n"

[[parts]]
harness = "claude"
kind = "region"
name = "notes"
file = "NOTES.md"
block = "Notes.\n"
"##;

fn built() -> Integration {
    Integration::new()
        .instructions("Use mytool.\n")
        .allow_command("mytool")
        .allow_mcp_tool("mytool", "run")
        .skill(Skill::new("mytool", "Use mytool: always.", "# Mytool\n").file("ref.md", "ref\n"))
        .hook(Hook::new(
            Event::SessionStart,
            "agent-harness-adapter hook --tool mytool {harness} {event} -- mytool context",
        ))
        .hook(
            Hook::new(
                Event::PreTool,
                "agent-harness-adapter hook --tool mytool --tools shell {harness} {event} -- mytool guard",
            )
            .tools(ToolKind::Shell)
            .timeout(Duration::from_secs(5)),
        )
        .hook(Hook::new(Event::Stop, "mytool hook {harness} {event}"))
        .mcp_server(McpServer::stdio("mytool", "mytool", ["mcp"]).env("MYTOOL", "1"))
        .agent(Agent::new("reviewer", "Reviews.", "You review.\n"))
        .command(Command::new(
            "check",
            "Run the check.",
            "Check $ARGUMENTS.\n",
        ))
        .part("claude", Part::region("notes", "NOTES.md", "Notes.\n"))
}

/// The same tool written in Rust, with the manifest's default paths.
struct Built(PathBuf);

impl Tool for Built {
    fn name(&self) -> &str {
        "mytool"
    }
    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        harness::builtin()
    }
    fn integration(&self, _: Scope) -> Integration {
        built()
    }
    fn root(&self, scope: Scope) -> Result<PathBuf> {
        Ok(match scope {
            Scope::User => self.0.join("home"),
            _ => self.0.clone(),
        })
    }
    fn record_path(&self, scope: Scope) -> Result<PathBuf> {
        let file = if scope == Scope::Local {
            "harness.local.toml"
        } else {
            "harness.toml"
        };
        Ok(self.root(scope)?.join(".mytool").join(file))
    }
    fn declined_store(&self, scope: Scope) -> Result<Box<dyn DeclinedStore + '_>> {
        Ok(Box::new(TomlDeclined::new(
            self.root(scope)?.join(".mytool/config.toml"),
            ".mytool/config.toml",
        )))
    }
}

// @zen-test: AHA_P-1
// @zen-test: AHA-1_AC-4
#[test]
fn a_manifest_installs_what_the_builder_does_for_every_harness_and_scope() {
    let src = TempTree::empty("manifest-src");
    src.write("mytool.harness.toml", MANIFEST);
    src.write("instructions.md", "Use mytool.\n");
    src.write("skill.md", "# Mytool\n");
    let manifest = Manifest::load(&src.dir().join("mytool.harness.toml")).unwrap();
    for scope in [Scope::Project, Scope::User, Scope::Local] {
        let ids: Vec<String> = harness::builtin()
            .iter()
            .filter(|h| h.scopes().contains(&scope))
            .map(|h| h.id().to_string())
            .collect();
        for id in ids.iter().map(|i| vec![i.clone()]).chain([ids.clone()]) {
            let a = TempTree::empty("manifest-a");
            let b = TempTree::empty("manifest-b");
            let opts = InstallOptions::new(&id, scope);
            let tool = ManifestTool::new(manifest.clone(), a.dir(), a.dir().join("home"));
            let from_manifest = install(&tool, &opts).unwrap();
            let from_builder = install(&Built(b.dir().to_path_buf()), &opts).unwrap();
            let label = format!("{scope} {id:?}");
            assert_eq!(a.files(), b.files(), "{label}");
            assert!(!a.files().is_empty(), "{label}");
            assert_eq!(from_manifest.to_text(), from_builder.to_text(), "{label}");
        }
    }
}
