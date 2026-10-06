//! OpenAI Codex CLI.
//!
//! Follows <https://learn.chatgpt.com/docs/agent-configuration/agents-md>,
//! <https://learn.chatgpt.com/docs/hooks>,
//! <https://learn.chatgpt.com/docs/config-file/config-basic>,
//! <https://learn.chatgpt.com/docs/extend/mcp>,
//! <https://learn.chatgpt.com/docs/build-skills> and
//! <https://learn.chatgpt.com/docs/agent-configuration/subagents>
//! (checked 2026-10-06).
// @zen-component: HAR-Codex

use serde_json::{Value, json};

use crate::{
    Result,
    common::{
        parts::{self, GroupHooks},
        protocol, toml_out,
    },
    harness::{Context, Harness, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Integration, Item, McpServer, Transport},
};

/// OpenAI Codex CLI (`codex`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Codex;

const ID: &str = "codex";

/// Codex's name for `event`: Claude Code's names.
// @zen-impl: HAR-2_AC-3
fn event_name(event: Event) -> Option<&'static str> {
    Some(match event {
        Event::SessionStart => "SessionStart",
        Event::SessionEnd => "SessionEnd",
        Event::PromptSubmit => "UserPromptSubmit",
        Event::PreTool => "PreToolUse",
        Event::PostTool => "PostToolUse",
        Event::Stop => "Stop",
        Event::PreCompact => "PreCompact",
    })
}

/// Only the shell tool's name is confirmed.
fn matcher(kind: ToolKind) -> Option<&'static str> {
    (kind == ToolKind::Shell).then_some("^Bash$")
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Bash" | "shell" | "local_shell" => ToolKind::Shell,
        "apply_patch" => ToolKind::Write,
        n if n.starts_with("mcp__") => ToolKind::Mcp,
        _ => ToolKind::Other,
    }
}

const LAYOUT: GroupHooks = GroupHooks {
    path: |e| vec!["hooks".into(), e.into()],
    event: event_name,
    matcher,
    timeout: parts::seconds,
    harness: ID,
};

/// An MCP server as a `[mcp_servers.<name>]` table.
// @zen-impl: HAR-2_AC-5
fn mcp_table(s: &McpServer) -> Value {
    match s.transport() {
        Transport::Stdio { .. } => s.to_json(),
        Transport::Http { url, headers } => {
            let mut v = json!({ "url": url });
            if !headers.is_empty() {
                v["http_headers"] = s.to_json()["headers"].clone();
            }
            v
        }
    }
}

fn location(item: Item, cx: &Context) -> Option<&'static str> {
    let user = cx.scope == Scope::User;
    Some(match item {
        Item::Instructions if user => ".codex/AGENTS.md",
        Item::Instructions => "AGENTS.md",
        Item::Skills => ".agents/skills",
        Item::Hooks => ".codex/hooks.json",
        Item::Mcp => ".codex/config.toml",
        Item::Agents => ".codex/agents",
        // Allowed commands: rule-file syntax not confirmed. Commands:
        // deprecated in Codex for skills.
        _ => return None,
    })
}

impl Harness for Codex {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-2_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-2_AC-2
    // @zen-impl: HAR-2_AC-4
    // @zen-impl: HAR-2_AC-6
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        for item in integration.items() {
            let Some(at) = location(item, cx) else {
                continue;
            };
            out.push(match item {
                Item::Instructions => {
                    parts::instructions(at, integration.instructions_block().unwrap_or_default())
                }
                Item::Skills => parts::skills(at, integration),
                Item::Hooks => {
                    Part::merge(item.as_str(), at, parts::group_hooks(&LAYOUT, integration)?)
                }
                Item::Mcp => parts::mcp(at, "mcp_servers", integration, mcp_table),
                Item::Agents => parts::agents(at, ".toml", integration, |a| {
                    toml_out::strings(&[
                        ("name", a.name()),
                        ("description", a.description()),
                        ("developer_instructions", a.prompt()),
                    ])
                }),
                _ => continue,
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(Reads::always(location(item, cx)))
    }

    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        protocol::parse(ID, event, text, tool_kind, "last_assistant_message")
    }

    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        protocol::answer(ID, event, event_name(event).unwrap_or_default(), answer)
    }

    // @zen-impl: HAR-2_AC-7
    fn notes(&self, cx: &Context, parts: &[PartResult]) -> Vec<String> {
        let written = |name: &str| parts.iter().any(|p| p.part == name && p.action.is_some());
        let mut out = Vec::new();
        if !parts.iter().any(|p| p.action.is_some()) {
            return out;
        }
        if cx.scope == Scope::Project {
            out.push(
                "Codex reads the project's .codex files only once you trust the project".into(),
            );
        }
        if written("hooks") {
            out.push("new hooks run only once approved in Codex's /hooks".into());
        }
        out.push("restart Codex to load the changes".into());
        out
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        harness::{Action, MergeOp, Profile, State},
        integration::{Agent, Command, Hook, Skill},
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
            .hook(
                Hook::new(Event::PreTool, "t hook {harness} {event}")
                    .tools(ToolKind::Shell)
                    .timeout(Duration::from_secs(9)),
            )
            .hook(Hook::new(Event::PostTool, "t hook {harness} {event}").tools(ToolKind::Write))
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .mcp_server(McpServer::http("w", "https://w").header("A", "b"))
            .allow_command("t")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check.\n"))
    }

    fn rendered(scope: Scope) -> Profile {
        let cx = Context::new("t", scope, "/nowhere", None);
        Profile::new(ID, Codex.render(&full(), &cx).unwrap())
    }

    // @zen-test: HAR-2_AC-1
    // @zen-test: HAR-2_AC-2
    // @zen-test: HAR-2_AC-3
    // @zen-test: HAR-2_AC-4
    // @zen-test: HAR-2_AC-5
    // @zen-test: HAR-2_AC-6
    #[test]
    fn renders_every_item_it_takes() {
        let names = |s| -> Vec<String> {
            rendered(s)
                .parts()
                .iter()
                .map(|p| format!("{} {}", p.name(), p.location()))
                .collect()
        };
        assert_eq!(
            names(Scope::Project),
            [
                "instructions AGENTS.md",
                "skills .agents/skills",
                "hooks .codex/hooks.json",
                "mcp .codex/config.toml",
                "agents .codex/agents"
            ]
        );
        assert_eq!(names(Scope::User)[0], "instructions .codex/AGENTS.md");
        assert_eq!(Codex.scopes(), [Scope::Project, Scope::User]);
        let p = rendered(Scope::Project);
        let hooks = p.part("hooks").unwrap().ops();
        assert_eq!(hooks.len(), 7);
        assert_eq!(
            hooks[3].value(),
            &json!([{ "matcher": "^Bash$", "hooks": [{ "type": "command", "command": "t hook codex pre-tool", "timeout": 9 }] }])
        );
        // No confirmed matcher for writes: every tool.
        assert_eq!(
            hooks[4].value(),
            &json!([{ "hooks": [{ "type": "command", "command": "t hook codex post-tool" }] }])
        );
        assert_eq!(
            p.part("mcp").unwrap().ops(),
            &[
                MergeOp::object_member(
                    ["mcp_servers"],
                    "t",
                    json!({ "command": "t", "args": ["mcp"] })
                ),
                MergeOp::object_member(
                    ["mcp_servers"],
                    "w",
                    json!({ "url": "https://w", "http_headers": { "A": "b" } })
                ),
            ]
        );
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        assert_eq!(
            Codex.reads(Item::Permissions, &cx).unwrap(),
            Reads::default()
        );
        assert_eq!(
            Codex.reads(Item::Skills, &cx).unwrap(),
            Reads::always([".agents/skills"])
        );
    }

    #[test]
    fn agents_are_toml_files() {
        let i = Integration::new().agent(Agent::new("r", "Reviews.", "Review.\n"));
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        let parts = Codex.render(&i, &cx).unwrap();
        assert_eq!(parts[0].location(), ".codex/agents");
        assert!(parts[0].same_content(&Part::files(
            "agents",
            ".codex/agents",
            vec![(
                "r.toml".into(),
                "name = \"r\"\ndescription = \"Reviews.\"\ndeveloper_instructions = \"\"\"\nReview.\n\"\"\"\n".into()
            )]
        )));
    }

    #[test]
    fn input_and_answers_are_claudes() {
        let i = Codex
            .parse_hook(
                Event::PreTool,
                r#"{"tool_name":"apply_patch","tool_input":{}}"#,
            )
            .unwrap();
        assert_eq!(
            (i.harness.as_str(), i.tool.unwrap().kind),
            ("codex", ToolKind::Write)
        );
        assert_eq!(tool_kind("shell"), ToolKind::Shell);
        assert_eq!(tool_kind("mcp__a__b"), ToolKind::Mcp);
        assert_eq!(tool_kind("x"), ToolKind::Other);
        assert_eq!(
            Codex
                .answer(Event::Stop, &Answer::Continue { reason: "r".into() })
                .unwrap()
                .stdout,
            r#"{"decision":"block","reason":"r"}"#
        );
    }

    // @zen-test: HAR-2_AC-7
    #[test]
    fn notes_after_writes() {
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        let part = |name: &str, action| PartResult {
            part: name.into(),
            state: State::Absent,
            action,
            path: String::new(),
            by: None,
        };
        assert!(Codex.notes(&cx, &[part("hooks", None)]).is_empty());
        assert_eq!(
            Codex.notes(&cx, &[part("hooks", Some(Action::Created))]),
            [
                "Codex reads the project's .codex files only once you trust the project",
                "new hooks run only once approved in Codex's /hooks",
                "restart Codex to load the changes"
            ]
        );
        let user = Context::new("t", Scope::User, "/nowhere", None);
        assert_eq!(
            Codex.notes(&user, &[part("skills", Some(Action::Created))]),
            ["restart Codex to load the changes"]
        );
    }
}
