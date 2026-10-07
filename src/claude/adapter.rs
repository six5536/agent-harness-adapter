//! Claude Code.
//!
//! Follows <https://code.claude.com/docs/en/hooks>,
//! <https://code.claude.com/docs/en/settings>,
//! <https://code.claude.com/docs/en/memory>, <https://code.claude.com/docs/en/skills>,
//! <https://code.claude.com/docs/en/sub-agents>, <https://code.claude.com/docs/en/slash-commands>
//! and <https://code.claude.com/docs/en/mcp>.
// @zen-component: HAR-Claude

use std::path::Path;

use crate::{
    Error, Result,
    common::{
        parts::{self, GroupHooks},
        protocol,
    },
    harness::{Context, Harness, MergeOp, Part, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Integration, Item},
};

/// Claude Code (`claude`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Claude;

const ID: &str = "claude";

/// The instructions file under `root`: `AGENTS.md` when it exists and
/// `CLAUDE.md` is absent or has a line `@AGENTS.md`; otherwise `CLAUDE.md`,
/// created when absent.
// @zen-impl: HAR-1_AC-2
pub fn instructions_file(root: &Path) -> Result<&'static str> {
    let agents = root.join("AGENTS.md");
    let claude = root.join("CLAUDE.md");
    if !agents.is_file() {
        return Ok("CLAUDE.md");
    }
    if !claude.is_file() {
        return Ok("AGENTS.md");
    }
    let text = std::fs::read_to_string(&claude).map_err(|e| Error::io(&claude, e))?;
    Ok(if text.lines().any(|l| l.trim() == "@AGENTS.md") {
        "AGENTS.md"
    } else {
        "CLAUDE.md"
    })
}

/// Claude Code's name for `event`.
// @zen-impl: HAR-1_AC-3
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
        ToolKind::Shell => Some("Bash"),
        ToolKind::Read => Some("Read|Grep|Glob|LS"),
        ToolKind::Write => Some("Edit|Write|MultiEdit|NotebookEdit"),
        ToolKind::Mcp => Some("mcp__.*"),
        _ => None,
    }
}

/// The kind of a Claude Code tool.
pub(crate) fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Bash" => ToolKind::Shell,
        "Read" | "Grep" | "Glob" | "LS" => ToolKind::Read,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => ToolKind::Write,
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

/// The settings file of `scope`.
fn settings(scope: Scope) -> &'static str {
    match scope {
        Scope::Local => ".claude/settings.local.json",
        _ => ".claude/settings.json",
    }
}

/// Where the item goes at `cx`; `None` when Claude Code takes none there.
fn location(item: Item, cx: &Context) -> Result<Option<String>> {
    let local = cx.scope == Scope::Local;
    Ok(match item {
        Item::Instructions => match cx.scope {
            Scope::Project => Some(instructions_file(&cx.root)?.to_string()),
            Scope::User => Some(".claude/CLAUDE.md".into()),
            _ => None,
        },
        Item::Skills if !local => Some(".claude/skills".into()),
        Item::Agents if !local => Some(".claude/agents".into()),
        Item::Commands if !local => Some(".claude/commands".into()),
        // The user's MCP servers live in ~/.claude.json, Claude Code's own
        // state file.
        Item::Mcp if cx.scope == Scope::Project => Some(".mcp.json".into()),
        Item::Hooks | Item::Permissions => Some(settings(cx.scope).into()),
        _ => None,
    })
}

impl Harness for Claude {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-1_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User, Scope::Local]
    }

    // @zen-impl: HAR-1_AC-6
    // @zen-impl: HAR-1_AC-7
    // @zen-impl: HAR-1_AC-8
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        for item in integration.items() {
            let Some(at) = location(item, cx)? else {
                continue;
            };
            out.push(match item {
                Item::Instructions => {
                    parts::instructions(&at, integration.instructions_block().unwrap_or_default())
                }
                Item::Skills => parts::skills(&at, integration),
                Item::Hooks => {
                    Part::merge(item.as_str(), at, parts::group_hooks(&LAYOUT, integration)?)
                }
                Item::Mcp => parts::mcp(&at, "mcpServers", integration, |s| s.to_json()),
                Item::Permissions => Part::merge(
                    item.as_str(),
                    at,
                    integration
                        .allowed_commands()
                        .iter()
                        .map(|p| format!("Bash({p} *)"))
                        .chain(
                            integration
                                .allowed_mcp_tools()
                                .iter()
                                .map(|(s, t)| format!("mcp__{s}__{t}")),
                        )
                        .map(|rule| MergeOp::array_entry(["permissions", "allow"], rule))
                        .collect(),
                ),
                Item::Agents => parts::agents(&at, ".md", integration, |a| a.to_markdown(&[])),
                Item::Commands => parts::commands(&at, ".md", integration, |c| c.to_markdown()),
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(Reads::always(location(item, cx)?))
    }

    // @zen-impl: HAR-1_AC-4
    // @zen-impl: HAR-9_AC-2
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        // The Cursor CLI runs Claude Code's hooks with its own input.
        if protocol::raw_object(text)?.get("cursor_version").is_some() {
            return crate::cursor::Cursor.parse_hook(event, text);
        }
        protocol::parse(ID, event, text, tool_kind, "last_assistant_message")
    }

    // @zen-impl: HAR-1_AC-5
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        let name = event_name(event).unwrap_or_default();
        protocol::answer(ID, event, name, answer)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, time::Duration};

    use serde_json::json;

    use super::*;
    use crate::{
        harness::{EntryMatch, Profile},
        hook::emit,
        integration::{Agent, Command, Hook, McpServer, Skill},
        test_support::temp_dir,
    };

    // @zen-test: HAR-1_AC-2
    #[test]
    fn the_instructions_rule() {
        let dir = temp_dir("claude-rule");
        let t = || instructions_file(&dir).unwrap();
        // Neither file: CLAUDE.md, created later.
        assert_eq!(t(), "CLAUDE.md");
        // AGENTS.md alone.
        fs::write(dir.join("AGENTS.md"), "# Agents\n").unwrap();
        assert_eq!(t(), "AGENTS.md");
        // Both, CLAUDE.md without the import.
        fs::write(dir.join("CLAUDE.md"), "# Claude\n").unwrap();
        assert_eq!(t(), "CLAUDE.md");
        // Both, with the import, spaces around it allowed.
        fs::write(dir.join("CLAUDE.md"), "  @AGENTS.md  \n").unwrap();
        assert_eq!(t(), "AGENTS.md");
        // CLAUDE.md alone.
        fs::remove_file(dir.join("AGENTS.md")).unwrap();
        assert_eq!(t(), "CLAUDE.md");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unreadable_claude_md_is_an_io_error() {
        let dir = temp_dir("claude-unreadable");
        fs::write(dir.join("AGENTS.md"), "a").unwrap();
        // A directory named CLAUDE.md is not a file: AGENTS.md alone.
        fs::create_dir(dir.join("CLAUDE.md")).unwrap();
        assert_eq!(instructions_file(&dir).unwrap(), "AGENTS.md");
        fs::remove_dir(dir.join("CLAUDE.md")).unwrap();
        // Invalid UTF-8 cannot be read as text.
        fs::write(dir.join("CLAUDE.md"), [0xff, 0xfe]).unwrap();
        assert!(matches!(instructions_file(&dir), Err(Error::Io { .. })));
        fs::remove_dir_all(&dir).unwrap();
    }

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(Hook::new(Event::SessionStart, "t hook {harness} {event}"))
            .hook(
                Hook::new(Event::PreTool, "t hook {harness} {event}")
                    .tools(ToolKind::Shell)
                    .timeout(Duration::from_secs(5)),
            )
            .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .allow_command("t")
            .allow_mcp_tool("t", "run")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
    }

    fn rendered(scope: Scope) -> Profile {
        let dir = temp_dir("claude-render");
        let cx = Context::new("t", scope, &dir, None);
        let p = Profile::new(ID, Claude.render(&full(), &cx).unwrap());
        fs::remove_dir_all(&dir).unwrap();
        p
    }

    // @zen-test: HAR-1_AC-1
    // @zen-test: HAR-1_AC-3
    // @zen-test: KIT-11_AC-1
    // @zen-test: KIT-17_AC-2
    // @zen-test: HAR-1_AC-6
    // @zen-test: HAR-1_AC-7
    // @zen-test: HAR-1_AC-8
    #[test]
    fn renders_every_item_per_scope() {
        let p = rendered(Scope::Project);
        let rows: Vec<_> = p
            .parts()
            .iter()
            .map(|p| (p.name().to_string(), p.location()))
            .collect();
        insta::assert_debug_snapshot!(rows, @r#"
        [
            (
                "instructions",
                "CLAUDE.md",
            ),
            (
                "skills",
                ".claude/skills",
            ),
            (
                "hooks",
                ".claude/settings.json",
            ),
            (
                "mcp",
                ".mcp.json",
            ),
            (
                "permissions",
                ".claude/settings.json",
            ),
            (
                "agents",
                ".claude/agents",
            ),
            (
                "commands",
                ".claude/commands",
            ),
        ]
        "#);
        let ops = p.part("hooks").unwrap().ops();
        // One op per Claude event, the tool's hooks by matcher.
        assert_eq!(ops.len(), 7);
        assert_eq!(
            ops[3],
            MergeOp::group_entries(
                ["hooks", "PreToolUse"],
                "hooks",
                "command",
                EntryMatch::Prefix("t hook ".into()),
                vec![
                    json!({ "matcher": "Bash", "hooks": [{ "type": "command", "command": "t hook claude pre-tool", "timeout": 5 }] })
                ],
            )
        );
        assert_eq!(
            ops[5].value(),
            &json!([{ "hooks": [{ "type": "command", "command": "t hook claude stop" }] }])
        );
        let ops = p.part("permissions").unwrap().ops();
        assert_eq!(
            ops,
            &[
                MergeOp::array_entry(["permissions", "allow"], "Bash(t *)"),
                MergeOp::array_entry(["permissions", "allow"], "mcp__t__run")
            ]
        );
        let names = |s| -> Vec<String> {
            rendered(s)
                .parts()
                .iter()
                .map(|p| format!("{} {}", p.name(), p.location()))
                .collect()
        };
        assert_eq!(
            names(Scope::User),
            [
                "instructions .claude/CLAUDE.md",
                "skills .claude/skills",
                "hooks .claude/settings.json",
                "permissions .claude/settings.json",
                "agents .claude/agents",
                "commands .claude/commands"
            ]
        );
        assert_eq!(
            names(Scope::Local),
            [
                "hooks .claude/settings.local.json",
                "permissions .claude/settings.local.json"
            ]
        );
        assert_eq!(Claude.scopes().len(), 3);
        assert_eq!(Claude.id(), "claude");
        let cx = Context::new("t", Scope::Local, "/nope", None);
        assert_eq!(Claude.reads(Item::Skills, &cx).unwrap(), Reads::default());
        assert_eq!(
            Claude.reads(Item::Hooks, &cx).unwrap(),
            Reads::always([".claude/settings.local.json"])
        );
    }

    // @zen-test: HAR-1_AC-4
    // @zen-test: KIT-11_AC-3
    // @zen-test: KIT-11_AC-6
    #[test]
    fn input_is_read_and_tools_classified() {
        let i = Claude
            .parse_hook(
                Event::PreTool,
                r#"{"session_id":"s","tool_name":"Edit","tool_input":{"file_path":"a"}}"#,
            )
            .unwrap();
        assert_eq!(i.harness, "claude");
        assert_eq!(i.tool.unwrap().kind, ToolKind::Write);
        for (name, kind) in [
            ("Bash", ToolKind::Shell),
            ("Grep", ToolKind::Read),
            ("mcp__x__y", ToolKind::Mcp),
            ("Task", ToolKind::Other),
        ] {
            assert_eq!(tool_kind(name), kind);
        }
        assert!(
            Claude
                .parse_hook(Event::Stop, r#"{"stop_hook_active":true}"#)
                .unwrap()
                .continuing
        );
        // A Cursor payload is read as Cursor's.
        // @zen-test: HAR-9_AC-2
        let i = Claude
            .parse_hook(
                Event::Stop,
                r#"{"cursor_version":"2","loop_count":2,"conversation_id":"c"}"#,
            )
            .unwrap();
        assert_eq!((i.harness.as_str(), i.continuing), ("cursor", true));
    }

    // @zen-test: HAR-1_AC-5
    // @zen-test: KIT-11_AC-4
    #[test]
    fn each_answer_in_claudes_form() {
        let out = |e, a: Answer| Claude.answer(e, &a).map(|o| o.stdout);
        assert_eq!(
            out(Event::Stop, Answer::Allow { stderr: None }).unwrap(),
            "{}"
        );
        assert_eq!(
            out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
            r#"{"decision":"block","reason":"r"}"#
        );
        assert_eq!(
            out(Event::PromptSubmit, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"decision":"block","reason":"r"}"#
        );
        assert_eq!(
            out(Event::PreTool, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"r"}}"#
        );
        assert_eq!(
            out(Event::SessionStart, Answer::Context { text: "hi".into() }).unwrap(),
            r#"{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"hi"}}"#
        );
        let e = out(Event::Stop, Answer::Deny { reason: "r".into() }).unwrap_err();
        assert_eq!(e.to_string(), "harness `claude` cannot answer deny at stop");
        assert!(out(Event::PreTool, Answer::Continue { reason: "r".into() }).is_err());
        assert!(out(Event::Stop, Answer::Context { text: "t".into() }).is_err());
    }

    // @zen-test: KIT-11_AC-5
    #[test]
    fn emit_writes_the_answer_or_the_error() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = emit::<String>(
            &Claude,
            Event::Stop,
            Ok(Answer::Allow {
                stderr: Some("report\n".into()),
            }),
            &mut out,
            &mut err,
        )
        .unwrap();
        assert_eq!(
            (code, out.as_slice(), err.as_slice()),
            (0, &b"{}\n"[..], &b"report\n"[..])
        );
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = emit(
            &Claude,
            Event::Stop,
            Err::<Answer, _>("no config"),
            &mut out,
            &mut err,
        )
        .unwrap();
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert_eq!(String::from_utf8(err).unwrap(), "error: no config\n");
        // An answer the harness cannot give is a failure too.
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = emit::<String>(
            &Claude,
            Event::Stop,
            Ok(Answer::Deny { reason: "r".into() }),
            &mut out,
            &mut err,
        )
        .unwrap();
        assert_eq!(code, 1);
        assert!(out.is_empty());
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "error: harness `claude` cannot answer deny at stop\n"
        );
    }
}
