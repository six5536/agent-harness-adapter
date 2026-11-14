//! The JSON glue the language bindings share: manifests in,
//! results out, the hook contract, the schemas. Errors are the library's
//! messages, for the binding to raise.

use std::path::PathBuf;

use agent_harness_adapter_core::{
    BlockOrder, InstallOptions, InstallResult, Request, Scope, UninstallOptions, cli,
    follow as kit_follow, fs,
    harness::{
        self, edits as kit_edits, expand, install as kit_install, installed,
        keep_instructions as kit_keep, status as kit_status, uninstall as kit_uninstall,
    },
    hook::{Answer, Event, HookInput, wire},
    manifest::{self, Manifest, ManifestTool, find_config},
};
use serde::Deserialize;
pub use serde_json::Value;
use serde_json::json;

/// The manifest format's version the bindings read.
pub const MANIFEST_VERSION: u64 = manifest::VERSION;

// sokf:begin source-options
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
    /// Overwrite parts edited by hand: `true` for every part not declined,
    /// or the parts named.
    pub force: ForceOption,
}

/// `force` as the bindings pass it: a switch, or a list of part names.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ForceOption {
    /// `true`: every part not declined; `false`: none.
    Switch(bool),
    /// The parts named.
    Parts(Vec<String>),
}

impl Default for ForceOption {
    fn default() -> Self {
        ForceOption::Switch(false)
    }
}
// sokf:end source-options

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
// sokf-implements: BND-1_AC-1
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
// sokf-implements: BND-1_AC-1
// sokf-implements: BND-1_AC-3
pub fn install(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let opts = InstallOptions::new(expand(&tool, &options.harnesses, scope), scope);
    let opts = match &options.force {
        ForceOption::Switch(f) => opts.force(*f),
        ForceOption::Parts(parts) => opts.force_parts(parts.iter().cloned()),
    };
    to_json(&kit_install(&tool, &opts).map_err(msg)?)
}

/// Take `source`'s integration out of `options.harnesses` (`all`: the
/// installed ones).
// sokf-implements: BND-1_AC-4
pub fn uninstall(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let ForceOption::Switch(force) = options.force else {
        return Err("options: uninstall takes `force` as true or false".into());
    };
    let opts = UninstallOptions::new(options.harnesses.iter().cloned(), scope).force(force);
    to_json(&kit_uninstall(&tool, &opts).map_err(msg)?)
}

/// The state of `options.harnesses`, or of the installed ones when none
/// is named.
// sokf-implements: BND-1_AC-2
pub fn status(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let ids = if options.harnesses.is_empty() {
        installed(&tool, scope).map_err(msg)?
    } else {
        expand(&tool, &options.harnesses, scope)
    };
    to_json(&kit_status(&tool, ids, scope).map_err(msg)?)
}

/// The edits of `options.harnesses`, or of the installed ones when none is
/// named: each edited file's text `install` would write and the text found.
// sokf-implements: BND-1_AC-5
pub fn edits(source: Source, options: &Options) -> Result<Value> {
    let (tool, scope) = tool(source, options)?;
    let ids = if options.harnesses.is_empty() {
        installed(&tool, scope).map_err(msg)?
    } else {
        expand(&tool, &options.harnesses, scope)
    };
    serde_json::to_value(kit_edits(&tool, ids, scope).map_err(msg)?).map_err(msg)
}

/// Write `text` as the project's own beside the tool's block in each file of
/// an installed harness's instructions (KIT-27): `{report}`, the report the
/// tool prints as it stands.
/// It takes the installed harnesses, so `harnesses` and `force` are refused.
// sokf-implements: BND-1_AC-6
pub fn keep_instructions(source: Source, text: &str, options: &Options) -> Result<Value> {
    if !options.harnesses.is_empty() {
        return Err(
            "options: keep_instructions takes the installed harnesses: give no `harnesses`".into(),
        );
    }
    if !matches!(&options.force, ForceOption::Switch(false))
        && options.force != ForceOption::Parts(vec![])
    {
        return Err("options: keep_instructions takes no `force`".into());
    }
    let (tool, scope) = tool(source, options)?;
    let kept = kit_keep(&tool, text, scope).map_err(msg)?;
    Ok(json!({ "report": kept.to_text() }))
}

/// The options of [`follow`], as the bindings pass them.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FollowOptions {
    /// The tool's instructions; none takes its block out.
    pub instructions: Option<String>,
    /// A text the project keeps as its own beside the block.
    pub keep: Option<String>,
    /// `installed` (default) or `last`.
    pub block_order: Option<BlockOrder>,
    /// `project` (default), `user` or `local`.
    pub scope: Option<String>,
    /// The project directory (default: the nearest CONFIG's project above
    /// the working directory, else the git repository's root, else the
    /// working directory).
    pub root: Option<PathBuf>,
    /// The home directory (default: `HOME`, else `USERPROFILE`).
    pub home: Option<PathBuf>,
}

/// The library's `follow` (KIT-28): `{report, exit}`; a refusal, or options
/// that do not parse, is a report and exit 2, never an error.
// sokf-implements: BND-1_AC-7
pub fn follow(tool: &str, options: &str) -> Value {
    let refused =
        |e: String| json!({ "report": format!("error: {e}\n"), "exit": cli::EXIT_FAILURE });
    let o: FollowOptions = match serde_json::from_str(options) {
        Ok(o) => o,
        Err(e) => return refused(format!("options: {e}")),
    };
    let scope: Scope = match o.scope.as_deref().unwrap_or("project").parse() {
        Ok(s) => s,
        Err(e) => return refused(msg(e)),
    };
    let mut r = Request::new(tool, scope).block_order(o.block_order.unwrap_or_default());
    if let Some(t) = o.instructions {
        r = r.instructions(t);
    }
    if let Some(k) = o.keep {
        r = r.keep(k);
    }
    if let Some(d) = o.root {
        r = r.root(d);
    }
    if let Some(d) = o.home {
        r = r.home(d);
    }
    let out = kit_follow(&r);
    json!({ "report": out.report, "exit": out.exit })
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
// sokf-implements: BND-2_AC-1
pub fn parse_hook(harness: &str, event: &str, text: &str) -> Result<Value> {
    let (h, e) = harness_event(harness, event)?;
    let input = HookInput::parse(h.as_ref(), e, text)
        .unwrap_or_else(|_| HookInput::new(h.id(), e, Value::Null));
    Ok(wire::input_json(h.as_ref(), &input))
}

// sokf:begin answer-hook
/// What a hook command writes for `answer` (Claude Code's answer JSON, as
/// text, read as exit 0): `{stdout, stderr, exit}`. Claude Code gets it as
/// it is; another harness its own form, with a note for what it cannot say.
// sokf-implements: BND-2_AC-2
// sokf-implements: BND-2_AC-3
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
// sokf:end answer-hook

/// The JSON Schema of `contract`.
// sokf-implements: BND-3_AC-1
// sokf-implements: BND-3_AC-2
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
