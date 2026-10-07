//! Kilo Code (`@kilocode/cli`; the VS Code extension runs the same agent).
//!
//! Follows `packages/kilo-docs/pages/` of <https://github.com/Kilo-Org/kilocode>
//! (`automate/extending/plugins.md`, `automate/mcp/using-in-kilo-code.md`,
//! `customize/agent-permissions.md`, `customize/custom-subagents.md`) and
//! its source (`config/paths.ts`, `config/plugin.ts`,
//! `kilocode/config/config.ts`, `session/instruction.ts`, `skill/index.ts`,
//! `cli/cmd/run.ts`), version 7.8.7 (checked 2026-10-07).
// @zen-component: HAR-Kilo

use crate::{
    Result,
    common::{extension, plugin},
    harness::{Context, Harness, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output},
    integration::{Integration, Item},
};

/// Kilo Code (`kilo`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Kilo;

const ID: &str = "kilo";

/// Kilo's paths: the project's `.kilo` and `kilo.json`, or `~/.config/kilo`
/// and its `kilo.json`; it also reads an `opencode.json` beside them (not
/// `.opencode`). The instructions file is the first of `AGENTS.md`,
/// `CLAUDE.md` that exists (`.config/kilo/AGENTS.md`, `.claude/CLAUDE.md` at
/// user scope), else the first; Kilo loads only that one.
// @zen-impl: HAR-12_AC-1
// @zen-impl: HAR-12_AC-2
const LAYOUT: plugin::Layout = plugin::Layout {
    harness: ID,
    agent: "Kilo Code",
    types: "@kilocode/plugin",
    project_dir: ".kilo",
    user_dir: ".config/kilo",
    config: "kilo.json",
    also_config: &["opencode.json"],
};

impl Harness for Kilo {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-12_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-12_AC-3
    // @zen-impl: HAR-12_AC-5
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        LAYOUT.render(integration, cx)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(LAYOUT.reads(item, cx))
    }

    // @zen-impl: HAR-12_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        extension::parse(ID, event, text, plugin::tool_kind)
    }

    // @zen-impl: HAR-12_AC-4
    // @zen-impl: AHA-2_AC-3
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        extension::answer(ID, event, answer)
    }

    // @zen-impl: HAR-12_AC-6
    fn notes(&self, _cx: &Context, parts: &[PartResult]) -> Vec<String> {
        if parts.iter().any(PartResult::changed) {
            vec!["restart Kilo Code to load the changes".into()]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        harness::{Action, State},
        integration::{Agent, Command, Hook, McpServer, Skill},
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .allow_command("t")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
    }

    // @zen-test: HAR-12_AC-1
    // @zen-test: HAR-12_AC-2
    // @zen-test: HAR-12_AC-3
    // @zen-test: HAR-12_AC-5
    #[test]
    fn renders_into_kilos_paths() {
        let names = |scope| -> Vec<String> {
            let cx = Context::new("t", scope, "/nowhere", None);
            Kilo.render(&full(), &cx)
                .unwrap()
                .iter()
                .map(|p| format!("{} {}", p.name(), p.location()))
                .collect()
        };
        assert_eq!(
            names(Scope::Project),
            [
                "instructions AGENTS.md",
                "skills .agents/skills",
                "hooks .kilo/plugins",
                "mcp kilo.json",
                "permissions kilo.json",
                "agents .kilo/agents",
                "commands .kilo/commands"
            ]
        );
        assert_eq!(
            names(Scope::User),
            [
                "instructions .config/kilo/AGENTS.md",
                "skills .agents/skills",
                "hooks .config/kilo/plugins",
                "mcp .config/kilo/kilo.json",
                "permissions .config/kilo/kilo.json",
                "agents .config/kilo/agents",
                "commands .config/kilo/commands"
            ]
        );
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        assert_eq!(
            Kilo.reads(Item::Mcp, &cx).unwrap(),
            Reads::always(["kilo.json", "opencode.json"])
        );
        let ts = plugin::plugin(ID, "Kilo Code", "@kilocode/plugin", "t", &full());
        assert!(ts.contains("\"t hook kilo stop\""), "{ts}");
        assert_eq!(Kilo.scopes(), [Scope::Project, Scope::User]);
    }

    // @zen-test: HAR-12_AC-4
    // @zen-test: HAR-12_AC-6
    #[test]
    fn hooks_and_notes() {
        let i = Kilo
            .parse_hook(Event::PreTool, r#"{"tool_name":"bash","session_id":"s"}"#)
            .unwrap();
        assert_eq!(i.harness, "kilo");
        let deny = Kilo
            .answer(Event::PreTool, &Answer::Deny { reason: "r".into() })
            .unwrap();
        assert_eq!(deny.stdout, r#"{"answer":"deny","reason":"r"}"#);
        let part = |action| PartResult {
            part: "hooks".into(),
            state: State::Absent,
            action,
            path: String::new(),
            by: None,
        };
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        assert!(Kilo.notes(&cx, &[part(None)]).is_empty());
        assert_eq!(
            Kilo.notes(&cx, &[part(Some(Action::Created))]),
            ["restart Kilo Code to load the changes"]
        );
    }
}
