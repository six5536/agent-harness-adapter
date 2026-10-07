//! Qwen Code (`@qwen-code/qwen-code`).
//!
//! Follows `docs/users/` of <https://github.com/QwenLM/qwen-code>
//! (`features/hooks.md`, `memory.md`, `skills.md`, `sub-agents.md`,
//! `commands.md`, `mcp.md`, `configuration/settings.md`,
//! `configuration/trusted-folders.md`) and its source (`hooks/types.ts`,
//! `hooks/hook-matcher.ts`, `hooks/hook-timeout.ts`, `core/client.ts`,
//! `tools/tool-names.ts`, `memory/memoryDiscovery.ts`,
//! `permissions/rule-parser.ts`), version 0.25.0 (checked 2026-10-07).
// @zen-component: HAR-Qwen

use crate::{
    Result,
    common::{
        parts::{self, GroupHooks},
        protocol, settings,
    },
    harness::{Context, Harness, MergeOp, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Command, Integration, Item},
};

/// Qwen Code (`qwen`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Qwen;

const ID: &str = "qwen";
/// The settings file, under the project or the home directory.
const SETTINGS: &str = ".qwen/settings.json";

/// Qwen Code's name for `event`: Claude Code's names.
// @zen-impl: HAR-13_AC-3
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
    Some(match kind {
        ToolKind::Shell => "^run_shell_command$",
        ToolKind::Read => "^(read_file|grep_search|glob|list_directory)$",
        ToolKind::Write => "^(write_file|edit|notebook_edit)$",
        ToolKind::Mcp => "^mcp__",
        _ => return None,
    })
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "run_shell_command" => ToolKind::Shell,
        "read_file" | "grep_search" | "glob" | "list_directory" => ToolKind::Read,
        "write_file" | "edit" | "notebook_edit" => ToolKind::Write,
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

/// Where Qwen Code's context files are set and found: `QWEN.md` and
/// `AGENTS.md` unless `context.fileName` says otherwise.
const CONTEXT: settings::ContextLayout = settings::ContextLayout {
    settings: SETTINGS,
    user_display: "~/.qwen/settings.json",
    user_dir: ".qwen/",
    default: &["QWEN.md", "AGENTS.md"],
};

/// A command as Markdown, `$ARGUMENTS` written as `{{args}}`.
fn command_md(c: &Command) -> String {
    Command::new(
        c.name(),
        c.description(),
        c.prompt().replace("$ARGUMENTS", "{{args}}"),
    )
    .to_markdown()
}

fn location(item: Item, cx: &Context) -> Result<String> {
    Ok(match item {
        // @zen-impl: HAR-13_AC-2
        Item::Instructions => settings::context_files(cx, &CONTEXT)?.0,
        Item::Skills => ".agents/skills".into(),
        Item::Hooks | Item::Mcp | Item::Permissions => SETTINGS.into(),
        Item::Agents => ".qwen/agents".into(),
        Item::Commands => ".qwen/commands".into(),
    })
}

impl Harness for Qwen {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-13_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-13_AC-5
    // @zen-impl: HAR-13_AC-6
    // @zen-impl: HAR-13_AC-7
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        for item in integration.items() {
            let at = location(item, cx)?;
            out.push(match item {
                Item::Instructions => {
                    parts::instructions(&at, integration.instructions_block().unwrap_or_default())
                }
                Item::Skills => parts::skills(&at, integration),
                Item::Hooks => {
                    Part::merge(item.as_str(), at, parts::group_hooks(&LAYOUT, integration)?)
                }
                Item::Mcp => parts::mcp(&at, "mcpServers", integration, settings::mcp_entry),
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
                Item::Commands => parts::commands(&at, ".md", integration, command_md),
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(match item {
            Item::Instructions => Reads::always(settings::context_files(cx, &CONTEXT)?.1),
            Item::Skills => Reads::always([".agents/skills", ".qwen/skills"]),
            _ => Reads::always([location(item, cx)?]),
        })
    }

    // @zen-impl: HAR-13_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        protocol::parse(ID, event, text, tool_kind, "last_assistant_message")
    }

    // @zen-impl: HAR-13_AC-4
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        protocol::answer(ID, event, event_name(event).unwrap_or_default(), answer)
    }

    // @zen-impl: HAR-13_AC-8
    fn notes(&self, cx: &Context, parts: &[PartResult]) -> Vec<String> {
        let mut out = Vec::new();
        if !parts.iter().any(PartResult::changed) {
            return out;
        }
        if cx.scope == Scope::Project && parts.iter().any(PartResult::wrote) {
            out.push(
                "with folder trust on, Qwen Code reads the project's settings only in a trusted folder"
                    .into(),
            );
        }
        if cx.scope == Scope::Project && parts.iter().any(|p| p.part == "mcp" && p.wrote()) {
            out.push(
                "Qwen Code asks you to approve the project's MCP servers before it starts them"
                    .into(),
            );
        }
        out.push("restart Qwen Code to load the changes".into());
        out
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
