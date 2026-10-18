//! The hook contract (AHA-2): Claude Code's hook protocol, in every
//! harness. A hook reads Claude Code's input JSON on stdin (each harness's
//! tool calls named as Claude Code names them, plus `harness` and
//! `tool_kind`) and answers as Claude Code reads an answer: its exit code,
//! stdout and stderr. `agent-harness-adapter hook` and the language
//! bindings speak it, so a hook in any language sees one format.
// @zen-component: KIT-Wire

use serde_json::{Map, Value};

use crate::{
    Result,
    common::protocol,
    harness::Harness,
    hook::{Answer, Event, HookInput},
};

/// Claude Code's name for `event`, as `hook_event_name` and a hooks file
/// give it.
pub fn event_name(event: Event) -> &'static str {
    match event {
        Event::SessionStart => "SessionStart",
        Event::SessionEnd => "SessionEnd",
        Event::PromptSubmit => "UserPromptSubmit",
        Event::PreTool => "PreToolUse",
        Event::PostTool => "PostToolUse",
        Event::Stop => "Stop",
        Event::PreCompact => "PreCompact",
    }
}

/// The event Claude Code names `name`.
pub fn event_from_name(name: &str) -> Option<Event> {
    Event::ALL.into_iter().find(|e| event_name(*e) == name)
}

/// `input`, which `harness` parsed, as the contract's input. For Claude
/// Code, its own JSON with `harness` and `tool_kind` added; for any other
/// harness, Claude Code's fields built from the parsed input, absent values
/// left out.
// @zen-impl: AHA-2_AC-1
// @zen-impl: AHA-2_AC-5
pub fn input_json(harness: &dyn Harness, input: &HookInput) -> Value {
    // The harness that parsed the input: the Cursor CLI runs Claude Code's
    // hooks with its own input, which Claude Code's parser hands to Cursor's.
    let found = (input.harness != harness.id())
        .then(|| crate::harness::find(&input.harness))
        .flatten();
    let harness = found.as_deref().unwrap_or(harness);
    let mut out = match (&input.raw, harness.id()) {
        (Value::Object(raw), "claude") => raw.clone(),
        _ => built(harness, input),
    };
    out.insert("harness".into(), harness.id().into());
    if let Some(tool) = &input.tool {
        out.insert("tool_kind".into(), tool.kind.as_str().into());
    }
    Value::Object(out)
}

/// Claude Code's input fields from `input`.
fn built(harness: &dyn Harness, input: &HookInput) -> Map<String, Value> {
    let mut out = Map::new();
    let mut put = |k: &str, v: Option<Value>| {
        if let Some(v) = v {
            out.insert(k.into(), v);
        }
    };
    let text = |s: &Option<String>| s.clone().map(Value::String);
    put("hook_event_name", input.event.map(|e| event_name(e).into()));
    put("session_id", text(&input.session_id));
    put("transcript_path", text(&input.transcript_path));
    put("cwd", text(&input.cwd));
    put("prompt", text(&input.prompt));
    if let Some(tool) = &input.tool {
        let named = harness.claude_tool(tool);
        put("tool_name", Some(named.name.into()));
        put("tool_input", Some(named.input));
    }
    put("tool_response", input.tool_output.clone());
    put("source", text(&input.source));
    if input.event == Some(Event::Stop) || input.continuing {
        put("stop_hook_active", Some(input.continuing.into()));
    }
    put("last_assistant_message", text(&input.last_message));
    out
}

/// A hook's answer as the contract reads it, and the parts of it the
/// answer model cannot carry (AHA-2_AC-4), by name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Reply {
    /// The answer.
    pub answer: Answer,
    /// What was left out, e.g. `permissionDecision: ask`.
    pub dropped: Vec<&'static str>,
}

/// A hook's exit code, stdout and stderr for `event`, read as Claude Code
/// reads them. Never fails: anything it cannot read allows, with the
/// hook's stderr passed on.
// @zen-impl: AHA-2_AC-2
// @zen-impl: AHA-2_AC-3
// @zen-impl: AHA-2_AC-4
pub fn parse_answer(event: Event, exit: i32, stdout: &str, stderr: &str) -> Reply {
    let err = (!stderr.trim().is_empty()).then(|| stderr.to_string());
    let allow = || Answer::Allow {
        stderr: err.clone(),
    };
    let mut dropped = Vec::new();
    // A block: deny before a tool or a prompt, continue at stop, context
    // after a tool (Claude Code shows it to the model).
    let block = |reason: String| match event {
        Event::PreTool | Event::PromptSubmit => Some(Answer::Deny { reason }),
        Event::Stop => Some(Answer::Continue { reason }),
        Event::PostTool => Some(Answer::Context { text: reason }),
        _ => None,
    };
    if exit == 2 {
        let reason = non_empty(stderr.trim()).unwrap_or("blocked by a hook");
        return Reply {
            answer: block(reason.to_string()).unwrap_or_else(allow),
            dropped,
        };
    }
    if exit != 0 {
        return Reply {
            answer: allow(),
            dropped,
        };
    }
    let out = stdout.trim();
    let Some(Value::Object(obj)) = serde_json::from_str::<Value>(out).ok() else {
        let answer = match (non_empty(out), event) {
            (Some(text), Event::SessionStart | Event::PromptSubmit) => Answer::Context {
                text: text.to_string(),
            },
            _ => allow(),
        };
        return Reply { answer, dropped };
    };
    if obj.get("continue") == Some(&Value::Bool(false)) {
        dropped.push("continue: false");
    }
    if obj
        .get("suppressOutput")
        .is_some_and(|v| v != &Value::Bool(false))
    {
        dropped.push("suppressOutput");
    }
    if obj.get("systemMessage").is_some() {
        dropped.push("systemMessage");
    }
    let specific = obj.get("hookSpecificOutput").and_then(Value::as_object);
    let field = |k: &str| specific.and_then(|s| s.get(k)).and_then(Value::as_str);
    if specific.is_some_and(|s| s.contains_key("updatedInput")) {
        dropped.push("updatedInput");
    }
    let mut answer = None;
    match field("permissionDecision") {
        Some("deny") if event == Event::PreTool => {
            let reason = field("permissionDecisionReason").unwrap_or("denied by a hook");
            answer = Some(Answer::Deny {
                reason: reason.to_string(),
            });
        }
        Some("ask") => dropped.push("permissionDecision: ask"),
        _ => {}
    }
    if answer.is_none() && obj.get("decision").and_then(Value::as_str) == Some("block") {
        let reason = obj.get("reason").and_then(Value::as_str);
        answer = block(reason.unwrap_or("blocked by a hook").to_string());
    }
    if let (None, Some(text)) = (&answer, field("additionalContext")) {
        match event {
            Event::SessionStart | Event::PromptSubmit | Event::PostTool => {
                answer = Some(Answer::Context {
                    text: text.to_string(),
                });
            }
            _ => dropped.push("additionalContext"),
        }
    }
    Reply {
        answer: answer.unwrap_or_else(allow),
        dropped,
    }
}

fn non_empty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

/// `answer` to `event` as Claude Code's answer JSON (exit 0); an answer
/// Claude Code cannot give for the event is an error.
// @zen-impl: AHA-2_AC-6
pub fn answer_json(event: Event, answer: &Answer) -> Result<Value> {
    let out = protocol::answer("claude", event, event_name(event), answer)?;
    Ok(serde_json::from_str(&out.stdout).expect("the Claude answer is JSON"))
}

/// The JSON Schema of the contract's input.
#[cfg(feature = "schemars")]
pub fn input_schema() -> Value {
    let text = serde_json::json!({ "type": "string" });
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "AHA hook input",
        "description": "Claude Code's hook input, in every harness; `harness` and `tool_kind` added.",
        "type": "object",
        "properties": {
            "hook_event_name": { "enum": Event::ALL.map(event_name) },
            "session_id": text, "transcript_path": text, "cwd": text,
            "prompt": { "type": "string", "description": "prompt submit" },
            "tool_name": { "type": "string", "description": "as Claude Code names it, e.g. Bash" },
            "tool_input": { "description": "as Claude Code gives it: `command` for Bash, `file_path` for a file tool" },
            "tool_response": { "description": "post tool" },
            "source": { "type": "string", "description": "session start" },
            "stop_hook_active": { "type": "boolean", "description": "stop: the agent is already continuing for a stop hook" },
            "last_assistant_message": text,
            "harness": { "type": "string", "description": "the harness that ran the hook, e.g. codex" },
            "tool_kind": { "enum": ["shell", "read", "write", "mcp", "other"] },
        },
        "required": ["hook_event_name", "harness"],
    })
}

/// The JSON Schema of the contract's answer JSON (stdout, exit 0).
#[cfg(feature = "schemars")]
pub fn answer_schema() -> Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "AHA hook answer",
        "description": "Claude Code's hook answer on stdout with exit 0. Exit 2 with a reason on stderr blocks; plain stdout at session start and prompt submit is context.",
        "type": "object",
        "properties": {
            "decision": { "const": "block", "description": "block a prompt or a tool, or keep the agent going at stop" },
            "reason": { "type": "string" },
            "hookSpecificOutput": {
                "type": "object",
                "properties": {
                    "hookEventName": { "enum": Event::ALL.map(event_name) },
                    "permissionDecision": { "enum": ["allow", "deny", "ask"] },
                    "permissionDecisionReason": { "type": "string" },
                    "additionalContext": { "type": "string" },
                    "updatedInput": {},
                },
            },
            "continue": { "type": "boolean" },
            "stopReason": { "type": "string" },
            "suppressOutput": { "type": "boolean" },
            "systemMessage": { "type": "string" },
        },
    })
}

#[cfg(test)]
mod tests;
