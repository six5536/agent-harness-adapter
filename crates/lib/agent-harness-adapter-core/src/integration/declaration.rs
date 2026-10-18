//! A tool's integration, declared once without naming a harness.
// @zen-component: KIT-Integration

use std::{fmt, path::PathBuf, str::FromStr, time::Duration};

use serde::Serialize;

use crate::{
    Error, Result,
    harness::{Context, EntryMatch, Part, Scope},
    hook::{Event, HooksFile, file::DEFAULT_TIMEOUT},
    integration::{Agent, Command, Hook, McpServer, Skill, Source, source},
};

/// The command dispatch hooks run when the integration names none.
pub const ADAPTER: &str = "agent-harness-adapter";

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
    mcp_tools: Vec<(String, String)>,
    agents: Vec<Agent>,
    commands: Vec<Command>,
    parts: Vec<(String, Part)>,
    sources: Vec<Source>,
    missing: Vec<Source>,
    warnings: Vec<String>,
    adapter: Option<String>,
    /// The hooks file and, per event it dispatches, the sum of its entries'
    /// timeouts in seconds.
    dispatch: Option<(PathBuf, Vec<(Event, u64)>)>,
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

    /// Allow the agent to call `tool` of the MCP server `server` without
    /// asking.
    #[must_use]
    pub fn allow_mcp_tool(mut self, server: impl Into<String>, tool: impl Into<String>) -> Self {
        self.mcp_tools.push((server.into(), tool.into()));
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

    /// With the instructions read from the file `file`, its SOURCE; a
    /// missing file gives no instructions (KIT-17_AC-8).
    ///
    /// # Errors
    ///
    /// When the file exists and cannot be read as UTF-8 text.
    // @zen-impl: KIT-17_AC-7
    // @zen-impl: KIT-17_AC-8
    pub fn instructions_from(mut self, file: impl Into<PathBuf>) -> Result<Self> {
        let src = self.source_of(Item::Instructions, file.into());
        if let Some(src) = src {
            self.instructions = Some(super::skill::read(&src)?);
        }
        Ok(self)
    }

    /// With the skills of the directory `dir`, their SOURCE: each directory
    /// in it holding a `SKILL.md`.
    ///
    /// # Errors
    ///
    /// When a skill directory cannot be read (KIT-17_AC-6).
    // @zen-impl: KIT-17_AC-9
    pub fn skills_from(mut self, dir: impl Into<PathBuf>) -> Result<Self> {
        if let Some(src) = self.source_of(Item::Skills, dir.into()) {
            self.skills.extend(source::skills(&src)?);
        }
        Ok(self)
    }

    /// With the hooks of the hooks file `file` (KIT-24), their SOURCE: one
    /// dispatch hook per event it has an entry for, added for each harness
    /// at install.
    ///
    /// # Errors
    ///
    /// When the file is not a hooks file (KIT-24_AC-4).
    // @zen-impl: KIT-24_AC-2
    // @zen-impl: KIT-24_AC-7
    pub fn hooks_from(mut self, file: impl Into<PathBuf>) -> Result<Self> {
        let Some(src) = self.source_of(Item::Hooks, file.into()) else {
            return Ok(self);
        };
        let hooks = HooksFile::read(&src)?.unwrap_or_default();
        let display = src.display();
        for name in hooks.other_events() {
            self.warnings.push(format!(
                "{display}: `{name}` hooks do not run: agent-harness-adapter runs SessionStart, SessionEnd, UserPromptSubmit, PreToolUse, PostToolUse, Stop and PreCompact"
            ));
        }
        if hooks.other_types() > 0 {
            self.warnings.push(format!(
                "{display}: hooks of another type than `command` do not run"
            ));
        }
        let events = hooks
            .events()
            .into_iter()
            .map(|e| {
                let total = hooks
                    .groups(e)
                    .iter()
                    .flat_map(|g| &g.hooks)
                    .filter(|x| x.dispatched())
                    .map(|x| x.timeout.unwrap_or(DEFAULT_TIMEOUT))
                    .sum();
                (e, total)
            })
            .collect();
        self.dispatch = Some((src, events));
        Ok(self)
    }

    /// With the MCP servers of the MCP JSON file `file`, their SOURCE.
    ///
    /// # Errors
    ///
    /// When the file is not MCP JSON.
    pub fn mcp_servers_from(mut self, file: impl Into<PathBuf>) -> Result<Self> {
        if let Some(src) = self.source_of(Item::Mcp, file.into()) {
            self.mcp_servers.extend(source::mcp_servers(&src)?);
        }
        Ok(self)
    }

    /// With the agents of the directory `dir`, their SOURCE: each
    /// `<name>.md`, kept as written.
    ///
    /// # Errors
    ///
    /// When an agent file has no frontmatter.
    pub fn agents_from(mut self, dir: impl Into<PathBuf>) -> Result<Self> {
        if let Some(src) = self.source_of(Item::Agents, dir.into()) {
            self.agents.extend(source::agents(&src)?);
        }
        Ok(self)
    }

    /// Record `path` as `item`'s SOURCE: its absolute path when it exists,
    /// else `None`, the SOURCE listed as missing.
    fn source_of(&mut self, item: Item, path: PathBuf) -> Option<PathBuf> {
        let src = Source::new(item, path);
        if src.exists() {
            let at = src.path.clone();
            self.sources.retain(|s| s.item != item);
            self.sources.push(src);
            Some(at)
        } else {
            self.missing.push(src);
            None
        }
    }

    /// With `command` as the program dispatch hooks run (default
    /// `agent-harness-adapter`), e.g. an absolute path.
    #[must_use]
    pub fn adapter(mut self, command: impl Into<String>) -> Self {
        self.adapter = Some(command.into());
        self
    }

    /// The SOURCE `item` was read from, when it was.
    pub fn source(&self, item: Item) -> Option<&Source> {
        self.sources.iter().find(|s| s.item == item)
    }

    /// The SOURCES that did not exist (KIT-17_AC-8).
    pub fn missing(&self) -> &[Source] {
        &self.missing
    }

    /// What reading the SOURCES found to warn about (KIT-24_AC-7).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The integration for `cx`: with a dispatch hook per event of its hooks
    /// file, `<adapter> hook --hooks <path> {harness} {event}`, the path
    /// relative to the root (`~/…` at user scope).
    // @zen-impl: KIT-24_AC-2
    // @zen-impl: HAR-14_AC-6
    pub(crate) fn resolve(&self, cx: &Context) -> Integration {
        let mut out = self.clone();
        let Some((file, events)) = &self.dispatch else {
            return out;
        };
        let adapter = self.adapter.as_deref().unwrap_or(ADAPTER);
        let path = match (cx.relative(file), cx.scope) {
            (Some(rel), Scope::User) => format!("~/{rel}"),
            (Some(rel), _) => rel,
            (None, _) => file.display().to_string(),
        };
        let path = if path.contains(|c: char| c.is_whitespace() || "\"'$`\\;&|<>()".contains(c)) {
            format!("\"{}\"", path.replace('"', "\\\""))
        } else {
            path
        };
        let owner = EntryMatch::Prefix(format!("{adapter} hook --hooks "));
        for (event, seconds) in events {
            out.hooks.push(
                Hook::new(
                    *event,
                    format!("{adapter} hook --hooks {path} {{harness}} {{event}}"),
                )
                .timeout(Duration::from_secs(*seconds))
                .owned(owner.clone()),
            );
        }
        out
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
    /// else the integration's, else a prefix of each of its commands (the
    /// hook's and each harness's own): the text before its first placeholder
    /// (the whole command when it has none). A command that starts with a
    /// placeholder needs a match: an empty prefix would own every entry.
    // @zen-impl: KIT-11_AC-7
    // @zen-impl: KIT-11_AC-8
    pub fn hook_owner(&self, hook: &Hook) -> Result<EntryMatch> {
        if let Some(m) = hook.owned_by().or(self.hook_match.as_ref()) {
            return Ok(m.clone());
        }
        let prefixes = hook.prefixes();
        if prefixes.iter().any(String::is_empty) {
            return Err(Error::Internal(format!(
                "the hook command `{}` starts with a placeholder: give it an entry match",
                hook.template()
            )));
        }
        Ok(EntryMatch::any_of(
            prefixes.into_iter().map(EntryMatch::Prefix),
        ))
    }

    /// The MCP servers.
    pub fn mcp_servers(&self) -> &[McpServer] {
        &self.mcp_servers
    }

    /// The allowed command prefixes.
    pub fn allowed_commands(&self) -> &[String] {
        &self.allowed
    }

    /// The allowed MCP tools: (server, tool).
    pub fn allowed_mcp_tools(&self) -> &[(String, String)] {
        &self.mcp_tools
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
                Item::Permissions => !self.allowed.is_empty() || !self.mcp_tools.is_empty(),
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
            .allow_mcp_tool("t", "run")
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
        // A harness's own command adds its prefix.
        let over = plain
            .clone()
            .command_for("claude", "$CLAUDE_PROJECT_DIR/t hook {harness} {event}");
        assert_eq!(
            over.command("claude"),
            "$CLAUDE_PROJECT_DIR/t hook claude stop"
        );
        assert_eq!(over.command("codex"), "t hook codex stop");
        assert_eq!(
            i.hook_owner(&over).unwrap(),
            EntryMatch::Any(vec![
                EntryMatch::Prefix("t hook ".into()),
                EntryMatch::Prefix("$CLAUDE_PROJECT_DIR/t hook ".into())
            ])
        );
        let bad = plain.clone().command_for("pi", "{harness} x");
        assert!(i.hook_owner(&bad).is_err());
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
