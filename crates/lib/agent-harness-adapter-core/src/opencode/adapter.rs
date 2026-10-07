//! OpenCode (`opencode-ai`).
//!
//! Follows `packages/web/src/content/docs/` of
//! <https://github.com/sst/opencode> (`rules.mdx`, `config.mdx`,
//! `skills.mdx`, `agents.mdx`, `commands.mdx`, `mcp-servers.mdx`,
//! `permissions.mdx`, `plugins.mdx`) and its source (`session/instruction.ts`,
//! `config/plugin.ts`, `session/prompt.ts`, `session/tools.ts`,
//! `cli/cmd/run.ts`), version 1.18.35 (checked 2026-10-07).
// @zen-component: HAR-OpenCode

use serde_json::{Value, json};

use crate::{
    Result,
    common::{extension, parts},
    harness::{Context, Harness, MergeOp, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output, ToolKind},
    integration::{Integration, Item, McpServer, Transport},
};

/// OpenCode (`opencode`).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCode;

const ID: &str = "opencode";
const TEMPLATE: &str = include_str!("plugin.ts");

/// The directory OpenCode's own files are under at `scope`: the project's
/// `.opencode`, or `~/.config/opencode`.
fn base(scope: Scope) -> &'static str {
    match scope {
        Scope::User => ".config/opencode",
        _ => ".opencode",
    }
}

/// The config file at `scope`: the project's `opencode.json`, or
/// `~/.config/opencode/opencode.json`.
fn config(scope: Scope) -> &'static str {
    match scope {
        Scope::User => ".config/opencode/opencode.json",
        _ => "opencode.json",
    }
}

/// The instructions file: the first of `AGENTS.md`, `CLAUDE.md` that exists
/// (`.config/opencode/AGENTS.md`, `.claude/CLAUDE.md` at user scope), else
/// the first. OpenCode loads only that one.
// @zen-impl: HAR-10_AC-2
fn instructions_file(cx: &Context) -> &'static str {
    let names: [&'static str; 2] = match cx.scope {
        Scope::User => [".config/opencode/AGENTS.md", ".claude/CLAUDE.md"],
        _ => ["AGENTS.md", "CLAUDE.md"],
    };
    names
        .into_iter()
        .find(|p| cx.is_file(p))
        .unwrap_or(names[0])
}

/// The plugin that runs the integration's hooks.
// @zen-impl: HAR-10_AC-3
fn plugin(tool: &str, integration: &Integration) -> String {
    extension::fill(TEMPLATE, ID, tool, integration)
}

/// OpenCode's entry for `server`: `{type: "local", command: [command,
/// ...args], environment?}` or `{type: "remote", url, headers?}`.
// @zen-impl: HAR-10_AC-5
fn mcp_json(server: &McpServer) -> Value {
    let pairs = |kv: &[(String, String)]| -> Value {
        Value::Object(
            kv.iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        )
    };
    match server.transport() {
        Transport::Stdio { command, args, env } => {
            let mut v = json!({
                "type": "local",
                "command": std::iter::once(command).chain(args).collect::<Vec<_>>(),
            });
            if !env.is_empty() {
                v["environment"] = pairs(env);
            }
            v
        }
        Transport::Http { url, headers } => {
            let mut v = json!({ "type": "remote", "url": url });
            if !headers.is_empty() {
                v["headers"] = pairs(headers);
            }
            v
        }
    }
}

/// OpenCode's tools by name; MCP tools (`<server>_<tool>`) cannot be told
/// from its own.
fn tool_kind(name: &str) -> ToolKind {
    match name {
        "bash" => ToolKind::Shell,
        "read" | "glob" | "grep" | "list" => ToolKind::Read,
        "edit" | "write" | "apply_patch" => ToolKind::Write,
        _ => ToolKind::Other,
    }
}

fn location(item: Item, cx: &Context) -> Option<String> {
    let base = base(cx.scope);
    Some(match item {
        Item::Instructions => instructions_file(cx).into(),
        Item::Skills => ".agents/skills".into(),
        Item::Hooks => format!("{base}/plugins"),
        Item::Mcp | Item::Permissions => config(cx.scope).into(),
        Item::Agents => format!("{base}/agents"),
        Item::Commands => format!("{base}/commands"),
    })
}

impl Harness for OpenCode {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-10_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-10_AC-5
    // @zen-impl: HAR-10_AC-6
    // @zen-impl: HAR-10_AC-7
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
                    vec![(format!("{}.ts", cx.tool), plugin(&cx.tool, integration))],
                ),
                Item::Mcp => parts::mcp(&at, "mcp", integration, mcp_json),
                // Allowed MCP tools: how OpenCode matches them is not confirmed.
                Item::Permissions if integration.allowed_commands().is_empty() => continue,
                Item::Permissions => Part::merge(
                    item.as_str(),
                    at,
                    integration
                        .allowed_commands()
                        .iter()
                        .map(|p| {
                            MergeOp::object_member(
                                ["permission", "bash"],
                                format!("{p} *"),
                                "allow",
                            )
                        })
                        .collect(),
                ),
                Item::Agents => parts::agents(&at, ".md", integration, |a| {
                    a.to_markdown(&[("mode", "subagent")])
                }),
                Item::Commands => parts::commands(&at, ".md", integration, |c| c.to_markdown()),
            });
        }
        Ok(out)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(match item {
            Item::Skills => Reads::always([
                ".agents/skills".to_string(),
                ".claude/skills".to_string(),
                format!("{}/skills", base(cx.scope)),
            ]),
            _ => Reads::always(location(item, cx)),
        })
    }

    // @zen-impl: HAR-10_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        extension::parse(ID, event, text, tool_kind)
    }

    // @zen-impl: HAR-10_AC-4
    // @zen-impl: AHA-2_AC-3
    fn answer(&self, event: Event, answer: &Answer) -> Result<Output> {
        extension::answer(ID, event, answer)
    }

    // @zen-impl: HAR-10_AC-8
    fn notes(&self, _cx: &Context, parts: &[PartResult]) -> Vec<String> {
        if parts.iter().any(PartResult::changed) {
            vec!["restart OpenCode to load the changes".into()]
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
