# PLAN-011: Workspace, `ahk` CLI with manifests, language bindings

| Field              | Value |
| ------------------ | ----- |
| Status             | in-progress: P1–P6 done (2026-10-07) |
| Workflow direction | top-down (layout → architecture → requirements → design → code → docs → release) |
| Traces to          | ARCHITECTURE, REQ-KIT, REQ-HAR, DESIGN-KIT, DESIGN-HAR, PLAN-009 (D9-4, D9-5, F8, F9), PLAN-010 P9 |

## 1. Goal

Make the kit usable from tools in any language:

1. A Rust workspace again. The library becomes `agent-harness-kit-core`; the crate `agent-harness-kit` is the `ahk` CLI, also shipped through npm (the smllm launcher and platform packages layout).
2. `ahk` reads a tool's integration from a manifest file and bridges hooks over one JSON format (option 1).
3. Python (PyO3) and Node (napi-rs) bindings over the same JSON formats (option 2).

## 2. Decisions

- D11-1: Layout (as in smllm before PLAN-009 D9-4):
  - `crates/lib/agent-harness-kit-core`: today's `src/`, `tests/`, `proptest-regressions/`. Restructure only: no code change except the crate name in paths and docs.
  - `crates/app/agent-harness-kit`: the CLI crate; binary `ahk`.
  - `crates/bind/agent-harness-kit-py`, `crates/bind/agent-harness-kit-node`: bindings (P7, P8).
  - `packages/`: npm launcher `agent-harness-kit` (bin `ahk`) and one package per platform: linux-x64, linux-arm64, darwin-x64, darwin-arm64, win32-x64.
  - Root `Cargo.toml`: workspace with shared `[workspace.package]` (version, edition, MSRV, license, repository) and `[workspace.dependencies]`.
- D11-2: One version for every crate and package, set from the workspace `Cargo.toml`; `scripts/set-version.mjs` / `verify-version.mjs` keep npm and the Python package in step (from smllm).
- D11-3: The first release (PLAN-010 P9) moves here. Nothing is published under the old single-crate layout, so the name `agent-harness-kit` never means the library on crates.io.
- D11-4: Consumers (smllm, sokf) depend on `agent-harness-kit-core`. `scripts/validate-consumers.sh` patches that name to `crates/lib/agent-harness-kit-core`.
- D11-5: The library's public API stays as it is for Rust users. P3+ may add to it (serde on hook types, loading an `Integration` from data) but change nothing existing.
- D11-6: Manifest: TOML, `<tool>.harness.toml`, holding everything `Tool` provides:
  - `version` (manifest format, starts at 1), `name`, `harnesses` (list or `"all"`), `root` per scope (default: project = working dir, user = home), `record`, `declined` (path of the declined-parts TOML and its table).
  - Integration items, per scope with a shared default: instructions, skills, hooks (template, event, timeout, per-harness commands, entry match), MCP servers, allowed commands, allowed MCP tools, agents, commands.
  - Bodies inline or `file = "..."` relative to the manifest.
  - Raw parts per harness: `file`, `region`, `merge` only (an `external` part needs code; not offered).
  - Unknown keys are an error (a typo never installs silently).
- D11-7: Hook bridge: `ahk hook <harness> <event> -- <command...>`:
  - Parses the harness's input through the kit (`HookInput::parse`), writes one neutral JSON object to the command's stdin, reads one JSON answer from its stdout, and emits it through the harness (`emit`).
  - The answer format is the one Pi's extension already uses: `{"answer": "allow" | "deny" | "continue" | "context", "reason"?, "text"?}`. Pi's extension and `ahk` share it; one contract.
  - Input and answer JSON carry `"v": 1`.
  - A command that fails, times out, or answers badly allows (as Pi's extension does); `ahk` writes the reason to stderr.
  - Hook templates installed from a manifest default to `ahk hook {harness} {event} -- <tool command>`; the tool may give its own.
- D11-8: Public contracts, versioned and published as JSON Schema under `schema/` (from the `schemars` feature): the manifest, hook input, hook answer, the `--json` install/status result. A breaking change to one bumps its `version`/`v` and is a minor release before 1.0.
- D11-9: CLI surface:
  - `ahk install --manifest <file> [--harness a,b|all] [--scope project|user|local] [--force] [--without <items>] [--json]`
  - `ahk status --manifest <file> [--harness ...] [--scope ...] [--json]`
  - `ahk hook <harness> <event> -- <command...>`
  - `ahk schema <manifest|hook-input|hook-answer|result>`: prints a schema
  - Exit codes and output through the kit's `cli` module.
- D11-10: Bindings are thin, over the D11-8 JSON formats: `install(manifest, options) -> result`, `status(...)`, `hook(harness, event, input_text) -> HookInput JSON`, `emit(harness, event, answer) -> (stdout, stderr, exit)`. No native mirror of the builder API.
  - Python: PyO3 + maturin, abi3 wheels per platform, on PyPI.
  - Node: napi-rs, platform packages on npm (as for the CLI).
- D11-11: Distribution: crates.io (`agent-harness-kit-core`, `agent-harness-kit`), npm (CLI launcher + platform packages; Node binding), PyPI (Python binding), GitHub release binaries. One `vX.Y.Z` tag publishes all, after a dry run of each.

## 3. Functional requirements

| ID | Requirement |
| -- | ----------- |
| F1 | The workspace builds and every existing test passes, unchanged, in `crates/lib/agent-harness-kit-core` (D11-1). |
| F2 | `ahk` installs and reports a manifest's integration for the named harnesses and scope, with the same results as a Rust `Tool` with the same integration. |
| F3 | Manifest loading: inline and file bodies, scopes, raw parts, unknown keys and missing files reported with the manifest path and key. |
| F4 | `ahk hook` translates every harness's input and answer for every event the harness supports (D11-7), including failures that allow. |
| F5 | `ahk schema` prints each contract's schema; the checked-in `schema/*.json` match it (CI). |
| F6 | npm: `npx agent-harness-kit` / `npm i -g agent-harness-kit` runs `ahk` from the platform package; an unsupported platform gets a clear error. |
| F7 | Python binding: D11-10 functions; results equal the CLI's JSON. |
| F8 | Node binding: D11-10 functions; results equal the CLI's JSON. |
| F9 | Release: one tag publishes crates, npm, PyPI and GitHub release binaries (D11-11). |

## 4. Non-functional requirements

| ID | Requirement |
| -- | ----------- |
| N1 | Core keeps MSRV 1.85, ≥ 90% line coverage, the 800-line limit and its trace; the CLI and bindings get their own ≥ 90% gate (bindings: their Rust glue). |
| N2 | `ahk hook` adds no more than one process per hook run; static Linux binaries (musl). |
| N3 | New dependencies: only those of D11-15 (serde is already one); tooling: `cargo-zigbuild` + zig, `maturin`, `@napi-rs/cli`. |
| N4 | CI: the PLAN-009 F8 checks per crate, plus launcher tests, schema check, a smoke run of the release binary and of each binding on Linux, macOS and Windows. |

## 5. Specs

- `REQ-KIT`, `REQ-HAR`, `DESIGN-*`: paths and crate name only (P1); serde/manifest loading ACs added in P3.
- New `REQ-AHK` / `DESIGN-AHK`: manifest, CLI, hook bridge, contracts.
- New `REQ-BND` / `DESIGN-BND`: bindings.
- `ARCHITECTURE.md`: workspace, components, distribution.

## 6. Phases

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P1 Workspace | D11-1 layout for core only; workspace `Cargo.toml`; CI, release, deny, coverage, scripts, README, ARCHITECTURE paths; specs renamed to the core crate. No code change. | All core checks pass from the workspace; `git diff -M` shows moves plus manifest/doc edits only |
| P2 CLI skeleton + npm | `crates/app/agent-harness-kit` (`ahk`, clap, `--version`); `packages/` launcher and platform packages, scripts (set/verify version, launcher smoke) from smllm; release builds per platform. | Launcher tests pass; a locally packed npm launcher runs `ahk --version` |
| P3 Contracts | REQ/DESIGN-AHK; core: `Deserialize` / `Serialize` and schemas for hook input and answers, `Integration` from data (D11-5); manifest types and loader in the CLI; `schema/`. | Contract round-trip tests; schemas checked in |
| P4 install / status | F2, F3. | Each harness: manifest vs Rust `Tool` give equal results (tests) |
| P5 Hook bridge | F4; Pi's extension uses the shared answer types' docs. | Per-harness, per-event tests through the binary (`assert_cmd`) |
| P6 Consumer check | Port smllm and sokf to `agent-harness-kit-core`; a sample tool in another language (`examples/`, shell or Python) driving `ahk` end to end. | Both consumers pass; example passes in CI |
| P7 Python binding | REQ/DESIGN-BND; F7; maturin wheels. | Python tests pass on three OSes |
| P8 Node binding | F8; napi platform packages. | Node tests pass on three OSes |
| P9 Release | F9 (with PLAN-010 P9's checks); tokens: `CARGO_REGISTRY_TOKEN`, npm, PyPI (trusted publishing). Tag only on explicit confirmation. | Everything published at 0.1.0 |

## 7. Resolved questions

Registry check (2026-10-07): crates.io `agent-harness-kit` and `agent-harness-kit-core` are free; npm `agent-harness-kit` and `ahk`, and PyPI `agent-harness-kit`, belong to other projects.

- D11-12: npm names: `@six5536/agent-harness-kit` (bin `ahk`) and `@six5536/agent-harness-kit-<platform>`.
- D11-13: Bindings' names: npm `@six5536/agent-harness-kit-node`; PyPI `six5536-agent-harness-kit` (import `agent_harness_kit`).
- D11-14: 0.1.0 ships everything: P9 follows P8.
- D11-15: Dependencies approved: `clap` (P2), `pyo3` (P7), `napi` / `napi-derive` / `napi-build` (P8).
