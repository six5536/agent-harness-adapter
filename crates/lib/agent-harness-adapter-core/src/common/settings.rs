//! What Gemini CLI and its fork Qwen Code share in their settings: the
//! context files `context.fileName` names, and MCP entries with an http
//! server's URL as `httpUrl`.

use serde_json::{Value, json};

use crate::{
    Error, Result,
    fs::read_text,
    harness::{Context, Scope},
    integration::{McpServer, Transport},
};

/// Where an agent's context files are set and found.
pub(crate) struct ContextLayout {
    /// The settings file, relative to the scope's root.
    pub(crate) settings: &'static str,
    /// The user's settings file, as errors name it.
    pub(crate) user_display: &'static str,
    /// The directory of the context files at user scope, with a trailing `/`.
    pub(crate) user_dir: &'static str,
    /// The context files when no settings name them.
    pub(crate) default: &'static [&'static str],
}

/// The context file names `text` lists in `context.fileName` (a string or
/// an array); `None` when it does not set it. A file that does not parse is
/// a refusal naming `display`.
fn file_names(text: Option<String>, display: &str) -> Result<Option<Vec<String>>> {
    let Some(text) = text else {
        return Ok(None);
    };
    let doc: Value = serde_json::from_str(&text)
        .map_err(|e| Error::file(display, format!("does not parse as JSON: {e}")))?;
    Ok(match &doc["context"]["fileName"] {
        Value::String(s) => Some(vec![s.clone()]),
        Value::Array(a) => Some(
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
        )
        .filter(|v: &Vec<String>| !v.is_empty()),
        _ => None,
    })
}

/// The context files the agent loads at `cx`: the scope's setting, else (at
/// project scope) the user's, else the default; and the one the
/// instructions go in: `AGENTS.md` when listed, else the first.
pub(crate) fn context_files(cx: &Context, layout: &ContextLayout) -> Result<(String, Vec<String>)> {
    let mut names = file_names(read_text(&cx.path(layout.settings))?, layout.settings)?;
    if names.is_none() && cx.scope == Scope::Project {
        if let Some(path) = cx.user_path(layout.settings) {
            names = file_names(read_text(&path)?, layout.user_display)?;
        }
    }
    let names = names.unwrap_or_else(|| layout.default.iter().map(|n| (*n).to_string()).collect());
    let dir = if cx.scope == Scope::User {
        layout.user_dir
    } else {
        ""
    };
    let all: Vec<String> = names.iter().map(|n| format!("{dir}{n}")).collect();
    let chosen = names
        .iter()
        .position(|n| n == "AGENTS.md")
        .map_or_else(|| all[0].clone(), |i| all[i].clone());
    Ok((chosen, all))
}

/// An MCP server with an http server's URL as `httpUrl`.
pub(crate) fn mcp_entry(s: &McpServer) -> Value {
    match s.transport() {
        Transport::Stdio { .. } => s.to_json(),
        Transport::Http { url, headers } => {
            let mut v = json!({ "httpUrl": url });
            if !headers.is_empty() {
                v["headers"] = s.to_json()["headers"].clone();
            }
            v
        }
    }
}
