//! Devin CLI and Devin Local (Devin Desktop runs the same agent).
//!
//! Follows <https://docs.devin.ai/cli/extensibility/hooks/overview>,
//! <https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks>,
//! <https://docs.devin.ai/cli/extensibility/rules>,
//! <https://docs.devin.ai/cli/extensibility/skills/overview>,
//! <https://docs.devin.ai/cli/extensibility/mcp/configuration>,
//! <https://docs.devin.ai/cli/reference/permissions>,
//! <https://docs.devin.ai/cli/reference/configuration/read-config-from> and
//! <https://docs.devin.ai/cli/subagents>, for Devin CLI 3000.11.3 (checked
//! 2026-10-07).
// @zen-component: HAR-Devin

use serde_json::json;

use crate::{
    Result,
    common::{
        parts::{self, GroupHooks},
        protocol,
    },
    harness::{Context, Harness, MergeOp, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Integration, Item},
};

/// Devin CLI and Devin Local (`devin`).
#[derive(Debug, Clone, Copy, Default)]
pub struct Devin;

const ID: &str = "devin";

/// Devin's name for `event`: Claude Code's names; it has no event before
/// compaction.
// @zen-impl: HAR-11_AC-3
fn event_name(event: Event) -> Option<&'static str> {
    Some(match event {
        Event::SessionStart => "SessionStart",
        Event::SessionEnd => "SessionEnd",
        Event::PromptSubmit => "UserPromptSubmit",
        Event::PreTool => "PreToolUse",
        Event::PostTool => "PostToolUse",
        Event::Stop => "Stop",
        Event::PreCompact => return None,
    })
}

/// Matchers are unanchored regular expressions on the tool's name.
fn matcher(kind: ToolKind) -> Option<&'static str> {
    Some(match kind {
        ToolKind::Shell => "^exec$",
        ToolKind::Read => "^(read|grep|glob|notebook_read)$",
        ToolKind::Write => "^(write|edit|apply_patch|notebook_edit)$",
        ToolKind::Mcp => "^mcp__",
        _ => return None,
    })
}

fn tool_kind(name: &str) -> ToolKind {
    match name {
        "exec" => ToolKind::Shell,
        "read" | "grep" | "glob" | "notebook_read" => ToolKind::Read,
        "write" | "edit" | "apply_patch" | "notebook_edit" => ToolKind::Write,
        n if n.starts_with("mcp__") => ToolKind::Mcp,
        _ => ToolKind::Other,
    }
}

/// The project's hooks file is the hooks object itself; the user's are
/// under `hooks` of the user's config.
const PROJECT_HOOKS: GroupHooks = GroupHooks {
    path: |e| vec![e.into()],
    event: event_name,
    matcher,
    timeout: parts::seconds,
    harness: ID,
};
const USER_HOOKS: GroupHooks = GroupHooks {
    path: |e| vec!["hooks".into(), e.into()],
    ..PROJECT_HOOKS
};

fn location(item: Item, cx: &Context) -> Option<&'static str> {
    let user = cx.scope == Scope::User;
    Some(match item {
        Item::Instructions if user => ".config/devin/AGENTS.md",
        Item::Instructions => "AGENTS.md",
        Item::Skills => ".agents/skills",
        Item::Hooks if user => ".config/devin/config.json",
        Item::Hooks => ".devin/hooks.v1.json",
        Item::Mcp if user => ".config/devin/mcp_config.json",
        Item::Mcp => ".devin/mcp_config.json",
        Item::Permissions if user => ".config/devin/config.json",
        Item::Permissions => ".devin/config.json",
        Item::Agents if user => ".config/devin/agents",
        Item::Agents => ".devin/agents",
        // Slash commands are skills in Devin.
        Item::Commands => return None,
    })
}

impl Harness for Devin {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-11_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-11_AC-2
    // @zen-impl: HAR-11_AC-6
    // @zen-impl: HAR-11_AC-7
    // @zen-impl: HAR-11_AC-8
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
                    let layout = if cx.scope == Scope::User {
                        &USER_HOOKS
                    } else {
                        &PROJECT_HOOKS
                    };
                    Part::merge(item.as_str(), at, parts::group_hooks(layout, integration)?)
                }
                Item::Mcp => parts::mcp(at, "mcpServers", integration, |s| s.to_json()),
                Item::Permissions => Part::merge(
                    item.as_str(),
                    at,
                    integration
                        .allowed_commands()
                        .iter()
                        .map(|p| format!("Exec({p})"))
                        .chain(
                            integration
                                .allowed_mcp_tools()
                                .iter()
                                .map(|(s, t)| format!("mcp__{s}__{t}")),
                        )
                        .map(|rule| MergeOp::array_entry(["permissions", "allow"], rule))
                        .collect(),
                ),
                Item::Agents => parts::agents(at, ".md", integration, |a| a.to_markdown(&[])),
                Item::Commands => continue,
            });
        }
        Ok(out)
    }

    // Devin also reads Claude Code's files (`read_config_from.claude`, on
    // unless turned off): CLAUDE.md, its skills, `.mcp.json`, its agents,
    // and the hooks in its settings.
    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        let user = cx.scope == Scope::User;
        let own = Reads::always(location(item, cx));
        Ok(match item {
            Item::Instructions if user => own.maybe([".claude/CLAUDE.md"]),
            Item::Instructions => Reads::always(["AGENTS.md", "CLAUDE.md"]),
            Item::Skills if user => {
                Reads::always([".agents/skills", ".config/devin/skills", ".claude/skills"])
            }
            Item::Skills => Reads::always([
                ".agents/skills",
                ".devin/skills",
                ".claude/skills",
                ".github/skills",
            ]),
            Item::Hooks => own.maybe([".claude/settings.json"]),
            Item::Mcp if !user => Reads::always([".devin/mcp_config.json", ".mcp.json"]),
            Item::Agents if user => own.maybe([".claude/agents"]),
            Item::Agents => own.maybe([".agents/agents", ".claude/agents"]),
            _ => own,
        })
    }

    // @zen-impl: HAR-11_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        let mut input = protocol::parse(ID, event, text, tool_kind, "last_assistant_message")?;
        // Devin sends no working directory; it sets DEVIN_PROJECT_DIR.
        if input.cwd.is_none() {
            input.cwd = std::env::var("DEVIN_PROJECT_DIR").ok();
        }
        Ok(input)
    }

    // @zen-impl: HAR-11_AC-5
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        Ok(match (answer, event) {
            (Answer::Allow { stderr }, _) => Output::json(json!({})).stderr(stderr.clone()),
            (Answer::Deny { reason }, Event::PreTool | Event::PromptSubmit)
            | (Answer::Continue { reason }, Event::Stop) => {
                Output::json(json!({ "decision": "block", "reason": reason }))
            }
            (
                Answer::Context { text },
                Event::SessionStart | Event::PromptSubmit | Event::PostTool,
            ) => Output::json(json!({
                "hookSpecificOutput": {
                    "hookEventName": event_name(event).unwrap_or_default(),
                    "additionalContext": text,
                }
            })),
            _ => return Err(protocol::unsupported(ID, event, answer)),
        })
    }

    // @zen-impl: HAR-11_AC-9
    fn notes(&self, cx: &Context, parts: &[PartResult]) -> Vec<String> {
        let mut out = Vec::new();
        if !parts.iter().any(PartResult::changed) {
            return out;
        }
        if cx.scope == Scope::Project && parts.iter().any(PartResult::wrote) {
            out.push("Devin reads the project's files only once you trust the folder".into());
        }
        out.push("start a new Devin session to load the changes".into());
        out
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
