//! A hook's input, the same for every harness.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    harness::Harness,
    hook::{Event, ToolCall},
};

/// What a harness sends a hook command, parsed by that harness into one
/// shape. Every field is optional; the raw JSON keeps the rest.
///
/// As JSON (the hook contract's input, AHK-2), an absent value is left out:
/// `continuing` when false and `raw` when null too.
// @zen-impl: KIT-11_AC-3
// @zen-impl: AHK-2_AC-1
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct HookInput {
    /// The harness that ran the hook, e.g. `claude`.
    pub harness: String,
    /// The event, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<Event>,
    /// The session (or conversation) id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// The session transcript's path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transcript_path: Option<String>,
    /// The user's prompt (prompt submit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The tool call (pre tool, post tool).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<ToolCall>,
    /// The tool's output (post tool).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<Value>,
    /// What started the session, e.g. `startup`, `resume`, `clear`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Whether the agent is already continuing because a stop hook asked it
    /// to: Claude's `stop_hook_active`, Cursor's `loop_count` above 0.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub continuing: bool,
    /// The agent's last message (stop), when the harness sends it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    /// The JSON as the harness sent it.
    #[serde(skip_serializing_if = "Value::is_null")]
    pub raw: Value,
}

impl HookInput {
    /// `text` parsed by `harness` for `event`. Input that is not JSON is an
    /// error the caller handles, e.g. with `unwrap_or_default()`.
    pub fn parse(harness: &dyn Harness, event: Event, text: &str) -> serde_json::Result<Self> {
        harness.parse_hook(event, text)
    }

    /// An input of `harness` for `event` holding `raw`, every other field
    /// empty; a harness's parser fills the rest.
    pub fn new(harness: impl Into<String>, event: Event, raw: Value) -> Self {
        HookInput {
            harness: harness.into(),
            event: Some(event),
            raw,
            ..HookInput::default()
        }
    }
}
