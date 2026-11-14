//! The Node addon: the bindings' glue with strings across the boundary;
//! `bindings/packages/agent-harness-adapter-node/index.js` makes it idiomatic.
#![expect(
    clippy::needless_pass_by_value,
    reason = "napi converts JavaScript strings into owned values and rejects `&str`; the \
              expectation sits on the crate, as `#[napi]` copies a function's attributes onto \
              its generated wrapper, where the lint never fires"
)]

use agent_harness_adapter_bind::{self as bind, Options, Source};
use napi::{Error, Result};
use napi_derive::napi;

fn err(e: String) -> Error {
    Error::from_reason(e)
}

fn source(path: Option<String>, json: Option<String>) -> Result<Source> {
    match (path, json) {
        (Some(p), None) => Ok(Source::Path(p.into())),
        (None, Some(j)) => bind::value(&j).map(Source::Json).map_err(err),
        (None, None) => Ok(Source::Config),
        _ => Err(err("give a manifest path or a manifest, not both".into())),
    }
}

/// Install: `options` is JSON; returns the result as JSON.
// sokf-implements: BND-1_AC-3
#[napi]
pub fn install(path: Option<String>, json: Option<String>, options: String) -> Result<String> {
    let o = Options::parse(&options).map_err(err)?;
    bind::install(source(path, json)?, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// Uninstall: `options` is JSON; returns the result as JSON.
#[napi]
pub fn uninstall(path: Option<String>, json: Option<String>, options: String) -> Result<String> {
    let o = Options::parse(&options).map_err(err)?;
    bind::uninstall(source(path, json)?, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// Status: `options` is JSON; returns the result as JSON.
#[napi]
pub fn status(path: Option<String>, json: Option<String>, options: String) -> Result<String> {
    let o = Options::parse(&options).map_err(err)?;
    bind::status(source(path, json)?, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// Edits: `options` is JSON; returns each edited file's texts as JSON.
#[napi]
pub fn edits(path: Option<String>, json: Option<String>, options: String) -> Result<String> {
    let o = Options::parse(&options).map_err(err)?;
    bind::edits(source(path, json)?, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// Keep `text` as the project's own beside the tool's block: `options` is
/// JSON; returns `{report}`, the report of what it kept, as JSON.
// sokf-implements: BND-1_AC-6
#[napi]
pub fn keep_instructions(
    path: Option<String>,
    json: Option<String>,
    text: String,
    options: String,
) -> Result<String> {
    let o = Options::parse(&options).map_err(err)?;
    bind::keep_instructions(source(path, json)?, &text, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// Follow the project (KIT-28): `options` is JSON; returns `{report, exit}`
/// as JSON.
// sokf-implements: BND-1_AC-7
#[napi]
pub fn follow(tool: String, options: String) -> String {
    bind::follow(&tool, &options).to_string()
}

/// The hook contract's input (Claude Code's), as JSON.
#[napi]
pub fn parse_hook(harness: String, event: String, text: String) -> Result<String> {
    bind::parse_hook(&harness, &event, &text)
        .map(|v| v.to_string())
        .map_err(err)
}

/// What the hook writes for `answer` (Claude Code's answer JSON), as JSON.
#[napi]
pub fn answer_hook(harness: String, event: String, answer: String) -> Result<String> {
    bind::answer_hook(&harness, &event, &answer)
        .map(|v| v.to_string())
        .map_err(err)
}

/// A contract's schema, as JSON.
#[napi]
pub fn schema(contract: String) -> Result<String> {
    bind::schema(&contract).map(|v| v.to_string()).map_err(err)
}

/// The versions: the package's and the manifest format's, as JSON.
#[napi]
pub fn versions() -> String {
    format!(
        r#"{{"version":"{}","manifest":{}}}"#,
        env!("CARGO_PKG_VERSION"),
        bind::MANIFEST_VERSION
    )
}
