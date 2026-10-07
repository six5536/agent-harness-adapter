//! Factory Droid.
//!
//! Follows <https://docs.factory.com/cli/configuration/agents-md>,
//! <https://docs.factory.com/reference/hooks-reference>,
//! <https://docs.factory.com/cli/configuration/settings>,
//! <https://docs.factory.com/cli/configuration/mcp>,
//! <https://docs.factory.com/cli/configuration/skills>,
//! <https://docs.factory.com/cli/configuration/custom-slash-commands> and
//! <https://docs.factory.com/harness/subagents> (checked 2026-10-06).
// @zen-component: HAR-Factory

use serde_json::{Value, json};

use crate::{
    Error, Result,
    common::{
        parts::{self, GroupHooks},
        protocol,
    },
    fs::read_text,
    harness::{Context, Harness, Part, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Integration, Item, McpServer, Transport},
};

/// Factory Droid (`factory`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Factory;

const ID: &str = "factory";
const HOOKS_FILE: &str = ".factory/hooks.json";
const SETTINGS: &str = ".factory/settings.json";

/// Droid's name for `event`: Claude Code's names.
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

fn matcher(kind: ToolKind) -> Option<&'static str> {
    match kind {
        ToolKind::Shell => Some("Execute"),
        ToolKind::Read => Some("Read|LS|Glob|Grep"),
        ToolKind::Write => Some("Edit|Create|ApplyPatch"),
        ToolKind::Mcp => Some("mcp__.*"),
        _ => None,
    }
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Execute" => ToolKind::Shell,
        "Read" | "LS" | "Glob" | "Grep" => ToolKind::Read,
        "Edit" | "Create" | "ApplyPatch" => ToolKind::Write,
        n if n.starts_with("mcp__") => ToolKind::Mcp,
        _ => ToolKind::Other,
    }
}

/// Where hooks go: `.factory/hooks.json` (events at the top) unless it is
/// absent and the settings file already holds a `hooks` key, which Droid
/// then reads instead.
// @zen-impl: HAR-3_AC-3
fn hooks_in_settings(cx: &Context) -> Result<bool> {
    if cx.is_file(HOOKS_FILE) {
        return Ok(false);
    }
    let Some(text) = read_text(&cx.path(SETTINGS))? else {
        return Ok(false);
    };
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| Error::file(SETTINGS, format!("does not parse as JSON: {e}")))?;
    Ok(doc.get("hooks").is_some())
}

const TOP: GroupHooks = GroupHooks {
    path: |e| vec![e.into()],
    event: event_name,
    matcher,
    timeout: parts::seconds,
    harness: ID,
};

const UNDER_HOOKS: GroupHooks = GroupHooks {
    path: |e| vec!["hooks".into(), e.into()],
    ..TOP
};

/// An MCP server with Droid's `type`.
// @zen-impl: HAR-3_AC-5
fn mcp_entry(s: &McpServer) -> Value {
    let mut v = json!({
        "type": match s.transport() {
            Transport::Stdio { .. } => "stdio",
            Transport::Http { .. } => "http",
        }
    });
    if let (Some(v), Some(m)) = (v.as_object_mut(), s.to_json().as_object()) {
        v.extend(m.clone());
    }
    v
}

fn location(item: Item, cx: &Context) -> Result<Option<&'static str>> {
    let user = cx.scope == Scope::User;
    Ok(Some(match item {
        Item::Instructions if user => ".factory/AGENTS.md",
        Item::Instructions => "AGENTS.md",
        Item::Skills if user => ".factory/skills",
        Item::Skills => ".agents/skills",
        Item::Hooks if hooks_in_settings(cx)? => SETTINGS,
        Item::Hooks => HOOKS_FILE,
        Item::Mcp => ".factory/mcp.json",
        Item::Agents => ".factory/droids",
        Item::Commands => ".factory/commands",
        // `permissionRules`: shape not confirmed.
        _ => return Ok(None),
    }))
}

impl Harness for Factory {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-3_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-3_AC-2
    // @zen-impl: HAR-3_AC-4
    // @zen-impl: HAR-3_AC-6
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        for item in integration.items() {
            let Some(at) = location(item, cx)? else {
                continue;
            };
            out.push(match item {
                Item::Instructions => {
                    parts::instructions(at, integration.instructions_block().unwrap_or_default())
                }
                Item::Skills => parts::skills(at, integration),
                Item::Hooks => {
                    let layout = if at == SETTINGS { &UNDER_HOOKS } else { &TOP };
                    Part::merge(item.as_str(), at, parts::group_hooks(layout, integration)?)
                }
                Item::Mcp => parts::mcp(at, "mcpServers", integration, mcp_entry),
                Item::Agents => parts::agents(at, ".md", integration, |a| {
                    a.to_markdown(&[("model", "inherit")])
                }),
                Item::Commands => parts::commands(at, ".md", integration, |c| c.to_markdown()),
                _ => continue,
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        let reads = Reads::always(location(item, cx)?);
        Ok(match (item, cx.scope) {
            // Not confirmed whether `CLAUDE.md` is read beside `AGENTS.md`.
            (Item::Instructions, Scope::Project) => reads.maybe(["CLAUDE.md"]),
            _ => reads,
        })
    }

    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        protocol::parse(ID, event, text, tool_kind, "last_assistant_message")
    }

    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        protocol::answer(ID, event, event_name(event).unwrap_or_default(), answer)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{
        harness::{MergeOp, Profile},
        integration::{Agent, Command, Hook, Skill},
        test_support::temp_dir,
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(Hook::new(Event::PreTool, "t hook {harness} {event}").tools(ToolKind::Write))
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .mcp_server(McpServer::http("w", "https://w"))
            .allow_command("t")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
    }

    fn names(p: &Profile) -> Vec<String> {
        p.parts()
            .iter()
            .map(|p| format!("{} {}", p.name(), p.location()))
            .collect()
    }

    // @zen-test: HAR-3_AC-1
    // @zen-test: HAR-3_AC-2
    // @zen-test: HAR-3_AC-3
    // @zen-test: HAR-3_AC-4
    // @zen-test: HAR-3_AC-5
    // @zen-test: HAR-3_AC-6
    #[test]
    fn renders_every_item_it_takes() {
        let dir = temp_dir("factory");
        let cx = Context::new("t", Scope::Project, &dir, None);
        let p = Profile::new(Factory.render(&full(), &cx).unwrap());
        assert_eq!(
            names(&p),
            [
                "instructions AGENTS.md",
                "skills .agents/skills",
                "hooks .factory/hooks.json",
                "mcp .factory/mcp.json",
                "agents .factory/droids",
                "commands .factory/commands"
            ]
        );
        let hooks = p.part("hooks").unwrap().ops();
        assert_eq!(
            hooks[3],
            MergeOp::group_entries(
                ["PreToolUse"],
                "hooks",
                "command",
                crate::harness::EntryMatch::Prefix("t hook ".into()),
                vec![
                    json!({ "matcher": "Edit|Create|ApplyPatch", "hooks": [{ "type": "command", "command": "t hook factory pre-tool" }] })
                ],
            )
        );
        assert_eq!(
            p.part("mcp").unwrap().ops()[1].value(),
            &json!({ "type": "http", "url": "https://w" })
        );
        assert_eq!(
            Factory.reads(Item::Instructions, &cx).unwrap(),
            Reads::always(["AGENTS.md"]).maybe(["CLAUDE.md"])
        );
        // Hooks already in the settings file stay there.
        fs::create_dir_all(dir.join(".factory")).unwrap();
        fs::write(dir.join(SETTINGS), r#"{"hooks": {}}"#).unwrap();
        let p = Profile::new(Factory.render(&full(), &cx).unwrap());
        assert_eq!(p.part("hooks").unwrap().location(), SETTINGS);
        assert_eq!(
            p.part("hooks").unwrap().ops()[0].path(),
            ["hooks", "SessionStart"]
        );
        // Unless hooks.json exists.
        fs::write(dir.join(HOOKS_FILE), "{}").unwrap();
        assert_eq!(
            Factory.reads(Item::Hooks, &cx).unwrap(),
            Reads::always([HOOKS_FILE])
        );
        fs::remove_file(dir.join(HOOKS_FILE)).unwrap();
        fs::write(dir.join(SETTINGS), "{ nope").unwrap();
        let e = Factory.render(&full(), &cx).unwrap_err();
        assert!(matches!(e, Error::File { .. }), "{e}");
        let user = Context::new("t", Scope::User, &dir, None);
        let p = Profile::new(
            Factory
                .render(
                    &Integration::new()
                        .instructions("x")
                        .skill(Skill::new("t", "d", "b")),
                    &user,
                )
                .unwrap(),
        );
        assert_eq!(
            names(&p),
            ["instructions .factory/AGENTS.md", "skills .factory/skills"]
        );
        assert_eq!(Factory.scopes(), [Scope::Project, Scope::User]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn input_and_answers_are_claudes() {
        let i = Factory
            .parse_hook(
                Event::PreTool,
                r#"{"tool_name":"Execute","tool_input":{"command":"ls"}}"#,
            )
            .unwrap();
        assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
        for (n, k) in [
            ("LS", ToolKind::Read),
            ("Create", ToolKind::Write),
            ("mcp__a__b", ToolKind::Mcp),
            ("Task", ToolKind::Other),
        ] {
            assert_eq!(tool_kind(n), k);
        }
        assert_eq!(
            Factory
                .answer(Event::PreTool, &Answer::Deny { reason: "r".into() })
                .unwrap()
                .stdout,
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"r"}}"#
        );
    }
}
