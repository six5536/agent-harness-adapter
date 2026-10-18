---
title: The hook contract
description: Every hook speaks Claude Code's hook protocol, in every agent.
---

A hook from `.agents/hooks.json`, and a tool's bridged hook (`run`), speak
Claude Code's hook protocol in every agent. The adapter translates.

## Where it runs

Your command runs through the shell (`sh -c`, or `cmd /C` on Windows) in the
agent's working directory, with `CLAUDE_PROJECT_DIR` set to the project's
root and the hook's input on stdin. A hook in `.agents/hooks.json` stops after
its `timeout` (60 seconds by default).

## Input

Your command gets Claude Code's input on stdin, with the agent's tool call
named as Claude Code names it, plus `harness` and `tool_kind`:

```json
{"hook_event_name": "PreToolUse", "session_id": "…", "cwd": "…",
 "tool_name": "Bash", "tool_input": {"command": "rm -rf /"},
 "harness": "codex", "tool_kind": "shell"}
```

| Field | Holds |
| --- | --- |
| `hook_event_name` | `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact` |
| `harness` | the agent that ran the hook: `claude`, `codex`, … |
| `session_id`, `transcript_path`, `cwd` | when the agent gives them |
| `prompt` | at prompt submit |
| `tool_name`, `tool_input` | Claude Code's: `Bash` with `command` as text; `Read`, `Edit`, `Write` with `file_path`; `mcp__<server>__<tool>`. A tool Claude Code does not have keeps its own name |
| `tool_kind` | `shell`, `read`, `write`, `mcp` or `other` |
| `tool_response` | after a tool |
| `source` | at session start |
| `stop_hook_active` | at stop: the agent is already continuing because a hook asked it to |

Schema: [hook-input.json](/agent-harness-adapter/schema/hook-input.json).

## Answer

| Answer | Session start | Prompt | Before a tool | After a tool | Stop |
| --- | --- | --- | --- | --- | --- |
| `exit 2`, the reason on stderr | – | block it | block it | reason to the agent | keep going |
| `{"decision": "block", "reason": "…"}` | – | block it | block it | reason to the agent | keep going |
| `{"hookSpecificOutput": {"permissionDecision": "deny", "permissionDecisionReason": "…"}}` | – | – | block it | – | – |
| `{"hookSpecificOutput": {"additionalContext": "…"}}` | add context | add context | – | add context | – |
| plain text on stdout | add context | add context | – | – | – |

Anything else, including exit 0 with no output, allows (at stop: lets the
agent stop). "Keep going" sends the reason to the agent as its next input.

`exit 2` is the hook protocol's "block"; it is not the command's own exit
code 2 (a refused command line).

Schema: [hook-answer.json](/agent-harness-adapter/schema/hook-answer.json).

In Claude Code your hook's output passes through unchanged. In any other
agent it is translated; what that agent cannot do (`permissionDecision:
"ask"`, `updatedInput`, `continue: false`, …) is left out with a note on
stderr. [The table on Hooks](/agent-harness-adapter/write-once/hooks/#what-each-agent-can-do)
shows what each agent can do.

## Failure allows

A command that cannot start, exits with another code than 0 or 2, or runs
past its timeout allows, with a note on stderr. Your hook failing never
blocks the agent.

## Versioning

The contract follows Claude Code's hook protocol and changes only in a
release that says so.
