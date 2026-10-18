//! The JSON glue the language bindings share (REQ-BND): manifests in,
//! results out, the hook contract, the schemas. Errors are the library's
//! messages, for the binding to raise.
// @zen-component: BND-Glue

use std::path::PathBuf;

use agent_harness_adapter_core::{
    InstallOptions, InstallResult, Scope, UninstallOptions, fs,
    harness::{
        self, expand, install as kit_install, installed, status as kit_status,
        uninstall as kit_uninstall,
    },
    hook::{Answer, Event, HookInput, wire},
    manifest::{self, Manifest, ManifestTool, find_config},
};
use serde::Deserialize;
pub use serde_json::Value;
use serde_json::json;

/// The manifest format's version the bindings read.
pub const MANIFEST_VERSION: u64 = manifest::VERSION;

/// Where a manifest comes from.
#[derive(Debug, Clone)]
pub enum Source {
    /// A file: TOML, or JSON when it ends in `.json`.
    Path(PathBuf),
    /// A manifest as JSON; its TEXT files and SOURCES resolve against the
    /// working directory.
    Json(Value),
    /// The CONFIG, found as `agent-harness-adapter` finds it (AHA-7).
    Config,
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
// @zen-impl: BND-1_AC-1
fn tool(source: Source, o: &Options) -> Result<(ManifestTool, Scope)> {
    let cwd = std::env::current_dir().map_err(msg)?;
    let scope: Scope = o
        .scope
        .as_deref()
        .unwrap_or("project")
        .parse()
        .map_err(msg)?;
    let home = o
        .home
        .clone()
        .or_else(fs::home_dir)
        .ok_or("no home directory: set HOME or pass home")?;
    let (manifest, root) = match source {
        Source::Path(p) => (Manifest::load(&p), o.root.clone().unwrap_or(cwd)),
        Source::Json(v) => (
            Manifest::from_json(v, &cwd, "manifest"),
            o.root.clone().unwrap_or(cwd),
        ),
        Source::Config => {
            let found = find_config(scope, o.root.as_deref(), &cwd, &home)
                .map_err(msg)?
                .ok_or("no config: run agent-harness-adapter install")?;
            let root = match scope {
                Scope::User => o.root.clone().unwrap_or(cwd),
                _ => found.root.clone(),
            };
            (Manifest::load_in(&found.path, &found.root), root)
        }
    };
    Ok((ManifestTool::new(manifest.map_err(msg)?, root, home), scope))
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

/// Take `source`'s integration out of `options.harnesses` (`all`: the
/// installed ones).
// @zen-impl: BND-1_AC-4
pub fn uninstall(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let opts = UninstallOptions::new(options.harnesses.iter().cloned(), scope).force(options.force);
    to_json(&kit_uninstall(&tool, &opts).map_err(msg)?)
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
    Ok(wire::input_json(h.as_ref(), &input))
}

/// What a hook command writes for `answer` (Claude Code's answer JSON, as
/// text, read as exit 0): `{stdout, stderr, exit}`. Claude Code gets it as
/// it is; another harness its own form, with a note for what it cannot say.
// @zen-impl: BND-2_AC-2
// @zen-impl: BND-2_AC-3
pub fn answer_hook(harness: &str, event: &str, answer: &str) -> Result<Value> {
    let (h, e) = harness_event(harness, event)?;
    let value: Value = serde_json::from_str(answer).map_err(|e| format!("hook answer: {e}"))?;
    if !value.is_object() {
        return Err("hook answer: not a JSON object".into());
    }
    if h.id() == "claude" {
        return Ok(json!({ "stdout": format!("{value}\n"), "stderr": null, "exit": 0 }));
    }
    let reply = wire::parse_answer(e, 0, &value.to_string(), "");
    let mut notes: Vec<String> = reply
        .dropped
        .iter()
        .map(|d| format!("{d} is not supported by {}\n", h.id()))
        .collect();
    let out = h.answer(e, &reply.answer).or_else(|_| {
        notes.push(format!(
            "{} at {e} is not supported by {}\n",
            reply.answer.as_str(),
            h.id()
        ));
        h.answer(e, &Answer::Allow { stderr: None })
    });
    let out = out.map_err(msg)?;
    let stderr: String = notes.concat() + out.stderr.as_deref().unwrap_or_default();
    Ok(json!({
        "stdout": format!("{}\n", out.stdout),
        "stderr": (!stderr.is_empty()).then_some(stderr),
        "exit": out.exit,
    }))
}

/// The JSON Schema of `contract`.
// @zen-impl: BND-3_AC-1
// @zen-impl: BND-3_AC-2
pub fn schema(contract: &str) -> Result<Value> {
    Ok(match contract {
        "manifest" => Manifest::schema(),
        "hooks" => agent_harness_adapter_core::hook::file::schema(),
        "hook-input" => wire::input_schema(),
        "hook-answer" => wire::answer_schema(),
        "result" => InstallResult::schema(),
        other => {
            return Err(format!(
                "no contract `{other}`: manifest, hooks, hook-input, hook-answer or result"
            ));
        }
    })
}

#[cfg(test)]
mod tests;
