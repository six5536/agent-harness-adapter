//! The JSON glue the language bindings share (REQ-BND): manifests in,
//! results out, the hook contract, the schemas. Errors are the library's
//! messages, for the binding to raise.
// @zen-component: BND-Glue

use std::path::PathBuf;

use agent_harness_adapter_core::{
    InstallOptions, InstallResult, Scope, fs,
    harness::{self, expand, install as kit_install, installed, status as kit_status},
    hook::{Answer, Event, HookInput, wire},
    manifest::{self, Manifest, ManifestTool},
};
use serde::Deserialize;
pub use serde_json::Value;
use serde_json::json;

/// The manifest format's version the bindings read.
pub const MANIFEST_VERSION: u64 = manifest::VERSION;
/// The hook contract's version the bindings speak.
pub const HOOK_VERSION: u64 = wire::VERSION;

/// Where a manifest comes from.
#[derive(Debug, Clone)]
pub enum Source {
    /// A file: TOML, or JSON when it ends in `.json`.
    Path(PathBuf),
    /// A manifest as JSON; its TEXT files resolve against the working
    /// directory.
    Json(Value),
}

/// The options of [`install`] and [`status`], as the bindings pass them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    /// Harness ids, or `all`.
    pub harnesses: Vec<String>,
    /// `project` (default), `user` or `local`.
    pub scope: Option<String>,
    /// The project directory (default: the working directory).
    pub root: Option<PathBuf>,
    /// The home directory (default: `HOME`, else `USERPROFILE`).
    pub home: Option<PathBuf>,
    /// Overwrite parts edited by hand.
    pub force: bool,
    /// Parts to leave out, replacing those declined before.
    pub without: Option<Vec<String>>,
}

impl Options {
    /// `text` (a JSON object) as options.
    pub fn parse(text: &str) -> Result<Options> {
        serde_json::from_str(text).map_err(|e| format!("options: {e}"))
    }
}

type Result<T> = std::result::Result<T, String>;

/// `text` as JSON.
pub fn value(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|e| format!("manifest: {e}"))
}

fn msg(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// The manifest as a tool, and the scope.
fn tool(source: Source, o: &Options) -> Result<(ManifestTool, Scope)> {
    let cwd = std::env::current_dir().map_err(msg)?;
    let manifest = match source {
        Source::Path(p) => Manifest::load(&p),
        Source::Json(v) => Manifest::from_json(v, &cwd, "manifest"),
    }
    .map_err(msg)?;
    let scope: Scope = o
        .scope
        .as_deref()
        .unwrap_or("project")
        .parse()
        .map_err(msg)?;
    let root = o.root.clone().unwrap_or(cwd);
    let home = o
        .home
        .clone()
        .or_else(fs::home_dir)
        .ok_or("no home directory: set HOME or pass home")?;
    Ok((ManifestTool::new(manifest, root, home), scope))
}

fn to_json(result: &InstallResult) -> Result<Value> {
    serde_json::to_value(result).map_err(msg)
}

/// Install `source`'s integration for `options.harnesses`.
// @zen-impl: BND-1_AC-1
// @zen-impl: BND-1_AC-3
pub fn install(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let mut opts =
        InstallOptions::new(expand(&tool, &options.harnesses, scope), scope).force(options.force);
    if let Some(w) = &options.without {
        opts = opts.without(w.iter().cloned());
    }
    to_json(&kit_install(&tool, &opts).map_err(msg)?)
}

/// The state of `options.harnesses`, or of the installed ones when none
/// is named.
// @zen-impl: BND-1_AC-2
pub fn status(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let ids = if options.harnesses.is_empty() {
        installed(&tool, scope).map_err(msg)?
    } else {
        expand(&tool, &options.harnesses, scope)
    };
    to_json(&kit_status(&tool, ids, scope).map_err(msg)?)
}

fn harness_event(
    harness: &str,
    event: &str,
) -> Result<(std::sync::Arc<dyn harness::Harness>, Event)> {
    let h = harness::find(harness).ok_or_else(|| format!("no harness named `{harness}`"))?;
    let e: Event = event.parse().map_err(msg)?;
    Ok((h, e))
}

/// The hook contract's input for `harness`'s input `text` at `event`.
// @zen-impl: BND-2_AC-1
pub fn parse_hook(harness: &str, event: &str, text: &str) -> Result<Value> {
    let (h, e) = harness_event(harness, event)?;
    let input = HookInput::parse(h.as_ref(), e, text)
        .unwrap_or_else(|_| HookInput::new(h.id(), e, Value::Null));
    Ok(wire::input_json(&input))
}

/// What a hook command writes for `answer` (a contract answer, as JSON
/// text): `{stdout, stderr, exit}`.
// @zen-impl: BND-2_AC-2
// @zen-impl: BND-2_AC-3
pub fn answer_hook(harness: &str, event: &str, answer: &str) -> Result<Value> {
    let (h, e) = harness_event(harness, event)?;
    let answer: Answer = wire::parse_answer(answer).map_err(msg)?;
    let out = h.answer(e, &answer).map_err(msg)?;
    Ok(json!({
        "stdout": format!("{}\n", out.stdout),
        "stderr": out.stderr,
        "exit": out.exit,
    }))
}

/// The JSON Schema of `contract`.
// @zen-impl: BND-3_AC-1
// @zen-impl: BND-3_AC-2
pub fn schema(contract: &str) -> Result<Value> {
    Ok(match contract {
        "manifest" => Manifest::schema(),
        "hook-input" => wire::input_schema(),
        "hook-answer" => wire::answer_schema(),
        "result" => InstallResult::schema(),
        other => {
            return Err(format!(
                "no contract `{other}`: manifest, hook-input, hook-answer or result"
            ));
        }
    })
}

#[cfg(test)]
mod tests;
