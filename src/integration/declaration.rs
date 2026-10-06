//! A tool's integration, declared once without naming a harness.
// @zen-component: KIT-Integration

use std::{fmt, str::FromStr};

use serde::Serialize;

use crate::{
    Error, Result,
    harness::{EntryMatch, Part},
    integration::{Agent, Command, Hook, McpServer, Skill},
};

/// One kind of thing in an integration. An item becomes at most one part
/// per harness, named by the item's name (e.g. `hooks`).
// @zen-impl: KIT-17_AC-2
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Item {
    /// The instructions block.
    Instructions,
    /// Skills.
    Skills,
    /// Hooks.
    Hooks,
    /// MCP servers.
    Mcp,
    /// Allowed commands.
    Permissions,
    /// Subagents.
    Agents,
    /// Slash commands.
    Commands,
}

impl Item {
    /// Every item, in profile order.
    pub const ALL: [Item; 7] = [
        Item::Instructions,
        Item::Skills,
        Item::Hooks,
        Item::Mcp,
        Item::Permissions,
        Item::Agents,
        Item::Commands,
    ];

    /// The item's name, which is its part's name, e.g. `hooks`.
    pub fn as_str(self) -> &'static str {
        match self {
            Item::Instructions => "instructions",
            Item::Skills => "skills",
            Item::Hooks => "hooks",
            Item::Mcp => "mcp",
            Item::Permissions => "permissions",
            Item::Agents => "agents",
            Item::Commands => "commands",
        }
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Item {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, ()> {
        Item::ALL.into_iter().find(|i| i.as_str() == s).ok_or(())
    }
}

/// What a tool installs into every harness, built with the methods below.
/// Harnesses read it through the accessors and render it into their own
/// files.
// @zen-impl: KIT-17_AC-1
#[derive(Debug, Clone, Default)]
pub struct Integration {
    instructions: Option<String>,
    skills: Vec<Skill>,
    hooks: Vec<Hook>,
    hook_match: Option<EntryMatch>,
    mcp_servers: Vec<McpServer>,
    allowed: Vec<String>,
    agents: Vec<Agent>,
    commands: Vec<Command>,
    parts: Vec<(String, Part)>,
}

impl Integration {
    /// An empty integration.
    pub fn new() -> Self {
        Self::default()
    }

    /// With an instructions block, placed between the tool's markers in the
    /// file each harness reads.
    #[must_use]
    pub fn instructions(mut self, block: impl Into<String>) -> Self {
        self.instructions = Some(block.into());
        self
    }

    /// With a skill.
    #[must_use]
    pub fn skill(mut self, skill: Skill) -> Self {
        self.skills.push(skill);
        self
    }

    /// With a hook.
    #[must_use]
    pub fn hook(mut self, hook: Hook) -> Self {
        self.hooks.push(hook);
        self
    }

    /// The hook entries that are the tool's: those whose command `owned`
    /// matches, for every hook without its own ([`Hook::owned`]).
    #[must_use]
    pub fn hook_match(mut self, owned: EntryMatch) -> Self {
        self.hook_match = Some(owned);
        self
    }

    /// With an MCP server.
    #[must_use]
    pub fn mcp_server(mut self, server: McpServer) -> Self {
        self.mcp_servers.push(server);
        self
    }

    /// Allow the agent to run commands starting with `prefix` (e.g. the
    /// tool's program) without asking.
    #[must_use]
    pub fn allow_command(mut self, prefix: impl Into<String>) -> Self {
        self.allowed.push(prefix.into());
        self
    }

    /// With a subagent.
    #[must_use]
    pub fn agent(mut self, agent: Agent) -> Self {
        self.agents.push(agent);
        self
    }

    /// With a slash command.
    #[must_use]
    pub fn command(mut self, command: Command) -> Self {
        self.commands.push(command);
        self
    }

    /// With a raw part for the harness `harness` only, after its items.
    #[must_use]
    pub fn part(mut self, harness: impl Into<String>, part: Part) -> Self {
        self.parts.push((harness.into(), part));
        self
    }

    /// The instructions block.
    pub fn instructions_block(&self) -> Option<&str> {
        self.instructions.as_deref()
    }

    /// The skills.
    pub fn skills(&self) -> &[Skill] {
        &self.skills
    }

    /// The hooks.
    pub fn hooks(&self) -> &[Hook] {
        &self.hooks
    }

    /// The hook entries that are `hook`'s in a harness's file: its own match,
    /// else the integration's, else a prefix: the command's text before its
    /// first placeholder (the whole command when it has none). A command that
    /// starts with a placeholder needs a match: an empty prefix would own
    /// every entry.
    // @zen-impl: KIT-11_AC-7
    // @zen-impl: KIT-11_AC-8
    pub fn hook_owner(&self, hook: &Hook) -> Result<EntryMatch> {
        if let Some(m) = hook.owned_by().or(self.hook_match.as_ref()) {
            return Ok(m.clone());
        }
        let prefix = hook.prefix();
        if prefix.is_empty() {
            return Err(Error::Internal(format!(
                "the hook command `{}` starts with a placeholder: give it an entry match",
                hook.template()
            )));
        }
        Ok(EntryMatch::Prefix(prefix))
    }

    /// The MCP servers.
    pub fn mcp_servers(&self) -> &[McpServer] {
        &self.mcp_servers
    }

    /// The allowed command prefixes.
    pub fn allowed_commands(&self) -> &[String] {
        &self.allowed
    }

    /// The subagents.
    pub fn agents(&self) -> &[Agent] {
        &self.agents
    }

    /// The slash commands.
    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    /// The items it holds, in profile order.
    pub fn items(&self) -> Vec<Item> {
        Item::ALL
            .into_iter()
            .filter(|i| match i {
                Item::Instructions => self.instructions.is_some(),
                Item::Skills => !self.skills.is_empty(),
                Item::Hooks => !self.hooks.is_empty(),
                Item::Mcp => !self.mcp_servers.is_empty(),
                Item::Permissions => !self.allowed.is_empty(),
                Item::Agents => !self.agents.is_empty(),
                Item::Commands => !self.commands.is_empty(),
            })
            .collect()
    }

    /// The raw parts for `harness`, in order.
    pub fn parts_for<'a>(&'a self, harness: &'a str) -> impl Iterator<Item = &'a Part> + 'a {
        self.parts
            .iter()
            .filter(move |(h, _)| h == harness)
            .map(|(_, p)| p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::Event;

    #[test]
    fn items_parse_and_print() {
        for i in Item::ALL {
            assert_eq!(i.as_str().parse::<Item>(), Ok(i));
            assert_eq!(i.to_string(), i.as_str());
        }
        assert!("nope".parse::<Item>().is_err());
    }

    // @zen-test: KIT-17_AC-1
    #[test]
    fn an_integration_holds_its_items_in_profile_order() {
        assert!(Integration::new().items().is_empty());
        let i = Integration::new()
            .command(Command::new("c", "d", "p"))
            .instructions("b\n")
            .skill(Skill::new("s", "d", "b"))
            .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
            .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
            .allow_command("t")
            .agent(Agent::new("a", "d", "p"))
            .part("claude", Part::region("notes", "NOTES.md", "n"));
        assert_eq!(i.items(), Item::ALL);
        assert_eq!(i.instructions_block(), Some("b\n"));
        assert_eq!(
            (
                i.skills().len(),
                i.hooks().len(),
                i.mcp_servers().len(),
                i.allowed_commands(),
                i.agents().len(),
                i.commands().len()
            ),
            (1, 1, 1, &["t".to_string()][..], 1, 1)
        );
        assert_eq!(i.parts_for("claude").count(), 1);
        assert_eq!(i.parts_for("codex").count(), 0);
    }

    // @zen-test: KIT-11_AC-7
    // @zen-test: KIT-11_AC-8
    #[test]
    fn a_hook_is_owned_by_its_match_the_integrations_or_its_prefix() {
        let plain = Hook::new(Event::Stop, "t hook {harness} {event}");
        let first = Hook::new(Event::Stop, "{harness} t");
        let own = first.clone().owned(EntryMatch::Contains(" t".into()));
        let i = Integration::new();
        assert_eq!(
            i.hook_owner(&plain).unwrap(),
            EntryMatch::Prefix("t hook ".into())
        );
        let e = i.hook_owner(&first).unwrap_err();
        assert!(matches!(e, Error::Internal(_)), "{e}");
        assert_eq!(
            i.hook_owner(&own).unwrap(),
            EntryMatch::Contains(" t".into())
        );
        let i = i.hook_match(EntryMatch::Contains("t".into()));
        assert_eq!(
            i.hook_owner(&first).unwrap(),
            EntryMatch::Contains("t".into())
        );
        assert_eq!(
            i.hook_owner(&own).unwrap(),
            EntryMatch::Contains(" t".into())
        );
    }
}
