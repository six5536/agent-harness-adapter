//! Pi (`@earendil-works/pi-coding-agent`).
//!
//! Follows `packages/coding-agent/docs/` of <https://github.com/earendil-works/pi>
//! (`extensions.md`, `configuration.md`, `settings.md`, `skills.md`,
//! `prompt-templates.md`, `mcp.md`, `security.md`, `session-format.md`) and
//! the package's `dist/core/extensions/types.d.ts`, version 1.0.4 (checked
//! 2026-10-07).
// @zen-component: HAR-Pi

use serde_json::{Value, json};

use crate::{
    Result,
    common::{parts, protocol},
    harness::{Context, Harness, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolCall, ToolKind},
    integration::{Integration, Item},
};

/// Pi (`pi`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pi;

const ID: &str = "pi";
const TEMPLATE: &str = include_str!("extension.ts");

/// The directory Pi's files are under at `scope`: the project's `.pi`, or
/// `~/.pi/agent`.
fn base(scope: Scope) -> &'static str {
    match scope {
        Scope::User => ".pi/agent",
        _ => ".pi",
    }
}

/// The instructions file: the first of `AGENTS.override.md`, `AGENTS.md`,
/// `CLAUDE.md` that exists in the directory, else `AGENTS.md`. Pi loads only
/// that one.
// @zen-impl: HAR-7_AC-2
fn instructions_file(cx: &Context) -> String {
    let dir = match cx.scope {
        Scope::User => ".pi/agent/",
        _ => "",
    };
    ["AGENTS.override.md", "AGENTS.md", "CLAUDE.md"]
        .iter()
        .map(|n| format!("{dir}{n}"))
        .find(|p| cx.is_file(p))
        .unwrap_or_else(|| format!("{dir}AGENTS.md"))
}

/// The tool name as it may appear in a comment: no line breaks.
fn comment_safe(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// The extension that runs the integration's hooks.
// @zen-impl: HAR-7_AC-3
fn extension(tool: &str, integration: &Integration) -> String {
    let hooks: Vec<Value> = integration
        .hooks()
        .iter()
        .map(|h| {
            let mut spec = json!({ "event": h.event().as_str(), "command": h.command(ID) });
            if let Some(t) = h.timeout_value() {
                spec["timeout"] = json!(u64::try_from(t.as_millis()).unwrap_or(u64::MAX));
            }
            spec
        })
        .collect();
    TEMPLATE
        .replace("__TOOL_COMMENT__", &comment_safe(tool))
        .replace("__TOOL__", &Value::String(tool.to_string()).to_string())
        .replace("__HOOKS__", &Value::Array(hooks).to_string())
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "bash" | "powershell" => ToolKind::Shell,
        "read" | "grep" | "find" | "ls" => ToolKind::Read,
        "edit" | "write" => ToolKind::Write,
        n if n.starts_with("mcp") => ToolKind::Mcp,
        _ => ToolKind::Other,
    }
}

fn location(item: Item, cx: &Context) -> Option<String> {
    let base = base(cx.scope);
    Some(match item {
        Item::Instructions => instructions_file(cx),
        Item::Skills => ".agents/skills".into(),
        Item::Hooks => format!("{base}/extensions"),
        Item::Mcp => format!("{base}/mcp.json"),
        Item::Commands => format!("{base}/prompts"),
        // No subagents; no allow-list of commands.
        _ => return None,
    })
}

impl Harness for Pi {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-7_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-7_AC-5
    // @zen-impl: HAR-7_AC-6
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        for item in integration.items() {
            let Some(at) = location(item, cx) else {
                continue;
            };
            out.push(match item {
                Item::Instructions => {
                    parts::instructions(&at, integration.instructions_block().unwrap_or_default())
                }
                Item::Skills => parts::skills(&at, integration),
                Item::Hooks => Part::files(
                    item.as_str(),
                    at,
                    vec![(format!("{}.ts", cx.tool), extension(&cx.tool, integration))],
                ),
                Item::Mcp => parts::mcp(&at, "mcpServers", integration, |s| s.to_json()),
                Item::Commands => parts::commands(&at, ".md", integration, |c| c.to_markdown()),
                _ => continue,
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(Reads::always(location(item, cx)))
    }

    // @zen-impl: HAR-7_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        let raw = protocol::raw_object(text)?;
        let mut input = HookInput::new(ID, event, raw.clone());
        let text = |k: &str| protocol::text(&raw, k);
        input.session_id = text("session_id");
        input.transcript_path = text("transcript_path");
        input.cwd = text("cwd");
        input.prompt = text("prompt");
        input.source = text("source");
        input.tool = text("tool_name").map(|name| {
            let kind = tool_kind(&name);
            ToolCall::new(
                name,
                kind,
                raw.get("tool_input").cloned().unwrap_or(Value::Null),
            )
        });
        input.tool_output = raw.get("tool_output").cloned();
        input.continuing = raw["continuing"].as_bool().unwrap_or(false);
        Ok(input)
    }

    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        Ok(match (answer, event) {
            (Answer::Allow { stderr }, _) => {
                Output::json(json!({ "answer": "allow" })).stderr(stderr.clone())
            }
            (Answer::Deny { reason }, Event::PreTool) => {
                Output::json(json!({ "answer": "deny", "reason": reason }))
            }
            (Answer::Continue { reason }, Event::Stop) => {
                Output::json(json!({ "answer": "continue", "reason": reason }))
            }
            (Answer::Context { text }, Event::PromptSubmit | Event::PostTool) => {
                Output::json(json!({ "answer": "context", "text": text }))
            }
            _ => return Err(protocol::unsupported(ID, event, answer)),
        })
    }

    // @zen-impl: HAR-7_AC-7
    fn notes(&self, cx: &Context, parts: &[PartResult]) -> Vec<String> {
        if !parts.iter().any(|p| p.action.is_some()) {
            return Vec::new();
        }
        let mut out = Vec::new();
        if cx.scope == Scope::Project {
            out.push("Pi loads the project's .pi files only once you trust the project".into());
        }
        out.push("run /reload in Pi to load the changes".into());
        out
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use super::*;
    use crate::{
        harness::{Action, Profile, State},
        integration::{Agent, Command, Hook, McpServer, Skill},
        test_support::temp_dir,
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
            .hook(
                Hook::new(Event::PreTool, "t hook {harness} {event}")
                    .timeout(Duration::from_secs(5)),
            )
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .allow_command("t")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
    }

    // @zen-test: HAR-7_AC-1
    // @zen-test: HAR-7_AC-3
    // @zen-test: HAR-7_AC-5
    // @zen-test: HAR-7_AC-6
    #[test]
    fn renders_every_item_per_scope() {
        let names = |scope| -> Vec<String> {
            let cx = Context::new("t", scope, "/nowhere", None);
            Pi.render(&full(), &cx)
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
                "hooks .pi/extensions",
                "mcp .pi/mcp.json",
                "commands .pi/prompts"
            ]
        );
        assert_eq!(
            names(Scope::User),
            [
                "instructions .pi/agent/AGENTS.md",
                "skills .agents/skills",
                "hooks .pi/agent/extensions",
                "mcp .pi/agent/mcp.json",
                "commands .pi/agent/prompts"
            ]
        );
        assert_eq!(Pi.scopes(), [Scope::Project, Scope::User]);
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        let p = Profile::new(Pi.render(&full(), &cx).unwrap());
        assert!(p.part("hooks").unwrap().same_content(&Part::files(
            "hooks",
            ".pi/extensions",
            vec![("t.ts".into(), extension("t", &full()))]
        )));
    }

    #[test]
    fn the_extension_carries_the_hooks() {
        let ts = extension("my\ntool", &full());
        assert!(
            ts.starts_with("// Written by my tool harness install"),
            "{ts}"
        );
        assert!(ts.contains("const TOOL: string = \"my\\ntool\";"), "{ts}");
        assert!(
            ts.contains(r#"const HOOKS: Spec[] = [{"event":"pre-tool","command":"t hook pi pre-tool","timeout":5000},{"event":"stop","command":"t hook pi stop"}];"#)
                || ts.contains(r#"const HOOKS: Spec[] = [{"event":"stop","command":"t hook pi stop"},{"event":"pre-tool","command":"t hook pi pre-tool","timeout":5000}];"#),
            "{ts}"
        );
        assert!(!ts.contains("__"), "every placeholder filled: {ts}");
    }

    // @zen-test: HAR-7_AC-2
    #[test]
    fn the_first_instructions_file_wins() {
        let dir = temp_dir("pi-rule");
        let cx = Context::new("t", Scope::Project, &dir, None);
        assert_eq!(instructions_file(&cx), "AGENTS.md");
        fs::write(dir.join("CLAUDE.md"), "c").unwrap();
        assert_eq!(instructions_file(&cx), "CLAUDE.md");
        fs::write(dir.join("AGENTS.md"), "a").unwrap();
        assert_eq!(instructions_file(&cx), "AGENTS.md");
        fs::write(dir.join("AGENTS.override.md"), "o").unwrap();
        assert_eq!(instructions_file(&cx), "AGENTS.override.md");
        assert_eq!(
            Pi.reads(Item::Instructions, &cx).unwrap(),
            Reads::always(["AGENTS.override.md"])
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: HAR-7_AC-4
    #[test]
    fn input_and_answers() {
        let i = Pi
            .parse_hook(
                Event::PreTool,
                r#"{"event":"pre-tool","session_id":"s","transcript_path":"/t","cwd":"/w","tool_name":"bash","tool_input":{"command":"ls"},"continuing":false}"#,
            )
            .unwrap();
        assert_eq!(i.cwd.as_deref(), Some("/w"));
        assert_eq!(
            (i.session_id.as_deref(), i.transcript_path.as_deref()),
            (Some("s"), Some("/t"))
        );
        assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
        assert!(
            Pi.parse_hook(Event::Stop, r#"{"continuing":true}"#)
                .unwrap()
                .continuing
        );
        for (n, k) in [
            ("grep", ToolKind::Read),
            ("edit", ToolKind::Write),
            ("mcp_x", ToolKind::Mcp),
            ("x", ToolKind::Other),
        ] {
            assert_eq!(tool_kind(n), k);
        }
        let out = |e, a: Answer| Pi.answer(e, &a).map(|o| o.stdout);
        assert_eq!(
            out(Event::Stop, Answer::Allow { stderr: None }).unwrap(),
            r#"{"answer":"allow"}"#
        );
        assert_eq!(
            out(Event::PreTool, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"answer":"deny","reason":"r"}"#
        );
        assert_eq!(
            out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
            r#"{"answer":"continue","reason":"r"}"#
        );
        assert_eq!(
            out(Event::PostTool, Answer::Context { text: "c".into() }).unwrap(),
            r#"{"answer":"context","text":"c"}"#
        );
        assert!(out(Event::SessionStart, Answer::Context { text: "c".into() }).is_err());
        assert!(out(Event::PromptSubmit, Answer::Deny { reason: "r".into() }).is_err());
    }

    // @zen-test: HAR-7_AC-7
    #[test]
    fn notes_after_writes() {
        let part = |action| PartResult {
            part: "hooks".into(),
            state: State::Absent,
            action,
            path: String::new(),
            by: None,
        };
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        assert!(Pi.notes(&cx, &[part(None)]).is_empty());
        assert_eq!(Pi.notes(&cx, &[part(Some(Action::Created))]).len(), 2);
        let user = Context::new("t", Scope::User, "/nowhere", None);
        assert_eq!(
            Pi.notes(&user, &[part(Some(Action::Created))]),
            ["run /reload in Pi to load the changes"]
        );
    }
}
