# JSON schemas

The formats the adapter publishes, as JSON Schema (draft 2020-12). Print
any of them with `agent-harness-adapter schema <name>`.

| Schema | What it describes |
| ------ | ----------------- |
| [manifest.v1.json](schema/manifest.v1.json) | A tool's integration, the file `--manifest` reads (TOML or JSON) |
| [hook-input.v1.json](schema/hook-input.v1.json) | What a bridged hook command gets on stdin |
| [hook-answer.v1.json](schema/hook-answer.v1.json) | What a bridged hook command answers on stdout |
| [result.v1.json](schema/result.v1.json) | The `--json` result of `install`, `uninstall` and `status` |

A breaking change to a format bumps its version (`v1` → `v2`) and comes in a
minor release before 1.0.
