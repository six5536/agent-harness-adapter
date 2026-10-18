---
title: .agents/hooks.json
description: The hooks file, Claude Code's hooks format.
---

`.agents/hooks.json` (`~/.agents/hooks.json` at user scope) is Claude Code's
hooks format: the `hooks` object of its `settings.json`.
`agent-harness-adapter schema hooks` prints its
[JSON Schema](/agent-harness-adapter/schema/hooks-file.json).

```json
{
  "hooks": {
    "<Event>": [
      {
        "matcher": "<regular expression on Claude Code's tool name>",
        "hooks": [{ "type": "command", "command": "<shell command>", "timeout": 60 }]
      }
    ]
  }
}
```

| Field | Means |
| --- | --- |
| `<Event>` | `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact` |
| `matcher` | optional; matches the whole tool name (`Bash`, `Edit\|Write`, `mcp__.*`); empty or `*` matches every tool; ignored on events without a tool |
| `type` | `command` |
| `command` | run through `sh -c` (`cmd /C` on Windows) in the agent's working directory, with `CLAUDE_PROJECT_DIR` set to the project's root |
| `timeout` | seconds, default 60; past it the entry allows |
| `tool` | optional: the tool the entry belongs to, for a tool that writes into this file; the dispatcher skips those entries |

Other events and other `type`s are kept, run by Claude Code alone, and noted
at install. The file is read at hook time, so changes apply at once; install
again when you use an event for the first time.

See [Hooks](/agent-harness-adapter/write-once/hooks/) for how each agent runs it.
