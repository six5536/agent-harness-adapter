//! The Node addon: the bindings' glue with strings across the boundary;
//! `packages/agent-harness-kit-node/index.js` makes it idiomatic.
// @zen-component: BND-Node

use agent_harness_kit_bind::{self as bind, Options, Source};
use napi::{Error, Result};
use napi_derive::napi;

fn err(e: String) -> Error {
    Error::from_reason(e)
}

fn source(path: Option<String>, json: Option<String>) -> Result<Source> {
    match (path, json) {
        (Some(p), None) => Ok(Source::Path(p.into())),
        (None, Some(j)) => bind::value(&j).map(Source::Json).map_err(err),
        _ => Err(err("give a manifest path or a manifest".into())),
    }
}

/// Install: `options` is JSON; returns the result as JSON.
// @zen-impl: BND-1_AC-3
#[napi]
pub fn install(path: Option<String>, json: Option<String>, options: String) -> Result<String> {
    let o = Options::parse(&options).map_err(err)?;
    bind::install(source(path, json)?, &o)
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

/// The hook contract's input, as JSON.
#[napi]
pub fn parse_hook(harness: String, event: String, text: String) -> Result<String> {
    bind::parse_hook(&harness, &event, &text)
        .map(|v| v.to_string())
        .map_err(err)
}

/// What the hook writes for `answer` (JSON), as JSON.
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

/// The versions: the package's, the manifest format's and the hook
/// contract's, as JSON.
#[napi]
pub fn versions() -> String {
    format!(
        r#"{{"version":"{}","manifest":{},"hook":{}}}"#,
        env!("CARGO_PKG_VERSION"),
        bind::MANIFEST_VERSION,
        bind::HOOK_VERSION
    )
}
