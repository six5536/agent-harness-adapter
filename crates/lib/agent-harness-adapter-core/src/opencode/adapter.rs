//! OpenCode (`opencode-ai`).
//!
//! Follows `packages/web/src/content/docs/` of
//! <https://github.com/sst/opencode> (`rules.mdx`, `config.mdx`,
//! `skills.mdx`, `agents.mdx`, `commands.mdx`, `mcp-servers.mdx`,
//! `permissions.mdx`, `plugins.mdx`) and its source (`session/instruction.ts`,
//! `config/plugin.ts`, `session/prompt.ts`, `session/tools.ts`,
//! `cli/cmd/run.ts`), version 1.18.35 (checked 2026-10-07).
// @zen-component: HAR-OpenCode

use crate::{
    Result,
    common::{extension, plugin},
    harness::{Context, Harness, Part, PartResult, Reads, Scope},
    hook::{Answer, Event, HookInput, Output},
    integration::{Integration, Item},
};

/// OpenCode (`opencode`).
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenCode;

const ID: &str = "opencode";

/// OpenCode's paths: the project's `.opencode` and `opencode.json`, or
/// `~/.config/opencode` and its `opencode.json`. The instructions file is
/// the first of `AGENTS.md`, `CLAUDE.md` that exists
/// (`.config/opencode/AGENTS.md`, `.claude/CLAUDE.md` at user scope), else
/// the first; OpenCode loads only that one.
// @zen-impl: HAR-10_AC-2
const LAYOUT: plugin::Layout = plugin::Layout {
    harness: ID,
    agent: "OpenCode",
    types: "@opencode-ai/plugin",
    project_dir: ".opencode",
    user_dir: ".config/opencode",
    config: "opencode.json",
    also_config: &[],
};

impl Harness for OpenCode {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-10_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-10_AC-3
    // @zen-impl: HAR-10_AC-5
    // @zen-impl: HAR-10_AC-6
    // @zen-impl: HAR-10_AC-7
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        LAYOUT.render(integration, cx)
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(LAYOUT.reads(item, cx))
    }

    // @zen-impl: HAR-10_AC-4
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        extension::parse(ID, event, text, plugin::tool_kind)
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
