//! Cursor (the editor's agent and the Cursor CLI).
//!
//! Follows <https://cursor.com/docs/agent/hooks>,
//! <https://cursor.com/docs/context/rules>, <https://cursor.com/docs/context/mcp>,
//! <https://cursor.com/docs/context/skills>,
//! <https://cursor.com/docs/context/subagents>,
//! <https://cursor.com/docs/cli/reference/configuration>,
//! <https://cursor.com/docs/cli/reference/permissions> and
//! <https://cursor.com/docs/cli/changelog> (checked 2026-10-06).
// @zen-component: HAR-Cursor

use serde_json::{Value, json};

use crate::{
    Result,
    common::{parts, protocol},
    harness::{Context, Harness, MergeOp, Part, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolCall, ToolKind},
    integration::{Hook, Integration, Item},
};

/// Cursor (`cursor`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Cursor;

const ID: &str = "cursor";
const HOOKS: &str = ".cursor/hooks.json";

/// Cursor's name for `event`.
// @zen-impl: HAR-6_AC-3
fn event_name(event: Event) -> Option<&'static str> {
    Some(match event {
        Event::SessionStart => "sessionStart",
        Event::SessionEnd => "sessionEnd",
        Event::PromptSubmit => "beforeSubmitPrompt",
        Event::PreTool => "preToolUse",
        Event::PostTool => "postToolUse",
        Event::Stop => "stop",
        Event::PreCompact => "preCompact",
    })
}

/// Cursor's tool names are not confirmed: classify by what they say.
fn tool_kind(name: &str) -> ToolKind {
    let n = name.to_ascii_lowercase();
    if n.starts_with("mcp") {
        ToolKind::Mcp
    } else if n.contains("shell") || n.contains("terminal") {
        ToolKind::Shell
    } else if n.contains("edit") || n.contains("write") || n.contains("delete") {
        ToolKind::Write
    } else if n.contains("read") || n.contains("grep") || n.contains("search") || n.contains("glob")
    {
        ToolKind::Read
    } else {
        ToolKind::Other
    }
}

/// The hook ops: `"version": 1` and, per event, the tool's entries
/// `{"command", "timeout"?}`.
fn hook_ops(integration: &Integration) -> Result<Vec<MergeOp>> {
    let hooks: Vec<&Hook> = integration.hooks().iter().collect();
    let owned = parts::owner(integration, &hooks)?;
    let mut ops = vec![MergeOp::object_member(Vec::<String>::new(), "version", 1)];
    for event in Event::ALL {
        let Some(name) = event_name(event) else {
            continue;
        };
        let entries = hooks
            .iter()
            .filter(|h| h.event() == event)
            .map(|h| {
                let mut e = json!({ "command": h.command(ID) });
                if let Some(t) = h.timeout_value() {
                    e["timeout"] = parts::seconds(t);
                }
                e
            })
            .collect();
        ops.push(MergeOp::owned_entries(
            ["hooks", name],
            "command",
            owned.clone(),
            entries,
        ));
    }
    Ok(ops)
}

fn location(item: Item, cx: &Context) -> Option<&'static str> {
    let user = cx.scope == Scope::User;
    Some(match item {
        // User rules are not files.
        Item::Instructions if user => return None,
        Item::Instructions => "AGENTS.md",
        Item::Skills => ".agents/skills",
        Item::Hooks => HOOKS,
        Item::Mcp => ".cursor/mcp.json",
        Item::Permissions if user => ".cursor/cli-config.json",
        Item::Permissions => ".cursor/cli.json",
        Item::Agents => ".cursor/agents",
        Item::Commands => ".cursor/commands",
    })
}

impl Harness for Cursor {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-6_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-6_AC-2
    // @zen-impl: HAR-6_AC-6
    // @zen-impl: HAR-6_AC-7
    // @zen-impl: HAR-6_AC-8
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
                Item::Hooks => Part::merge(item.as_str(), at, hook_ops(integration)?),
                Item::Mcp => parts::mcp(at, "mcpServers", integration, |s| s.to_json()),
                Item::Permissions => Part::merge(
                    item.as_str(),
                    at,
                    integration
                        .allowed_commands()
                        .iter()
                        .map(|p| format!("Shell({})", p.split_whitespace().next().unwrap_or(p)))
                        .chain(
                            integration
                                .allowed_mcp_tools()
                                .iter()
                                .map(|(s, t)| format!("Mcp({s}:{t})")),
                        )
                        .map(|rule| MergeOp::array_entry(["permissions", "allow"], rule))
                        .collect(),
                ),
                Item::Agents => parts::agents(at, ".md", integration, |a| {
                    a.to_markdown(&[("model", "inherit")])
                }),
                Item::Commands => {
                    parts::commands(at, ".md", integration, |c| c.prompt().to_string())
                }
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        let own = Reads::always(location(item, cx));
        Ok(match item {
            // The Cursor CLI also reads CLAUDE.md, and runs Claude Code's
            // hooks.
            Item::Instructions if cx.scope == Scope::Project => own.maybe(["CLAUDE.md"]),
            Item::Hooks => own.maybe([".claude/settings.json"]),
            Item::Skills => Reads::always([".agents/skills", ".cursor/skills", ".claude/skills"]),
            Item::Agents => Reads::always([".cursor/agents", ".claude/agents"]),
            _ => own,
        })
    }

    // @zen-impl: HAR-6_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        let raw = protocol::raw_object(text)?;
        let mut input = HookInput::new(ID, event, raw.clone());
        let text = |k: &str| protocol::text(&raw, k);
        input.session_id = text("conversation_id").or_else(|| text("session_id"));
        input.cwd = raw["workspace_roots"][0]
            .as_str()
            .map(str::to_string)
            .or_else(|| text("cwd"));
        input.transcript_path = text("transcript_path");
        input.prompt = text("prompt");
        input.tool = text("tool_name").map(|name| {
            let kind = tool_kind(&name);
            ToolCall::new(
                name,
                kind,
                raw.get("tool_input").cloned().unwrap_or(Value::Null),
            )
        });
        input.tool_output = raw.get("tool_output").cloned();
        input.continuing = raw["loop_count"].as_u64().is_some_and(|n| n > 0);
        Ok(input)
    }

    // @zen-impl: HAR-6_AC-5
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        Ok(match (answer, event) {
            (Answer::Allow { stderr }, _) => Output::json(json!({})).stderr(stderr.clone()),
            (Answer::Deny { reason }, Event::PreTool) => Output::json(json!({
                "permission": "deny", "user_message": reason, "agent_message": reason
            })),
            (Answer::Deny { reason }, Event::PromptSubmit) => {
                Output::json(json!({ "continue": false, "user_message": reason }))
            }
            (Answer::Continue { reason }, Event::Stop) => {
                Output::json(json!({ "followup_message": reason }))
            }
            (Answer::Context { text }, Event::SessionStart) => {
                Output::json(json!({ "additional_context": text }))
            }
            _ => return Err(protocol::unsupported(ID, event, answer)),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        harness::{EntryMatch, Profile},
        integration::{Agent, Command, McpServer, Skill},
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(
                Hook::new(Event::Stop, "t hook {harness} {event}").timeout(Duration::from_secs(4)),
            )
            .hook(Hook::new(Event::PreTool, "t hook {harness} {event}").tools(ToolKind::Shell))
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .allow_command("t check")
            .allow_mcp_tool("t", "run")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
    }

    // @zen-test: HAR-6_AC-1
    // @zen-test: HAR-6_AC-2
    // @zen-test: HAR-6_AC-3
    // @zen-test: HAR-6_AC-6
    // @zen-test: HAR-6_AC-7
    // @zen-test: HAR-6_AC-8
    #[test]
    fn renders_every_item() {
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        let p = Profile::new(ID, Cursor.render(&full(), &cx).unwrap());
        let names: Vec<_> = p
            .parts()
            .iter()
            .map(|p| format!("{} {}", p.name(), p.location()))
            .collect();
        assert_eq!(
            names,
            [
                "instructions AGENTS.md",
                "skills .agents/skills",
                "hooks .cursor/hooks.json",
                "mcp .cursor/mcp.json",
                "permissions .cursor/cli.json",
                "agents .cursor/agents",
                "commands .cursor/commands"
            ]
        );
        let hooks = p.part("hooks").unwrap().ops();
        assert_eq!(hooks.len(), 8);
        assert_eq!(
            hooks[0],
            MergeOp::object_member(Vec::<String>::new(), "version", 1)
        );
        assert_eq!(
            hooks[4],
            MergeOp::owned_entries(
                ["hooks", "preToolUse"],
                "command",
                EntryMatch::Prefix("t hook ".into()),
                vec![json!({ "command": "t hook cursor pre-tool" })],
            )
        );
        assert_eq!(
            hooks[6].value(),
            &json!([{ "command": "t hook cursor stop", "timeout": 4 }])
        );
        assert_eq!(
            p.part("permissions").unwrap().ops(),
            &[
                MergeOp::array_entry(["permissions", "allow"], "Shell(t)"),
                MergeOp::array_entry(["permissions", "allow"], "Mcp(t:run)")
            ]
        );
        let user = Context::new("t", Scope::User, "/nowhere", None);
        let u = Profile::new(ID, Cursor.render(&full(), &user).unwrap());
        assert!(u.part("instructions").is_none());
        assert_eq!(
            u.part("permissions").unwrap().location(),
            ".cursor/cli-config.json"
        );
        assert_eq!(
            Cursor.reads(Item::Hooks, &cx).unwrap(),
            Reads::always([HOOKS]).maybe([".claude/settings.json"])
        );
        assert!(
            Cursor
                .reads(Item::Agents, &cx)
                .unwrap()
                .loads(".claude/agents")
        );
        assert!(
            Cursor
                .reads(Item::Skills, &cx)
                .unwrap()
                .loads(".claude/skills")
        );
        assert_eq!(
            Cursor.reads(Item::Instructions, &user).unwrap(),
            Reads::default()
        );
        assert_eq!(Cursor.scopes(), [Scope::Project, Scope::User]);
    }

    // @zen-test: HAR-6_AC-4
    #[test]
    fn input_is_read() {
        let i = Cursor
            .parse_hook(
                Event::Stop,
                r#"{"conversation_id":"c","workspace_roots":["/w"],"loop_count":1,"cursor_version":"2"}"#,
            )
            .unwrap();
        assert_eq!(
            (i.session_id.as_deref(), i.cwd.as_deref(), i.continuing),
            (Some("c"), Some("/w"), true)
        );
        let i = Cursor
            .parse_hook(
                Event::PreTool,
                r#"{"tool_name":"Shell","tool_input":{"command":"ls"},"loop_count":0}"#,
            )
            .unwrap();
        assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
        assert!(!i.continuing);
        for (n, k) in [
            ("run_terminal_cmd", ToolKind::Shell),
            ("Edit", ToolKind::Write),
            ("Read", ToolKind::Read),
            ("MCP:x", ToolKind::Mcp),
            ("Task", ToolKind::Other),
        ] {
            assert_eq!(tool_kind(n), k);
        }
    }

    // @zen-test: HAR-6_AC-5
    #[test]
    fn answers() {
        let out = |e, a: Answer| Cursor.answer(e, &a).map(|o| o.stdout);
        assert_eq!(
            out(Event::PreTool, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"permission":"deny","user_message":"r","agent_message":"r"}"#
        );
        assert_eq!(
            out(Event::PromptSubmit, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"continue":false,"user_message":"r"}"#
        );
        assert_eq!(
            out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
            r#"{"followup_message":"r"}"#
        );
        assert_eq!(
            out(Event::SessionStart, Answer::Context { text: "c".into() }).unwrap(),
            r#"{"additional_context":"c"}"#
        );
        assert_eq!(
            out(Event::Stop, Answer::Allow { stderr: None }).unwrap(),
            "{}"
        );
        assert!(out(Event::PromptSubmit, Answer::Context { text: "c".into() }).is_err());
    }
}
