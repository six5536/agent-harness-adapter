//! The items of an integration and the standard renderings several
//! harnesses share.

use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::{
    harness::EntryMatch,
    hook::{Event, ToolKind},
};

/// A frontmatter value: as is when plain YAML reads it back unchanged, else
/// double-quoted (as JSON, which YAML reads).
pub(super) fn yaml_scalar(text: &str) -> String {
    let plain = !text.is_empty()
        && !text.starts_with(|c: char| "-?:,[]{}#&*!|>'\"%@` ".contains(c))
        && !text.ends_with(' ')
        && !text.contains(": ")
        && !text.contains(" #")
        && !text.contains('\n');
    if plain {
        text.to_string()
    } else {
        Value::String(text.to_string()).to_string()
    }
}

/// A hook: an event and the command a harness runs for it.
///
/// The command is any command line: `{harness}` and `{event}` anywhere in
/// it, in any order, or not at all, are replaced by the harness id and the
/// event's name; `{{` and `}}` give literal braces.
// @zen-impl: KIT-11_AC-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hook {
    event: Event,
    template: String,
    kind: Option<ToolKind>,
    timeout: Option<Duration>,
    owned: Option<EntryMatch>,
    overrides: Vec<(String, String)>,
}

impl Hook {
    /// Run `command` (a template) on `event`.
    pub fn new(event: Event, command: impl Into<String>) -> Self {
        Hook {
            event,
            template: command.into(),
            kind: None,
            timeout: None,
            owned: None,
            overrides: Vec::new(),
        }
    }

    /// Run `command` (a template) instead for the harness `harness`, e.g. a
    /// path through an environment variable only that harness sets.
    #[must_use]
    pub fn command_for(mut self, harness: impl Into<String>, command: impl Into<String>) -> Self {
        let harness = harness.into();
        self.overrides.retain(|(h, _)| *h != harness);
        self.overrides.push((harness, command.into()));
        self
    }

    /// Only for tool calls of `kind` (pre tool, post tool). A harness that
    /// can match tool names installs it as the matcher; another runs the
    /// hook for every tool, and the hook filters on
    /// [`HookInput::tool`](crate::hook::HookInput::tool).
    #[must_use]
    pub fn tools(mut self, kind: ToolKind) -> Self {
        self.kind = Some(kind);
        self
    }

    /// With a timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// The hook entries that are this hook's in a harness's file: those
    /// whose command `owned` matches, instead of the default (see
    /// [`Integration::hook_owner`](crate::integration::Integration::hook_owner)).
    #[must_use]
    pub fn owned(mut self, owned: EntryMatch) -> Self {
        self.owned = Some(owned);
        self
    }

    /// The event.
    pub fn event(&self) -> Event {
        self.event
    }

    /// The command template as given.
    pub fn template(&self) -> &str {
        &self.template
    }

    /// The template `harness` runs: its own, else the hook's.
    pub fn template_for(&self, harness: &str) -> &str {
        self.overrides
            .iter()
            .find(|(h, _)| h == harness)
            .map_or(&self.template, |(_, t)| t)
    }

    /// The command a harness runs: placeholders filled, braces unescaped.
    pub fn command(&self, harness: &str) -> String {
        fill(self.template_for(harness), harness, self.event.as_str())
    }

    /// The tool kind the hook is limited to.
    pub fn tool_kind(&self) -> Option<ToolKind> {
        self.kind
    }

    /// The timeout.
    pub fn timeout_value(&self) -> Option<Duration> {
        self.timeout
    }

    /// The entry match set on this hook.
    pub fn owned_by(&self) -> Option<&EntryMatch> {
        self.owned.as_ref()
    }

    /// For each template (the hook's, then each harness's own), its text
    /// before its first placeholder, unescaped; the whole template when it
    /// has none.
    pub(crate) fn prefixes(&self) -> Vec<String> {
        std::iter::once(self.template.as_str())
            .chain(self.overrides.iter().map(|(_, t)| t.as_str()))
            .map(prefix)
            .collect()
    }
}

/// `template`'s text before its first placeholder, unescaped.
fn prefix(template: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("{{") {
            out.push('{');
            rest = r;
        } else if let Some(r) = rest.strip_prefix("}}") {
            out.push('}');
            rest = r;
        } else if rest.starts_with("{harness}") || rest.starts_with("{event}") {
            break;
        } else {
            let c = rest.chars().next().expect("not empty");
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// `template` with `{harness}` and `{event}` replaced and `{{` / `}}`
/// unescaped; any other brace is kept as written.
fn fill(template: &str, harness: &str, event: &str) -> String {
    let mut out = String::new();
    let mut rest = template;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("{{") {
            out.push('{');
            rest = r;
        } else if let Some(r) = rest.strip_prefix("}}") {
            out.push('}');
            rest = r;
        } else if let Some(r) = rest.strip_prefix("{harness}") {
            out.push_str(harness);
            rest = r;
        } else if let Some(r) = rest.strip_prefix("{event}") {
            out.push_str(event);
            rest = r;
        } else {
            let c = rest.chars().next().expect("not empty");
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// How an MCP server is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Transport {
    /// A local process on stdin / stdout.
    Stdio {
        /// The program.
        command: String,
        /// Its arguments.
        args: Vec<String>,
        /// Environment variables.
        env: Vec<(String, String)>,
    },
    /// A remote server over HTTP.
    Http {
        /// The URL.
        url: String,
        /// HTTP headers.
        headers: Vec<(String, String)>,
    },
}

/// An MCP server the tool provides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServer {
    name: String,
    transport: Transport,
}

impl McpServer {
    /// A server named `name` that runs `command` with `args`.
    pub fn stdio<I: IntoIterator<Item = S>, S: Into<String>>(
        name: impl Into<String>,
        command: impl Into<String>,
        args: I,
    ) -> Self {
        McpServer {
            name: name.into(),
            transport: Transport::Stdio {
                command: command.into(),
                args: args.into_iter().map(Into::into).collect(),
                env: Vec::new(),
            },
        }
    }

    /// A server named `name` at `url`.
    pub fn http(name: impl Into<String>, url: impl Into<String>) -> Self {
        McpServer {
            name: name.into(),
            transport: Transport::Http {
                url: url.into(),
                headers: Vec::new(),
            },
        }
    }

    /// With an environment variable (a stdio server; ignored for http).
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Transport::Stdio { env, .. } = &mut self.transport {
            env.push((key.into(), value.into()));
        }
        self
    }

    /// With an HTTP header (an http server; ignored for stdio).
    #[must_use]
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        if let Transport::Http { headers, .. } = &mut self.transport {
            headers.push((key.into(), value.into()));
        }
        self
    }

    /// The server's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// How it is reached.
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// The common `mcpServers` entry: `{command, args, env?}` or `{url,
    /// headers?}`; empty `env` and `headers` are left out.
    pub fn to_json(&self) -> Value {
        match &self.transport {
            Transport::Stdio { command, args, env } => {
                let mut v = json!({ "command": command, "args": args });
                if !env.is_empty() {
                    v["env"] = pairs(env);
                }
                v
            }
            Transport::Http { url, headers } => {
                let mut v = json!({ "url": url });
                if !headers.is_empty() {
                    v["headers"] = pairs(headers);
                }
                v
            }
        }
    }
}

/// Key-value pairs as a JSON object, in order.
fn pairs(kv: &[(String, String)]) -> Value {
    Value::Object(
        kv.iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect::<Map<_, _>>(),
    )
}

/// A subagent: a name, a description and its system prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    name: String,
    description: String,
    prompt: String,
}

impl Agent {
    /// An agent.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Agent {
            name: name.into(),
            description: description.into(),
            prompt: prompt.into(),
        }
    }

    /// The agent's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The agent's description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The agent's system prompt.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The common Markdown agent: frontmatter `name`, `description`, then
    /// `extra` (key, value), then the prompt as the body.
    pub fn to_markdown(&self, extra: &[(&str, &str)]) -> String {
        let mut out = format!(
            "---\nname: {}\ndescription: {}\n",
            self.name,
            yaml_scalar(&self.description)
        );
        for (k, v) in extra {
            out.push_str(&format!("{k}: {}\n", yaml_scalar(v)));
        }
        out.push_str("---\n\n");
        out.push_str(&self.prompt);
        out
    }
}

/// A slash command: a name, a description and its prompt, where
/// `$ARGUMENTS` stands for what the user typed after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    name: String,
    description: String,
    prompt: String,
}

impl Command {
    /// A command.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Command {
            name: name.into(),
            description: description.into(),
            prompt: prompt.into(),
        }
    }

    /// The command's name, as typed after `/`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The command's description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The command's prompt.
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The common Markdown command: frontmatter `description`, then the
    /// prompt as the body.
    pub fn to_markdown(&self) -> String {
        format!(
            "---\ndescription: {}\n---\n\n{}",
            yaml_scalar(&self.description),
            self.prompt
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yaml_scalars_are_plain_when_safe() {
        assert_eq!(yaml_scalar("Checks things."), "Checks things.");
        for quoted in ["", "a: b", "- x", "x #y", "two\nlines", "'q'", " x", "x "] {
            assert!(yaml_scalar(quoted).starts_with('"'), "{quoted}");
        }
    }

    // @zen-test: KIT-11_AC-2
    #[test]
    fn hook_commands_fill_placeholders_anywhere() {
        let h = Hook::new(Event::Stop, "mytool hook {harness} {event}");
        assert_eq!(h.command("codex"), "mytool hook codex stop");
        assert_eq!(h.prefixes()[0], "mytool hook ");
        let h = Hook::new(Event::PreTool, "guard --event={event} --agent={harness}");
        assert_eq!(h.command("cursor"), "guard --event=pre-tool --agent=cursor");
        assert_eq!(h.prefixes()[0], "guard --event=");
        let h = Hook::new(Event::Stop, "mytool-stop");
        assert_eq!(h.command("pi"), "mytool-stop");
        assert_eq!(h.prefixes()[0], "mytool-stop");
        let h = Hook::new(Event::Stop, "sh -c 'x {{a}} {y}' {event}");
        assert_eq!(h.command("claude"), "sh -c 'x {a} {y}' stop");
        assert_eq!(h.prefixes()[0], "sh -c 'x {a} {y}' ");
        let h = Hook::new(Event::Stop, "{harness}-hook")
            .tools(ToolKind::Shell)
            .timeout(Duration::from_secs(5))
            .owned(EntryMatch::Contains("-hook".into()));
        assert_eq!(h.prefixes()[0], "");
        assert_eq!(h.command("gemini"), "gemini-hook");
        assert_eq!(h.template(), "{harness}-hook");
        assert_eq!(h.tool_kind(), Some(ToolKind::Shell));
        assert_eq!(h.timeout_value(), Some(Duration::from_secs(5)));
        assert_eq!(h.owned_by(), Some(&EntryMatch::Contains("-hook".into())));
        assert_eq!(h.event(), Event::Stop);
        assert_eq!(fill("é{event}", "c", "stop"), "éstop");
    }

    #[test]
    fn mcp_servers_render_the_common_entry() {
        let s = McpServer::stdio("tool", "tool", ["mcp"])
            .env("A", "1")
            .header("ignored", "x");
        assert_eq!(
            s.to_json(),
            json!({"command": "tool", "args": ["mcp"], "env": {"A": "1"}})
        );
        assert_eq!(s.name(), "tool");
        let h = McpServer::http("web", "https://x/mcp")
            .header("Authorization", "Bearer t")
            .env("ignored", "x");
        assert_eq!(
            h.to_json(),
            json!({"url": "https://x/mcp", "headers": {"Authorization": "Bearer t"}})
        );
        assert!(matches!(h.transport(), Transport::Http { .. }));
        assert_eq!(
            McpServer::stdio("t", "t", Vec::<String>::new()).to_json(),
            json!({"command": "t", "args": []})
        );
    }

    #[test]
    fn agents_and_commands_render_markdown() {
        let a = Agent::new("reviewer", "Reviews code.", "You review.\n");
        assert_eq!(
            a.to_markdown(&[("model", "inherit")]),
            "---\nname: reviewer\ndescription: Reviews code.\nmodel: inherit\n---\n\nYou review.\n"
        );
        assert_eq!(
            (a.name(), a.description(), a.prompt()),
            ("reviewer", "Reviews code.", "You review.\n")
        );
        let c = Command::new("check", "Run the check.", "Check $ARGUMENTS.\n");
        assert_eq!(
            c.to_markdown(),
            "---\ndescription: Run the check.\n---\n\nCheck $ARGUMENTS.\n"
        );
        assert_eq!(
            (c.name(), c.description(), c.prompt()),
            ("check", "Run the check.", "Check $ARGUMENTS.\n")
        );
    }
}
