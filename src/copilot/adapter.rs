//! GitHub Copilot: the Copilot CLI, the VS Code agent and the cloud agent.
//!
//! Follows <https://docs.github.com/en/copilot/reference/hooks-configuration>,
//! <https://docs.github.com/copilot/customizing-copilot/adding-custom-instructions-for-github-copilot>,
//! <https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-config-dir-reference>,
//! <https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-mcp-servers>,
//! <https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/add-skills>,
//! <https://code.visualstudio.com/docs/copilot/customization/hooks> and
//! <https://code.visualstudio.com/docs/copilot/customization/mcp-servers>
//! (checked 2026-10-06).
// @zen-component: HAR-Copilot

use serde::Serialize;
use serde_json::{Map, Value, json, ser::PrettyFormatter};

use crate::{
    Result,
    common::{parts, protocol},
    harness::{Context, Harness, MergeOp, Part, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolCall, ToolKind},
    integration::{Hook, Integration, Item},
};

/// GitHub Copilot (`copilot`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Copilot;

const ID: &str = "copilot";
const LOCAL: &str = ".github/copilot/settings.local.json";

/// Copilot's name for `event`.
// @zen-impl: HAR-5_AC-3
fn event_name(event: Event) -> Option<&'static str> {
    Some(match event {
        Event::SessionStart => "sessionStart",
        Event::SessionEnd => "sessionEnd",
        Event::PromptSubmit => "userPromptSubmitted",
        Event::PreTool => "preToolUse",
        Event::PostTool => "postToolUse",
        Event::Stop => "agentStop",
        Event::PreCompact => "preCompact",
    })
}

/// Copilot's tool names are partly confirmed.
fn tool_kind(name: &str) -> ToolKind {
    match name {
        "bash" | "powershell" | "shell" => ToolKind::Shell,
        "view" | "read" | "grep" | "glob" => ToolKind::Read,
        "edit" | "create" | "write" => ToolKind::Write,
        n if n.starts_with("mcp") => ToolKind::Mcp,
        n => crate::claude::tool_kind(n),
    }
}

/// One hook entry: the command for bash and PowerShell.
fn entry(h: &Hook) -> Value {
    let command = h.command(ID);
    let mut e = json!({ "type": "command", "bash": command, "powershell": command });
    if let Some(t) = h.timeout_value() {
        e["timeoutSec"] = parts::seconds(t);
    }
    e
}

/// The tool's own hook file, `{"version": 1, "hooks": {<event>: [...]}}`,
/// events with hooks only, pretty-printed.
fn hook_file(integration: &Integration) -> String {
    let mut events = Map::new();
    for event in Event::ALL {
        let hooks: Vec<Value> = integration
            .hooks()
            .iter()
            .filter(|h| h.event() == event)
            .map(entry)
            .collect();
        if let (Some(name), false) = (event_name(event), hooks.is_empty()) {
            events.insert(name.to_string(), Value::Array(hooks));
        }
    }
    let doc = json!({ "version": 1, "hooks": events });
    let mut buf = Vec::new();
    let mut ser =
        serde_json::Serializer::with_formatter(&mut buf, PrettyFormatter::with_indent(b"  "));
    doc.serialize(&mut ser).expect("a JSON value serialises");
    let mut text = String::from_utf8(buf).expect("serde_json writes UTF-8");
    text.push('\n');
    text
}

/// The local settings' hook ops: per event, the tool's entries.
fn local_hook_ops(integration: &Integration) -> Result<Vec<MergeOp>> {
    let hooks: Vec<&Hook> = integration.hooks().iter().collect();
    let owned = parts::owner(integration, &hooks)?;
    Ok(Event::ALL
        .into_iter()
        .filter_map(|event| {
            let name = event_name(event)?;
            let entries = hooks
                .iter()
                .filter(|h| h.event() == event)
                .map(|h| entry(h))
                .collect();
            Some(MergeOp::owned_entries(
                ["hooks", name],
                "bash",
                owned.clone(),
                entries,
            ))
        })
        .collect())
}

fn location(item: Item, cx: &Context) -> Option<&'static str> {
    Some(match (item, cx.scope) {
        (Item::Hooks, Scope::Local) => LOCAL,
        (_, Scope::Local) => return None,
        (Item::Instructions, Scope::User) => ".copilot/copilot-instructions.md",
        (Item::Instructions, _) => "AGENTS.md",
        (Item::Skills, _) => ".agents/skills",
        (Item::Hooks, Scope::User) => ".copilot/hooks",
        (Item::Hooks, _) => ".github/hooks",
        (Item::Mcp, Scope::User) => ".copilot/mcp-config.json",
        (Item::Mcp, _) => ".mcp.json",
        (Item::Agents, Scope::User) => ".copilot/agents",
        (Item::Agents, _) => ".github/agents",
        // No file holds allowed commands; slash commands are prompt files,
        // deprecated.
        _ => return None,
    })
}

impl Harness for Copilot {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-5_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User, Scope::Local]
    }

    // @zen-impl: HAR-5_AC-2
    // @zen-impl: HAR-5_AC-6
    // @zen-impl: HAR-5_AC-7
    // @zen-impl: HAR-5_AC-8
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
                Item::Hooks if at == LOCAL => {
                    Part::merge(item.as_str(), at, local_hook_ops(integration)?)
                }
                Item::Hooks => Part::files(
                    item.as_str(),
                    at,
                    vec![(format!("{}.json", cx.tool), hook_file(integration))],
                ),
                Item::Mcp => parts::mcp(at, "mcpServers", integration, |s| s.to_json()),
                Item::Agents => parts::agents(at, ".agent.md", integration, |a| a.to_markdown(&[])),
                _ => continue,
            });
        }
        Ok(out)
    }

    // @zen-impl: HAR-5_AC-9
    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        let own = Reads::always(location(item, cx));
        Ok(match (item, cx.scope) {
            (Item::Instructions, Scope::Project) => {
                Reads::always(["AGENTS.md", "CLAUDE.md", ".github/copilot-instructions.md"])
            }
            (Item::Skills, Scope::Project) => {
                Reads::always([".agents/skills", ".github/skills", ".claude/skills"])
            }
            (Item::Skills, Scope::User) => {
                Reads::always([".agents/skills", ".copilot/skills"]).maybe([".claude/skills"])
            }
            (Item::Hooks, Scope::Project) => own.maybe([".claude/settings.json"]),
            (Item::Agents, Scope::Project) => own.maybe([".claude/agents"]),
            _ => own,
        })
    }

    // @zen-impl: HAR-5_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        // VS Code sends Claude-style fields; the CLI and the cloud agent
        // camelCase ones.
        let mut input = protocol::parse(ID, event, text, tool_kind, "last_assistant_message")?;
        let raw = input.raw.clone();
        let text = |k: &str| protocol::text(&raw, k);
        if let Some(s) = text("sessionId") {
            input.session_id = Some(s);
        }
        if let Some(name) = text("toolName") {
            let args = match raw.get("toolArgs") {
                Some(Value::String(s)) => {
                    serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone()))
                }
                Some(v) => v.clone(),
                None => Value::Null,
            };
            input.tool = Some(ToolCall::new(name.clone(), tool_kind(&name), args));
        }
        if let Some(r) = raw.get("toolResult") {
            input.tool_output = Some(r.clone());
        }
        Ok(input)
    }

    // @zen-impl: HAR-5_AC-5
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        Ok(match (answer, event) {
            (Answer::Allow { stderr }, _) => Output::json(json!({})).stderr(stderr.clone()),
            (Answer::Deny { reason }, Event::PreTool) => Output::json(json!({
                "permissionDecision": "deny", "permissionDecisionReason": reason
            })),
            (Answer::Continue { reason }, Event::Stop) => {
                Output::json(json!({ "decision": "block", "reason": reason }))
            }
            // Denying a prompt and adding context: not confirmed.
            _ => return Err(protocol::unsupported(ID, event, answer)),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{
        harness::Profile,
        integration::{Agent, Command, McpServer, Skill},
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
            .hook(
                Hook::new(Event::PreTool, "t hook {harness} {event}")
                    .timeout(Duration::from_secs(10)),
            )
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .allow_command("t")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check.\n"))
    }

    fn names(scope: Scope) -> Vec<String> {
        let cx = Context::new("t", scope, "/nowhere", None);
        Copilot
            .render(&full(), &cx)
            .unwrap()
            .iter()
            .map(|p| format!("{} {}", p.name(), p.location()))
            .collect()
    }

    // @zen-test: HAR-5_AC-1
    // @zen-test: HAR-5_AC-2
    // @zen-test: HAR-5_AC-3
    // @zen-test: HAR-5_AC-6
    // @zen-test: HAR-5_AC-7
    // @zen-test: HAR-5_AC-8
    // @zen-test: HAR-5_AC-9
    #[test]
    fn renders_every_item_per_scope() {
        assert_eq!(
            names(Scope::Project),
            [
                "instructions AGENTS.md",
                "skills .agents/skills",
                "hooks .github/hooks",
                "mcp .mcp.json",
                "agents .github/agents"
            ]
        );
        assert_eq!(
            names(Scope::User),
            [
                "instructions .copilot/copilot-instructions.md",
                "skills .agents/skills",
                "hooks .copilot/hooks",
                "mcp .copilot/mcp-config.json",
                "agents .copilot/agents"
            ]
        );
        assert_eq!(
            names(Scope::Local),
            ["hooks .github/copilot/settings.local.json"]
        );
        assert_eq!(
            hook_file(&full()),
            "{\n  \"version\": 1,\n  \"hooks\": {\n    \"preToolUse\": [\n      {\n        \"type\": \"command\",\n        \"bash\": \"t hook copilot pre-tool\",\n        \"powershell\": \"t hook copilot pre-tool\",\n        \"timeoutSec\": 10\n      }\n    ],\n    \"agentStop\": [\n      {\n        \"type\": \"command\",\n        \"bash\": \"t hook copilot stop\",\n        \"powershell\": \"t hook copilot stop\"\n      }\n    ]\n  }\n}\n"
        );
        let cx = Context::new("t", Scope::Local, "/nowhere", None);
        let p = Profile::new(ID, Copilot.render(&full(), &cx).unwrap());
        let ops = p.part("hooks").unwrap().ops();
        assert_eq!(ops.len(), 7);
        assert_eq!(ops[5].path(), ["hooks", "agentStop"]);
        let project = Context::new("t", Scope::Project, "/nowhere", None);
        assert!(
            Copilot
                .reads(Item::Instructions, &project)
                .unwrap()
                .loads("CLAUDE.md")
        );
        assert!(
            Copilot
                .reads(Item::Skills, &project)
                .unwrap()
                .loads(".claude/skills")
        );
        assert!(
            Copilot
                .reads(Item::Agents, &project)
                .unwrap()
                .may_load(".claude/agents")
        );
        assert!(
            Copilot
                .reads(Item::Hooks, &project)
                .unwrap()
                .may_load(".claude/settings.json")
        );
        let user = Context::new("t", Scope::User, "/nowhere", None);
        assert!(
            Copilot
                .reads(Item::Skills, &user)
                .unwrap()
                .loads(".copilot/skills")
        );
        assert_eq!(Copilot.scopes().len(), 3);
    }

    // @zen-test: HAR-5_AC-4
    #[test]
    fn input_in_either_casing() {
        let i = Copilot
            .parse_hook(
                Event::PreTool,
                r#"{"sessionId":"s","cwd":"/w","toolName":"bash","toolArgs":"{\"command\":\"ls\"}"}"#,
            )
            .unwrap();
        assert_eq!(i.session_id.as_deref(), Some("s"));
        let tool = i.tool.unwrap();
        assert_eq!(
            (tool.kind, tool.input),
            (ToolKind::Shell, json!({"command": "ls"}))
        );
        let i = Copilot
            .parse_hook(
                Event::PreTool,
                r#"{"toolName":"edit","toolArgs":{"p":1},"toolResult":"ok"}"#,
            )
            .unwrap();
        assert_eq!(i.tool.unwrap().input, json!({"p": 1}));
        assert_eq!(i.tool_output, Some(json!("ok")));
        let i = Copilot
            .parse_hook(
                Event::PreTool,
                r#"{"toolName":"view","toolArgs":"not json"}"#,
            )
            .unwrap();
        assert_eq!(i.tool.unwrap().input, json!("not json"));
        let i = Copilot
            .parse_hook(
                Event::Stop,
                r#"{"session_id":"v","stop_hook_active":true,"tool_name":"Bash"}"#,
            )
            .unwrap();
        assert_eq!((i.session_id.as_deref(), i.continuing), (Some("v"), true));
        assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
        assert_eq!(tool_kind("create"), ToolKind::Write);
        assert_eq!(tool_kind("mcp_x"), ToolKind::Mcp);
    }

    // @zen-test: HAR-5_AC-5
    #[test]
    fn answers() {
        let out = |e, a: Answer| Copilot.answer(e, &a).map(|o| o.stdout);
        assert_eq!(
            out(Event::PreTool, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"permissionDecision":"deny","permissionDecisionReason":"r"}"#
        );
        assert_eq!(
            out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
            r#"{"decision":"block","reason":"r"}"#
        );
        assert_eq!(
            out(Event::Stop, Answer::Allow { stderr: None }).unwrap(),
            "{}"
        );
        assert!(out(Event::PromptSubmit, Answer::Deny { reason: "r".into() }).is_err());
        assert!(out(Event::SessionStart, Answer::Context { text: "c".into() }).is_err());
    }
}
