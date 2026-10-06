//! A hook's input, the same for every harness.

use serde_json::Value;

use crate::{
    harness::Harness,
    hook::{Event, ToolCall},
};

/// What a harness sends a hook command, parsed by that harness into one
/// shape. Every field is optional; the raw JSON keeps the rest.
// @zen-impl: KIT-11_AC-3
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct HookInput {
    /// The harness that ran the hook, e.g. `claude`.
    pub harness: String,
    /// The event, when known.
    pub event: Option<Event>,
    /// The session (or conversation) id.
    pub session_id: Option<String>,
    /// The working directory.
    pub cwd: Option<String>,
    /// The session transcript's path.
    pub transcript_path: Option<String>,
    /// The user's prompt (prompt submit).
    pub prompt: Option<String>,
    /// The tool call (pre tool, post tool).
    pub tool: Option<ToolCall>,
    /// The tool's output (post tool).
    pub tool_output: Option<Value>,
    /// What started the session, e.g. `startup`, `resume`, `clear`.
    pub source: Option<String>,
    /// Whether the agent is already continuing because a stop hook asked it
    /// to: Claude's `stop_hook_active`, Cursor's `loop_count` above 0.
    pub continuing: bool,
    /// The agent's last message (stop), when the harness sends it.
    pub last_message: Option<String>,
    /// The JSON as the harness sent it.
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
