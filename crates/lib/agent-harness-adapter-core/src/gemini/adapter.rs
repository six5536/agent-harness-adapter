//! Gemini CLI.
//!
//! Follows <https://geminicli.com/docs/cli/gemini-md/>,
//! <https://geminicli.com/docs/reference/configuration/>,
//! <https://geminicli.com/docs/hooks/>, <https://geminicli.com/docs/hooks/reference/>,
//! <https://geminicli.com/docs/cli/custom-commands/>,
//! <https://geminicli.com/docs/cli/skills/>,
//! <https://geminicli.com/docs/core/subagents/> and
//! <https://geminicli.com/docs/cli/trusted-folders/> (checked 2026-10-06).
// @zen-component: HAR-Gemini

use std::time::Duration;

use serde_json::{Value, json};

use crate::{
    Error, Result,
    common::{
        parts::{self, GroupHooks},
        protocol, toml_out,
    },
    fs::read_text,
    harness::{Context, Harness, MergeOp, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Integration, Item, McpServer, Transport},
};

/// Gemini CLI (`gemini`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Gemini;

const ID: &str = "gemini";
const SETTINGS: &str = ".gemini/settings.json";

/// Gemini CLI's name for `event`.
// @zen-impl: HAR-4_AC-3
fn event_name(event: Event) -> Option<&'static str> {
    Some(match event {
        Event::SessionStart => "SessionStart",
        Event::SessionEnd => "SessionEnd",
        Event::PromptSubmit => "BeforeAgent",
        Event::PreTool => "BeforeTool",
        Event::PostTool => "AfterTool",
        Event::Stop => "AfterAgent",
        Event::PreCompact => "PreCompress",
    })
}

fn matcher(kind: ToolKind) -> Option<&'static str> {
    match kind {
        ToolKind::Shell => Some("run_shell_command"),
        ToolKind::Read => Some("read_file|read_many_files|glob|search_file_content|list_directory"),
        ToolKind::Write => Some("write_file|replace"),
        ToolKind::Mcp => Some("mcp_.*"),
        _ => None,
    }
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "run_shell_command" => ToolKind::Shell,
        "read_file" | "read_many_files" | "glob" | "search_file_content" | "list_directory" => {
            ToolKind::Read
        }
        "write_file" | "replace" => ToolKind::Write,
        n if n.starts_with("mcp_") => ToolKind::Mcp,
        _ => ToolKind::Other,
    }
}

fn millis(t: Duration) -> Value {
    json!(u64::try_from(t.as_millis()).unwrap_or(u64::MAX))
}

const LAYOUT: GroupHooks = GroupHooks {
    path: |e| vec!["hooks".into(), e.into()],
    event: event_name,
    matcher,
    timeout: millis,
    harness: ID,
};

/// The context file names `settings` lists in `context.fileName` (a string
/// or an array); `None` when it does not set it. A file that does not parse
/// is a refusal naming `display`.
fn file_names(text: Option<String>, display: &str) -> Result<Option<Vec<String>>> {
    let Some(text) = text else {
        return Ok(None);
    };
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| Error::file(display, format!("does not parse as JSON: {e}")))?;
    Ok(match &doc["context"]["fileName"] {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(a) => Some(
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        )
        .filter(|v: &Vec<String>| !v.is_empty()),
        _ => None,
    })
}

/// The context files Gemini CLI loads at `cx`: the scope's setting, else (at
/// project scope) the user's, else `GEMINI.md`; and the one the
/// instructions go in: `AGENTS.md` when listed, else the first.
// @zen-impl: HAR-4_AC-2
fn context_files(cx: &Context) -> Result<(String, Vec<String>)> {
    let mut names = file_names(read_text(&cx.path(SETTINGS))?, SETTINGS)?;
    if names.is_none() && cx.scope == Scope::Project {
        if let Some(path) = cx.user_path(SETTINGS) {
            names = file_names(read_text(&path)?, "~/.gemini/settings.json")?;
        }
    }
    let names = names.unwrap_or_else(|| vec!["GEMINI.md".into()]);
    let dir = if cx.scope == Scope::User {
        ".gemini/"
    } else {
        ""
    };
    let all: Vec<String> = names.iter().map(|n| format!("{dir}{n}")).collect();
    let chosen = names
        .iter()
        .position(|n| n == "AGENTS.md")
        .map_or_else(|| all[0].clone(), |i| all[i].clone());
    Ok((chosen, all))
}

/// An MCP server in Gemini CLI's form: an http server's URL as `httpUrl`.
// @zen-impl: HAR-4_AC-7
fn mcp_entry(s: &McpServer) -> Value {
    match s.transport() {
        Transport::Stdio { .. } => s.to_json(),
        Transport::Http { url, headers } => {
            let mut v = json!({ "httpUrl": url });
            if !headers.is_empty() {
                v["headers"] = s.to_json()["headers"].clone();
            }
            v
        }
    }
}

/// A command as a `.toml` file, `$ARGUMENTS` written as `{{args}}`.
fn command_toml(c: &crate::integration::Command) -> String {
    toml_out::strings(&[
        ("description", c.description()),
        ("prompt", &c.prompt().replace("$ARGUMENTS", "{{args}}")),
    ])
}

fn location(item: Item, cx: &Context) -> Result<Option<String>> {
    Ok(Some(match item {
        Item::Instructions => context_files(cx)?.0,
        Item::Skills => ".agents/skills".into(),
        Item::Hooks | Item::Mcp | Item::Permissions => SETTINGS.into(),
        Item::Agents => ".gemini/agents".into(),
        Item::Commands => ".gemini/commands".into(),
    }))
}

impl Harness for Gemini {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-4_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-4_AC-6
    // @zen-impl: HAR-4_AC-8
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
                Item::Mcp => parts::mcp(&at, "mcpServers", integration, mcp_entry),
                // MCP tools: no confirmed allow-list form.
                Item::Permissions if integration.allowed_commands().is_empty() => continue,
                Item::Permissions => Part::merge(
                    item.as_str(),
                    at,
                    integration
                        .allowed_commands()
                        .iter()
                        .map(|p| {
                            MergeOp::array_entry(
                                ["tools", "allowed"],
                                format!("run_shell_command({p})"),
                            )
                        })
                        .collect(),
                ),
                Item::Agents => parts::agents(&at, ".md", integration, |a| a.to_markdown(&[])),
                Item::Commands => parts::commands(&at, ".toml", integration, command_toml),
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(match item {
            Item::Instructions => Reads::always(context_files(cx)?.1),
            _ => Reads::always(location(item, cx)?),
        })
    }

    // @zen-impl: HAR-4_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        protocol::parse(ID, event, text, tool_kind, "prompt_response")
    }

    // @zen-impl: HAR-4_AC-5
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        Ok(match (answer, event) {
            (Answer::Allow { stderr }, _) => Output::json(json!({})).stderr(stderr.clone()),
            (Answer::Deny { reason }, Event::PreTool | Event::PromptSubmit)
            | (Answer::Continue { reason }, Event::Stop) => {
                Output::json(json!({ "decision": "deny", "reason": reason }))
            }
            (
                Answer::Context { text },
                Event::SessionStart | Event::PromptSubmit | Event::PostTool,
            ) => Output::json(json!({ "hookSpecificOutput": { "additionalContext": text } })),
            _ => return Err(protocol::unsupported(ID, event, answer)),
        })
    }

    // @zen-impl: HAR-4_AC-9
    fn notes(&self, cx: &Context, parts: &[PartResult]) -> Vec<String> {
        if cx.scope == Scope::Project && parts.iter().any(PartResult::wrote) {
            vec!["with folder trust on, Gemini CLI reads the project's settings only in a trusted folder".into()]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::{
        harness::{Action, Profile, State},
        integration::{Agent, Command, Hook, Skill},
        test_support::temp_dir,
    };

    fn full() -> Integration {
        Integration::new()
            .instructions("Use t.\n")
            .skill(Skill::new("t", "Use t.", "# T\n"))
            .hook(
                Hook::new(Event::PreTool, "t hook {harness} {event}")
                    .tools(ToolKind::Shell)
                    .timeout(Duration::from_secs(2)),
            )
            .mcp_server(McpServer::http("w", "https://w").header("A", "b"))
            .allow_command("t")
            .agent(Agent::new("r", "Reviews.", "Review.\n"))
            .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
    }

    // @zen-test: HAR-4_AC-1
    // @zen-test: HAR-4_AC-3
    // @zen-test: HAR-4_AC-6
    // @zen-test: HAR-4_AC-7
    // @zen-test: HAR-4_AC-8
    #[test]
    fn renders_every_item() {
        let dir = temp_dir("gemini");
        let cx = Context::new("t", Scope::Project, &dir, None);
        let p = Profile::new(Gemini.render(&full(), &cx).unwrap());
        let names: Vec<_> = p
            .parts()
            .iter()
            .map(|p| format!("{} {}", p.name(), p.location()))
            .collect();
        assert_eq!(
            names,
            [
                "instructions GEMINI.md",
                "skills .agents/skills",
                "hooks .gemini/settings.json",
                "mcp .gemini/settings.json",
                "permissions .gemini/settings.json",
                "agents .gemini/agents",
                "commands .gemini/commands"
            ]
        );
        assert_eq!(
            p.part("hooks").unwrap().ops()[3].value(),
            &json!([{ "matcher": "run_shell_command", "hooks": [{ "type": "command", "command": "t hook gemini pre-tool", "timeout": 2000 }] }])
        );
        assert_eq!(
            p.part("hooks").unwrap().ops()[5].path(),
            ["hooks", "AfterAgent"]
        );
        assert_eq!(
            p.part("mcp").unwrap().ops()[0].value(),
            &json!({ "httpUrl": "https://w", "headers": { "A": "b" } })
        );
        assert_eq!(
            p.part("permissions").unwrap().ops(),
            &[MergeOp::array_entry(
                ["tools", "allowed"],
                "run_shell_command(t)"
            )]
        );
        let cmd = command_toml(&Command::new("c", "Checks.", "Check $ARGUMENTS.\n"));
        assert!(cmd.contains("Check {{args}}."), "{cmd}");
        assert_eq!(Gemini.scopes(), [Scope::Project, Scope::User]);
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: HAR-4_AC-2
    #[test]
    fn the_context_file_follows_the_settings() {
        let dir = temp_dir("gemini-ctx");
        let home = temp_dir("gemini-home");
        let cx = Context::new("t", Scope::Project, &dir, Some(home.clone()));
        assert_eq!(
            context_files(&cx).unwrap(),
            ("GEMINI.md".into(), vec!["GEMINI.md".into()])
        );
        // The user's setting applies to the project.
        fs::create_dir_all(home.join(".gemini")).unwrap();
        fs::write(
            home.join(SETTINGS),
            r#"{"context":{"fileName":"AGENTS.md"}}"#,
        )
        .unwrap();
        assert_eq!(context_files(&cx).unwrap().0, "AGENTS.md");
        // The project's own setting wins; AGENTS.md is chosen when listed.
        fs::create_dir_all(dir.join(".gemini")).unwrap();
        fs::write(
            dir.join(SETTINGS),
            r#"{"context":{"fileName":["GEMINI.md","AGENTS.md"]}}"#,
        )
        .unwrap();
        assert_eq!(
            context_files(&cx).unwrap(),
            (
                "AGENTS.md".into(),
                vec!["GEMINI.md".into(), "AGENTS.md".into()]
            )
        );
        assert_eq!(
            Gemini.reads(Item::Instructions, &cx).unwrap(),
            Reads::always(["GEMINI.md", "AGENTS.md"])
        );
        fs::write(dir.join(SETTINGS), r#"{"context":{"fileName":["CTX.md"]}}"#).unwrap();
        assert_eq!(context_files(&cx).unwrap().0, "CTX.md");
        fs::write(dir.join(SETTINGS), r#"{"context":{"fileName":[]}}"#).unwrap();
        assert_eq!(context_files(&cx).unwrap().0, "AGENTS.md");
        // At user scope, under ~/.gemini.
        let user = Context::new("t", Scope::User, &home, Some(home.clone()));
        assert_eq!(context_files(&user).unwrap().0, ".gemini/AGENTS.md");
        fs::write(dir.join(SETTINGS), "{ nope").unwrap();
        assert!(matches!(context_files(&cx), Err(Error::File { .. })));
        fs::remove_dir_all(&dir).unwrap();
        fs::remove_dir_all(&home).unwrap();
    }

    // @zen-test: HAR-4_AC-4
    // @zen-test: HAR-4_AC-5
    #[test]
    fn input_and_answers() {
        let i = Gemini
            .parse_hook(
                Event::Stop,
                r#"{"stop_hook_active":true,"prompt_response":"done","tool_name":"replace"}"#,
            )
            .unwrap();
        assert!(i.continuing);
        assert_eq!(i.last_message.as_deref(), Some("done"));
        assert_eq!(i.tool.unwrap().kind, ToolKind::Write);
        for (n, k) in [
            ("run_shell_command", ToolKind::Shell),
            ("glob", ToolKind::Read),
            ("mcp_s_t", ToolKind::Mcp),
            ("web_fetch", ToolKind::Other),
        ] {
            assert_eq!(tool_kind(n), k);
        }
        let out = |e, a: Answer| Gemini.answer(e, &a).map(|o| o.stdout);
        assert_eq!(
            out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
            r#"{"decision":"deny","reason":"r"}"#
        );
        assert_eq!(
            out(Event::PreTool, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"decision":"deny","reason":"r"}"#
        );
        assert_eq!(
            out(Event::SessionStart, Answer::Context { text: "c".into() }).unwrap(),
            r#"{"hookSpecificOutput":{"additionalContext":"c"}}"#
        );
        assert_eq!(
            out(Event::Stop, Answer::Allow { stderr: None }).unwrap(),
            "{}"
        );
        assert!(out(Event::Stop, Answer::Deny { reason: "r".into() }).is_err());
    }

    // @zen-test: HAR-4_AC-9
    #[test]
    fn a_note_on_folder_trust() {
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        let written = PartResult {
            part: "hooks".into(),
            state: State::Absent,
            action: Some(Action::Created),
            path: String::new(),
            by: None,
        };
        assert_eq!(Gemini.notes(&cx, std::slice::from_ref(&written)).len(), 1);
        let user = Context::new("t", Scope::User, "/nowhere", None);
        assert!(Gemini.notes(&user, &[written]).is_empty());
    }
}
