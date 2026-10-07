//! What OpenCode and its fork Kilo Code share: the generated plugin that
//! runs the tool's hooks, the MCP entries, tool names, the choice of
//! instructions file, and where each item goes given the agent's own paths.

use serde_json::{Value, json};

use crate::{
    Result,
    common::{extension, parts},
    harness::{Context, MergeOp, Part, Reads, Scope},
    hook::ToolKind,
    integration::{Integration, Item, McpServer, Transport},
};

/// An agent of the OpenCode family: its id, names and paths.
pub(crate) struct Layout {
    /// The harness id.
    pub(crate) harness: &'static str,
    /// The agent's name, as the plugin's comments say it.
    pub(crate) agent: &'static str,
    /// The package of the plugin's types.
    pub(crate) types: &'static str,
    /// The directory of the agent's own files: in the project, and under
    /// the home directory.
    pub(crate) project_dir: &'static str,
    pub(crate) user_dir: &'static str,
    /// The config file the agent's MCP servers and permissions go in, and
    /// the config files it also reads (relative to the project root, or to
    /// `user_dir` at user scope).
    pub(crate) config: &'static str,
    pub(crate) also_config: &'static [&'static str],
}

impl Layout {
    fn dir(&self, scope: Scope) -> &'static str {
        match scope {
            Scope::User => self.user_dir,
            _ => self.project_dir,
        }
    }

    fn config(&self, scope: Scope, name: &str) -> String {
        match scope {
            Scope::User => format!("{}/{name}", self.user_dir),
            _ => name.to_string(),
        }
    }

    /// Where `item` goes at `cx`.
    pub(crate) fn location(&self, item: Item, cx: &Context) -> String {
        let dir = self.dir(cx.scope);
        match item {
            Item::Instructions => instructions_file(cx, self.user_dir),
            Item::Skills => ".agents/skills".into(),
            Item::Hooks => format!("{dir}/plugins"),
            Item::Mcp | Item::Permissions => self.config(cx.scope, self.config),
            Item::Agents => format!("{dir}/agents"),
            Item::Commands => format!("{dir}/commands"),
        }
    }

    /// Every item of the integration the agent takes, as parts: the
    /// plugin, MCP entries and `permission.bash` patterns in the config,
    /// subagents (`mode: subagent`) and commands. Allowed MCP tools are
    /// left out: how the agents match them is not confirmed.
    pub(crate) fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        for item in integration.items() {
            let at = self.location(item, cx);
            out.push(match item {
                Item::Instructions => {
                    parts::instructions(&at, integration.instructions_block().unwrap_or_default())
                }
                Item::Skills => parts::skills(&at, integration),
                Item::Hooks => Part::files(
                    item.as_str(),
                    at,
                    vec![(
                        format!("{}.ts", cx.tool),
                        plugin(self.harness, self.agent, self.types, &cx.tool, integration),
                    )],
                ),
                Item::Mcp => parts::mcp(&at, "mcp", integration, mcp_entry),
                Item::Permissions if integration.allowed_commands().is_empty() => continue,
                Item::Permissions => Part::merge(
                    item.as_str(),
                    at,
                    integration
                        .allowed_commands()
                        .iter()
                        .map(|p| {
                            MergeOp::object_member(["permission", "bash"], bash_pattern(p), "allow")
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

    /// What the agent reads for `item`: its own location; skills also from
    /// `.claude/skills` and its own `skills`; config also from the other
    /// config files.
    pub(crate) fn reads(&self, item: Item, cx: &Context) -> Reads {
        match item {
            Item::Skills => Reads::always([
                ".agents/skills".to_string(),
                ".claude/skills".to_string(),
                format!("{}/skills", self.dir(cx.scope)),
            ]),
            Item::Mcp | Item::Permissions => Reads::always(
                std::iter::once(self.config)
                    .chain(self.also_config.iter().copied())
                    .map(|n| self.config(cx.scope, n)),
            ),
            _ => Reads::always([self.location(item, cx)]),
        }
    }
}

const TEMPLATE: &str = include_str!("plugin.ts");

/// The plugin for `agent` (its display name; types from `types`) that runs
/// the integration's hooks of `harness`.
pub(crate) fn plugin(
    harness: &str,
    agent: &str,
    types: &str,
    tool: &str,
    integration: &Integration,
) -> String {
    extension::fill(TEMPLATE, harness, tool, integration)
        .replace("__AGENT__", agent)
        .replace("__PLUGIN_TYPES__", types)
}

/// The instructions file: the first of `AGENTS.md`, `CLAUDE.md` that exists
/// (`<user_dir>/AGENTS.md`, `.claude/CLAUDE.md` at user scope), else the
/// first. The agent loads only that one.
pub(crate) fn instructions_file(cx: &Context, user_dir: &str) -> String {
    let names = match cx.scope {
        Scope::User => [format!("{user_dir}/AGENTS.md"), ".claude/CLAUDE.md".into()],
        _ => ["AGENTS.md".into(), "CLAUDE.md".into()],
    };
    names
        .iter()
        .find(|p| cx.is_file(p))
        .unwrap_or(&names[0])
        .clone()
}

/// The `mcp` entry of `server`: `{type: "local", command: [command,
/// ...args], environment?}` or `{type: "remote", url, headers?}`.
pub(crate) fn mcp_entry(server: &McpServer) -> Value {
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

/// The agents' tools by name; MCP tools (`<server>_<tool>`) cannot be told
/// from their own.
pub(crate) fn tool_kind(name: &str) -> ToolKind {
    match name {
        "bash" => ToolKind::Shell,
        "read" | "glob" | "grep" | "list" => ToolKind::Read,
        "edit" | "write" | "apply_patch" => ToolKind::Write,
        _ => ToolKind::Other,
    }
}

/// An allowed command prefix as a `permission.bash` pattern.
fn bash_pattern(prefix: &str) -> String {
    format!("{prefix} *")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_entries() {
        assert_eq!(
            mcp_entry(&McpServer::stdio("t", "t", ["mcp"]).env("K", "v")),
            json!({ "type": "local", "command": ["t", "mcp"], "environment": { "K": "v" } })
        );
        assert_eq!(
            mcp_entry(&McpServer::http("w", "https://w.example").header("H", "h")),
            json!({ "type": "remote", "url": "https://w.example", "headers": { "H": "h" } })
        );
        assert_eq!(
            mcp_entry(&McpServer::http("w", "https://w.example")),
            json!({ "type": "remote", "url": "https://w.example" })
        );
    }

    #[test]
    fn tool_kinds() {
        for (n, k) in [
            ("bash", ToolKind::Shell),
            ("grep", ToolKind::Read),
            ("apply_patch", ToolKind::Write),
            ("t_run", ToolKind::Other),
        ] {
            assert_eq!(tool_kind(n), k);
        }
    }

    #[test]
    fn the_plugin_names_its_agent() {
        let ts = plugin(
            "kilo",
            "Kilo Code",
            "@kilocode/plugin",
            "t",
            &Integration::new(),
        );
        assert!(ts.contains("import type { Plugin } from \"@kilocode/plugin\";"));
        assert!(ts.contains("on Kilo Code's events"));
        assert!(!ts.contains("__"), "every placeholder filled: {ts}");
    }
}
