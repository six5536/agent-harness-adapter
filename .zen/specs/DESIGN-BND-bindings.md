# Design Specification

## Overview

REQ-BND as three crates. `agent-harness-adapter-bind` holds the JSON glue once: manifest sources, options, results as JSON, hook parsing and rendering, schemas. The PyO3 and napi crates only move strings across the boundary; a small Python package and a small JS module turn them into native objects and errors. Nothing here is published to crates.io.

## Architecture

AFFECTED LAYERS: bindings (new), library (unchanged)

### High-Level Architecture

```mermaid
flowchart LR
    Py[agent_harness_adapter (Python)] -->|JSON strings| PyExt[_native: PyO3]
    Js[@six5536/agent-harness-adapter-node] -->|JSON strings| Node[index.node: napi]
    PyExt --> Bind[agent-harness-adapter-bind]
    Node --> Bind
    Bind --> Core[agent-harness-adapter-core: manifest, install / status, hook::wire, schemas]
```

### Module Organization

```
crates/bind/
├── agent-harness-adapter-bind/     the shared glue (publish = false)
│   └── src/lib.rs              Source, Options, install, status, parse_hook, answer_hook, schema
├── agent-harness-adapter-py/       PyPI six5536-agent-harness-adapter
│   ├── Cargo.toml, pyproject.toml (maturin, abi3-py39)
│   ├── src/lib.rs              module agent_harness_adapter._native
│   ├── python/agent_harness_adapter/__init__.py, py.typed
│   └── tests/test_binding.py
└── agent-harness-adapter-node/     npm @six5536/agent-harness-adapter-node
    ├── Cargo.toml, build.rs (napi-build)
    └── src/lib.rs              the addon
packages/agent-harness-adapter-node/            index.js (loader, JSON), index.d.ts, test/
packages/agent-harness-adapter-node-<platform>/ index.node
```

### Architectural Decisions

- JSON STRINGS ACROSS THE BOUNDARY: one conversion, written once in the glue crate and once per wrapper with the language's own JSON. Alternatives: `pythonize` / napi object conversion (new dependencies, a converter per language)
- OUR OWN NODE LOADER: platform packages and a loader like the CLI launcher's, so the npm layout and release steps are one pattern. Alternatives: `@napi-rs/cli`'s generated loader and its release flow
- ANSWER WITHOUT `emit`: `answer_hook` renders through `Harness::answer` and raises on an answer the harness cannot express, where `agent-harness-adapter hook` (a process) prints `error:` and exits 1

## Components and Interfaces

### BND-Glue

Builds a `Manifest` from a path or a JSON value (TEXT files against the working directory), a `ManifestTool` at `root` / `home` (defaults: working directory, `fs::home_dir`), expands `all` (`harness::expand`), defaults status to `harness::installed`, and serialises results.

IMPLEMENTS: BND-1_AC-1, BND-1_AC-2, BND-1_AC-3, BND-2_AC-1, BND-2_AC-2, BND-2_AC-3, BND-3_AC-1, BND-3_AC-2

```rust
pub enum Source { Path(PathBuf), Json(Value) }
#[derive(Deserialize, Default)] pub struct Options { harnesses, scope, root, home, force, without }
pub fn install(source: Source, options: &Options) -> Result<Value, String>;
pub fn status(source: Source, options: &Options) -> Result<Value, String>;
pub fn parse_hook(harness: &str, event: &str, text: &str) -> Result<Value, String>;
pub fn answer_hook(harness: &str, event: &str, answer: &str) -> Result<Value, String>; // {stdout, stderr, exit}
pub fn schema(contract: &str) -> Result<Value, String>;
pub const MANIFEST_VERSION: u64; pub const HOOK_VERSION: u64;
```

### BND-Python

`_native` exposes the glue with `str` in and out and `ValueError` for errors; `agent_harness_adapter/__init__.py` takes `str | os.PathLike | dict` manifests and keyword options, and returns `dict`s.

IMPLEMENTS: BND-1_AC-3, BND-2_AC-4, BND-4_AC-1

```python
def install(manifest, harnesses, *, scope="project", root=None, home=None, force=False, without=None) -> dict
def status(manifest, harnesses=None, *, scope="project", root=None, home=None) -> dict
def parse_hook(harness: str, event: str, text: str) -> dict
def answer_hook(harness: str, event: str, answer: dict) -> dict   # {"stdout", "stderr", "exit"}
def run_hook(harness: str, event: str, decide: Callable[[dict], dict]) -> int
def schema(contract: str) -> dict
__version__: str; MANIFEST_VERSION: int; HOOK_VERSION: int
```

### BND-Node

The addon exposes the glue with strings and JS `Error`s; `index.js` loads the platform package's `index.node` and converts JSON; `index.d.ts` types it.

IMPLEMENTS: BND-1_AC-3, BND-2_AC-4, BND-4_AC-2

```typescript
install(manifest: string | object, harnesses: string[], options?: { scope?, root?, home?, force?, without? }): Result
status(manifest: string | object, harnesses?: string[], options?: { scope?, root?, home? }): Result
parseHook(harness: string, event: string, text: string): HookInput
answerHook(harness: string, event: string, answer: Answer): { stdout: string; stderr: string | null; exit: number }
runHook(harness: string, event: string, decide: (input: HookInput) => Answer | Promise<Answer>): Promise<number>
schema(contract: "manifest" | "hook-input" | "hook-answer" | "result"): object
version: string; MANIFEST_VERSION: number; HOOK_VERSION: number
```

## Data Models

### Core Types

- OPTIONS (JSON across the boundary): `{"harnesses": [..], "scope": "project", "root": null, "home": null, "force": false, "without": null}`
- EMITTED: `{"stdout": "<text>\n", "stderr": null, "exit": 0}`

## Correctness Properties

- BND_P-1 [Same as agent-harness-adapter]: for the same manifest and options, a binding's result equals `agent-harness-adapter --json`'s
  VALIDATES: BND-1_AC-1, BND-1_AC-2

## Error Handling

### Glue errors

A `String`: the library's error message (`Error`'s display), or what is wrong with the options.

### Strategy

PRINCIPLES:

- A binding raises; it never prints or exits

## Testing Strategy

### Property-Based Testing

- FRAMEWORK: none here; BND_P-1 is checked by example on every harness
- MINIMUM_ITERATIONS: 1

### Unit Testing

- AREAS: the glue crate in Rust (sources, options, errors, hook rendering)

### Integration Testing

- SCENARIOS: Python (`pytest`-free `unittest`) and Node (`node --test`) suites over a built package: install, status, errors, hooks, schemas; CI on Linux, macOS and Windows

## Requirements Traceability

SOURCE: .zen/specs/REQ-BND-bindings.md

- BND-1_AC-1 → BND-Glue (BND_P-1)
- BND-1_AC-2 → BND-Glue (BND_P-1)
- BND-1_AC-3 → BND-Glue, BND-Python, BND-Node
- BND-2_AC-1 → BND-Glue
- BND-2_AC-2 → BND-Glue
- BND-2_AC-3 → BND-Glue
- BND-2_AC-4 → BND-Python, BND-Node
- BND-3_AC-1 → BND-Glue
- BND-3_AC-2 → BND-Glue
- BND-4_AC-1 → BND-Python
- BND-4_AC-2 → BND-Node

## Library Usage

### External Libraries

- pyo3 (0.29, `abi3-py39`): the Python extension
- maturin (tool): Python wheels and sdist
- napi, napi-derive, napi-build (3): the Node addon

## Change Log

- 0.1.0 (2026-10-07): Initial design (PLAN-011)
