//! Hook events and tool calls, the same for every harness.

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Error;

/// A hook event. Each harness installs it under its own name and leaves out
/// the events it lacks.
// @zen-impl: KIT-11_AC-1
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Event {
    /// A session starts (or resumes).
    SessionStart,
    /// A session ends.
    SessionEnd,
    /// The user submits a prompt.
    PromptSubmit,
    /// Before a tool runs.
    PreTool,
    /// After a tool ran.
    PostTool,
    /// The agent is about to stop.
    Stop,
    /// Before the context is compacted.
    PreCompact,
}

impl Event {
    /// Every event, in this order.
    pub const ALL: [Event; 7] = [
        Event::SessionStart,
        Event::SessionEnd,
        Event::PromptSubmit,
        Event::PreTool,
        Event::PostTool,
        Event::Stop,
        Event::PreCompact,
    ];

    /// The event's name, as `{event}` in a hook command takes it, e.g.
    /// `pre-tool`.
    pub fn as_str(self) -> &'static str {
        match self {
            Event::SessionStart => "session-start",
            Event::SessionEnd => "session-end",
            Event::PromptSubmit => "prompt-submit",
            Event::PreTool => "pre-tool",
            Event::PostTool => "post-tool",
            Event::Stop => "stop",
            Event::PreCompact => "pre-compact",
        }
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Event {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Event::ALL
            .into_iter()
            .find(|e| e.as_str() == s)
            .ok_or_else(|| Error::UnknownEvent {
                name: s.to_string(),
            })
    }
}

/// What a tool call does, from the harness's tool name.
// @zen-impl: KIT-11_AC-6
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum ToolKind {
    /// Runs a shell command.
    Shell,
    /// Reads or searches files.
    Read,
    /// Creates or edits a file.
    Write,
    /// Calls an MCP server's tool.
    Mcp,
    /// Anything else.
    Other,
}

/// The tool call a hook is about.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ToolCall {
    /// The harness's name for the tool, e.g. `Bash`.
    pub name: String,
    /// What it does.
    pub kind: ToolKind,
    /// Its input, as the harness gives it.
    #[serde(default)]
    pub input: Value,
}

impl ToolCall {
    /// A tool call.
    pub fn new(name: impl Into<String>, kind: ToolKind, input: Value) -> Self {
        ToolCall {
            name: name.into(),
            kind,
            input,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_parse_and_print() {
        for e in Event::ALL {
            assert_eq!(e.as_str().parse::<Event>().unwrap(), e);
            assert_eq!(e.to_string(), e.as_str());
            assert_eq!(
                serde_json::to_value(e).unwrap(),
                serde_json::json!(e.as_str())
            );
        }
        assert_eq!(
            "nope".parse::<Event>().unwrap_err().to_string(),
            "no hook event named `nope`"
        );
        assert_eq!(
            serde_json::to_value(ToolKind::Shell).unwrap(),
            serde_json::json!("shell")
        );
        let c = ToolCall::new("Bash", ToolKind::Shell, serde_json::json!({}));
        assert_eq!(c.name, "Bash");
    }
}
