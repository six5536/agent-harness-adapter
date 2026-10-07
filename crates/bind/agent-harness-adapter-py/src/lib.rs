//! The Python extension `agent_harness_adapter._native`: the bindings' glue
//! with strings across the boundary; `agent_harness_adapter/__init__.py` makes
//! it Pythonic.
// @zen-component: BND-Python

use agent_harness_adapter_bind::{self as bind, Options, Source};
use pyo3::{exceptions::PyValueError, prelude::*};

fn err(e: String) -> PyErr {
    PyValueError::new_err(e)
}

fn source(path: Option<String>, json: Option<String>) -> PyResult<Source> {
    match (path, json) {
        (Some(p), None) => Ok(Source::Path(p.into())),
        (None, Some(j)) => serde_json_value(&j).map(Source::Json),
        _ => Err(err("give a manifest path or a manifest".into())),
    }
}

fn serde_json_value(text: &str) -> PyResult<bind::Value> {
    bind::value(text).map_err(err)
}

/// Install: `options` is JSON; returns the result as JSON.
// @zen-impl: BND-1_AC-3
#[pyfunction]
#[pyo3(signature = (path, json, options))]
fn install(path: Option<String>, json: Option<String>, options: &str) -> PyResult<String> {
    let o = Options::parse(options).map_err(err)?;
    bind::install(source(path, json)?, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// Status: `options` is JSON; returns the result as JSON.
#[pyfunction]
#[pyo3(signature = (path, json, options))]
fn status(path: Option<String>, json: Option<String>, options: &str) -> PyResult<String> {
    let o = Options::parse(options).map_err(err)?;
    bind::status(source(path, json)?, &o)
        .map(|v| v.to_string())
        .map_err(err)
}

/// The hook contract's input, as JSON.
#[pyfunction]
fn parse_hook(harness: &str, event: &str, text: &str) -> PyResult<String> {
    bind::parse_hook(harness, event, text)
        .map(|v| v.to_string())
        .map_err(err)
}

/// What the hook writes for `answer` (JSON), as JSON.
#[pyfunction]
fn answer_hook(harness: &str, event: &str, answer: &str) -> PyResult<String> {
    bind::answer_hook(harness, event, answer)
        .map(|v| v.to_string())
        .map_err(err)
}

/// A contract's schema, as JSON.
#[pyfunction]
fn schema(contract: &str) -> PyResult<String> {
    bind::schema(contract).map(|v| v.to_string()).map_err(err)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(install, m)?)?;
    m.add_function(wrap_pyfunction!(status, m)?)?;
    m.add_function(wrap_pyfunction!(parse_hook, m)?)?;
    m.add_function(wrap_pyfunction!(answer_hook, m)?)?;
    m.add_function(wrap_pyfunction!(schema, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("MANIFEST_VERSION", bind::MANIFEST_VERSION)?;
    m.add("HOOK_VERSION", bind::HOOK_VERSION)?;
    Ok(())
}
