# Tests outside cargo

Cargo's own tests live with each crate (`crates/*/*/tests/` and the
`#[cfg(test)]` modules) and run with `cargo nextest run --workspace`. These
are the rest: checks of what ships, run from the repository root.

| What | Where | Run | CI |
| ---- | ----- | --- | -- |
| The release binary: install, status, uninstall, hook, schema | `tests/smoke/release-smoke.mjs` | `npm run smoke` (after `cargo build --release -p agent-harness-adapter`) | release builds, every platform it can run |
| The packed npm launcher runs the binary | `tests/smoke/launcher-smoke.mjs` | `npm run smoke:launcher` (binary staged into its platform package) | release builds |
| Real agents load what the adapter installs and run its hooks | `tests/agents/` ([README](agents/README.md)) | `npm run agents -- run` | no: needs the agents' logins |
| smllm and sokf build and pass against this checkout | `tests/consumers/validate-consumers.sh` | `SMLLM_REPO=… SOKF_REPO=… tests/consumers/validate-consumers.sh` | no: private repositories; before each release |

Tests that belong to one package stay beside it:

| What | Where | Run | CI |
| ---- | ----- | --- | -- |
| The npm launcher's unit tests | `packages/agent-harness-adapter/test/` | `npm run test:launcher` | yes |
| The Node binding | `packages/agent-harness-adapter-node/test/` | `npm run build:node`, then `AHA_NODE_ADDON=… npm run test:node` | yes |
| The Python binding | `crates/bind/agent-harness-adapter-py/tests/` | `maturin develop`, then `python -m unittest discover -s crates/bind/agent-harness-adapter-py/tests` | yes |
| The generated Pi extension and OpenCode plugin | `crates/lib/agent-harness-adapter-core/tests/extensions/` | `AHA=target/debug/agent-harness-adapter npm run test:extensions` | yes |
| A Python tool, end to end | `examples/python-tool/test_example.py` | `python3 examples/python-tool/test_example.py target/debug/agent-harness-adapter` | yes |
