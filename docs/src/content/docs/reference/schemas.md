---
title: JSON schemas
description: The formats the adapter publishes, as JSON Schema.
---

The formats the adapter publishes, as JSON Schema (draft 2020-12). Print any
of them with `agent-harness-adapter schema <name>`.

| Schema | What it describes |
| ------ | ----------------- |
| [manifest.v1.json](/agent-harness-adapter/schema/manifest.v1.json) | The config and a tool's manifest (TOML or JSON) |
| [hooks-file.json](/agent-harness-adapter/schema/hooks-file.json) | The hooks file, `.agents/hooks.json`: Claude Code's hooks format |
| [hook-input.json](/agent-harness-adapter/schema/hook-input.json) | What a hook command gets on stdin: Claude Code's hook input |
| [hook-answer.json](/agent-harness-adapter/schema/hook-answer.json) | What a hook command answers on stdout: Claude Code's answer JSON |
| [result.v1.json](/agent-harness-adapter/schema/result.v1.json) | The `--json` result of `install`, `uninstall` and `status` |

A breaking change to the manifest or the result bumps its version (`v1` →
`v2`) and comes in a minor release before 1.0. The hook contract follows
Claude Code's hook protocol.
