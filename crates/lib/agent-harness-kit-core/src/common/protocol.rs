//! The Claude-family hook protocol: snake_case input fields, exit 0 with a
//! JSON answer. Claude Code, Codex and Factory speak it; Gemini CLI reads
//! the same fields with a few of its own.

use serde_json::{Value, json};

use crate::{
    Error, Result,
    hook::{Answer, Event, HookInput, Output, ToolCall, ToolKind},
};

/// A string field of `raw`.
pub(crate) fn text(raw: &Value, key: &str) -> Option<String> {
    raw.get(key).and_then(Value::as_str).map(str::to_string)
}

/// `text` as a JSON object; any other JSON is an empty object, so a harness
/// that sends something else never fails the hook.
pub(crate) fn raw_object(text: &str) -> serde_json::Result<Value> {
    let raw: Value = serde_json::from_str(text)?;
    Ok(if raw.is_object() { raw } else { json!({}) })
}

/// `text` read as Claude-family input: `session_id`, `transcript_path`,
/// `cwd`, `prompt`, `tool_name` / `tool_input` / `tool_response`, `source`,
/// `stop_hook_active`, and the last message from `last_message_key`.
pub(crate) fn parse(
    harness: &str,
    event: Event,
    text: &str,
    kind: fn(&str) -> ToolKind,
    last_message_key: &str,
) -> serde_json::Result<HookInput> {
    let raw = raw_object(text)?;
    let mut input = HookInput::new(harness, event, raw.clone());
    input.session_id = self::text(&raw, "session_id");
    input.transcript_path = self::text(&raw, "transcript_path");
    input.cwd = self::text(&raw, "cwd");
    input.prompt = self::text(&raw, "prompt");
    input.tool = self::text(&raw, "tool_name").map(|name| {
        let k = kind(&name);
        ToolCall::new(
            name,
            k,
            raw.get("tool_input").cloned().unwrap_or(Value::Null),
        )
    });
    input.tool_output = raw.get("tool_response").cloned();
    input.source = self::text(&raw, "source");
    input.continuing = raw
        .get("stop_hook_active")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    input.last_message = self::text(&raw, last_message_key);
    Ok(input)
}

/// An answer the harness cannot give for an event.
pub(crate) fn unsupported(harness: &str, event: Event, answer: &Answer) -> Error {
    Error::Unsupported {
        harness: harness.to_string(),
        what: format!("answer {} at {event}", answer.as_str()),
    }
}

/// `answer` in Claude Code's form; `name` is the harness's name for
/// `event`.
pub(crate) fn answer(harness: &str, event: Event, name: &str, answer: &Answer) -> Result<Output> {
    let context = |text: &str| {
        Output::json(json!({
            "hookSpecificOutput": { "hookEventName": name, "additionalContext": text }
        }))
    };
    Ok(match (answer, event) {
        (Answer::Allow { stderr }, _) => Output::json(json!({})).stderr(stderr.clone()),
        (Answer::Deny { reason }, Event::PreTool) => Output::json(json!({
            "hookSpecificOutput": {
                "hookEventName": name,
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        })),
        (Answer::Deny { reason }, Event::PromptSubmit)
        | (Answer::Continue { reason }, Event::Stop) => {
            Output::json(json!({ "decision": "block", "reason": reason }))
        }
        (Answer::Context { text }, Event::SessionStart | Event::PromptSubmit | Event::PostTool) => {
            context(text)
        }
        _ => return Err(unsupported(harness, event, answer)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(name: &str) -> ToolKind {
        if name == "Bash" {
            ToolKind::Shell
        } else {
            ToolKind::Other
        }
    }

    #[test]
    fn claude_family_input() {
        let i = parse(
            "claude",
            Event::PreTool,
            r#"{"session_id":"s","cwd":"/p","transcript_path":"/t","tool_name":"Bash","tool_input":{"command":"ls"},"other":1}"#,
            kind,
            "last_assistant_message",
        )
        .unwrap();
        assert_eq!(i.harness, "claude");
        assert_eq!(i.event, Some(Event::PreTool));
        assert_eq!(i.session_id.as_deref(), Some("s"));
        assert_eq!(i.cwd.as_deref(), Some("/p"));
        assert_eq!(i.transcript_path.as_deref(), Some("/t"));
        let tool = i.tool.unwrap();
        assert_eq!((tool.name.as_str(), tool.kind), ("Bash", ToolKind::Shell));
        assert_eq!(tool.input, json!({"command": "ls"}));
        assert_eq!(i.raw["other"], 1);
        let i = parse(
            "claude",
            Event::Stop,
            r#"{"stop_hook_active":true,"last_assistant_message":"done","source":"clear","prompt":"p","tool_response":"r"}"#,
            kind,
            "last_assistant_message",
        )
        .unwrap();
        assert!(i.continuing);
        assert_eq!(i.last_message.as_deref(), Some("done"));
        assert_eq!(i.source.as_deref(), Some("clear"));
        assert_eq!(i.prompt.as_deref(), Some("p"));
        assert_eq!(i.tool_output, Some(json!("r")));
        assert!(parse("c", Event::Stop, "nope", kind, "x").is_err());
        let i = parse("c", Event::Stop, "[]", kind, "x").unwrap();
        assert_eq!(i.raw, json!({}));
    }
}
