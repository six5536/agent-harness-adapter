//! The library's own hook format, between a generated extension (Pi's
//! extension, the OpenCode and Kilo Code plugin) and the tool: the
//! extension writes the event as JSON on the command's stdin and reads
//! `{"answer": "allow" | "deny" | "continue" | "context", "reason"?,
//! "text"?}` back.

use serde_json::{Value, json};

use crate::{
    Result,
    common::protocol,
    hook::{Answer, Event, HookInput, Output, ToolCall, ToolKind},
};

/// The input an extension of `harness` wrote: `session_id`,
/// `transcript_path`, `cwd`, `prompt`, `source`, `tool_name`, `tool_input`,
/// `tool_output`, `continuing`; `tool_kind` classifies the tool's name.
pub(crate) fn parse(
    harness: &str,
    event: Event,
    text: &str,
    tool_kind: fn(&str) -> ToolKind,
) -> serde_json::Result<HookInput> {
    let raw = protocol::raw_object(text)?;
    let mut input = HookInput::new(harness, event, raw.clone());
    let text = |k: &str| protocol::text(&raw, k);
    input.session_id = text("session_id");
    input.transcript_path = text("transcript_path");
    input.cwd = text("cwd");
    input.prompt = text("prompt");
    input.source = text("source");
    input.tool = text("tool_name").map(|name| {
        let kind = tool_kind(&name);
        ToolCall::new(
            name,
            kind,
            raw.get("tool_input").cloned().unwrap_or(Value::Null),
        )
    });
    input.tool_output = raw.get("tool_output").cloned();
    input.continuing = raw["continuing"].as_bool().unwrap_or(false);
    Ok(input)
}

/// The answer for an extension of `harness`: allow anywhere; deny before a
/// tool; continue at stop; context at session start, on a prompt and after
/// a tool. Anything else is unsupported.
pub(crate) fn answer(harness: &str, event: Event, answer: &Answer) -> Result<Output> {
    Ok(match (answer, event) {
        (Answer::Allow { stderr }, _) => {
            Output::json(json!({ "answer": "allow" })).stderr(stderr.clone())
        }
        (Answer::Deny { reason }, Event::PreTool) => {
            Output::json(json!({ "answer": "deny", "reason": reason }))
        }
        (Answer::Continue { reason }, Event::Stop) => {
            Output::json(json!({ "answer": "continue", "reason": reason }))
        }
        (Answer::Context { text }, Event::SessionStart | Event::PromptSubmit | Event::PostTool) => {
            Output::json(json!({ "answer": "context", "text": text }))
        }
        _ => return Err(protocol::unsupported(harness, event, answer)),
    })
}

/// The tool name as it may appear in a comment: no line breaks.
pub(crate) fn comment_safe(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

/// `template` with the tool's name and its hooks filled in: `__TOOL_COMMENT__`
/// (the name for a comment), `__TOOL__` (a string literal) and `__HOOKS__`
/// (`[{event, command, timeout?}]`, the timeout in milliseconds).
pub(crate) fn fill(
    template: &str,
    harness: &str,
    tool: &str,
    integration: &crate::integration::Integration,
) -> String {
    let hooks: Vec<Value> = integration
        .hooks()
        .iter()
        .map(|h| {
            let mut spec = json!({ "event": h.event().as_str(), "command": h.command(harness) });
            if let Some(t) = h.timeout_value() {
                spec["timeout"] = json!(u64::try_from(t.as_millis()).unwrap_or(u64::MAX));
            }
            spec
        })
        .collect();
    template
        .replace("__TOOL_COMMENT__", &comment_safe(tool))
        .replace("__TOOL__", &Value::String(tool.to_string()).to_string())
        .replace("__HOOKS__", &Value::Array(hooks).to_string())
}
