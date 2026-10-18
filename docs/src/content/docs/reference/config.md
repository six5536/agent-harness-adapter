---
title: The config and the manifest
description: .agents/agent-harness-adapter.toml and a tool's manifest, key by key.
---

The config and a tool's manifest are one format: TOML, or JSON when the file
name ends in `.json`. `agent-harness-adapter schema manifest` prints its
[JSON Schema](/agent-harness-adapter/schema/manifest.v1.json).

## The config

The first `install` writes `.agents/agent-harness-adapter.toml` (at user
scope `~/.agents/agent-harness-adapter.toml`):

```toml title=".agents/agent-harness-adapter.toml"
version = 1
name = "agents"

instructions = { from = "AGENTS.md" }
skills       = { from = ".agents/skills" }
hooks        = { from = ".agents/hooks.json" }
mcp_servers  = { from = ".mcp.json" }
agents       = { from = ".agents/agents" }
```

It is found from the working directory upwards to the repository's root
(outside git, to the first `.agents`), never at or above the home directory.
Its paths are relative to the directory holding its `.agents`. Change a
source to keep a file elsewhere, or add any key below to declare more (for
example `commands` or `allow_commands`). Note that the `agents` key means
subagents, and `name = "agents"` is only the name of your setup's markers
and record.

## Keys

| Key | What it holds |
| --- | --- |
| `version` | `1` |
| `name` | the tool's name; region markers are `<!-- <name>:harness -->` |
| `harnesses` | `"all"` (default) or ids in preference order: the earliest writes a file several harnesses share |
| `adapter` | the command bridged and dispatched hooks run (default `agent-harness-adapter`), e.g. an absolute path |
| `record`, `declined` | paths under the scope's root (defaults `.<name>/harness.toml`, `.<name>/harness.local.toml` at local scope, `.<name>/config.toml`) |
| `instructions` | text placed between the tool's markers in each harness's instructions file; or `{ from = "<file>" }`, a source |
| `skills` | each `dir` (a skill directory: its `SKILL.md` as it is and every other file in it, hidden ones aside), or `name`, `description`, `body`, `files` (path → text); or `{ from = "<dir>" }`, every skill directory in it |
| `hooks` | each `event`, then `run` (bridged) or `command` (a template), `tools`, `timeout` (seconds), `commands` (harness → template), `owned`; or `{ from = "<hooks file>" }` |
| `hook_match` | which hook entries are the tool's: `{ prefix }`, `{ contains }` or `{ any = [..] }` |
| `mcp_servers` | each `name`, then `command`, `args`, `env`, or `url`, `headers`; or `{ from = "<MCP JSON file>" }` |
| `allow_commands`, `allow_mcp_tools` | what the agent may run without asking: command prefixes; `{ server, tool }` |
| `agents`, `commands` | each `name`, `description`, `prompt`; agents also `{ from = "<dir>" }`, each `<name>.md` kept as written |
| `parts` | raw `file`, `region` or `merge` parts for one harness |
| `scopes.<scope>` | items, `record` and `declined` that replace the top level's at that scope |

Any text is a string or `{ file = "<path>" }` (copied), relative to the
manifest. An item given `{ from = "<path>" }` is read from that source: an
agent that loads the source where it is gets nothing written
(`shared … (…, in place)`), Claude Code and Gemini CLI are pointed at it, the
others get a copy. A missing source installs nothing and is listed as
`missing source` (in the text output only when it is not the config's
standard place for that item, which you may simply not use; `--json` lists
every one).

## Hooks

Events: `session-start`, `session-end`, `prompt-submit`, `pre-tool`,
`post-tool`, `stop`, `pre-compact`. `tools`: `shell`, `read`, `write`, `mcp`.

- `run = "<command>"`: bridged. The harness runs
  `agent-harness-adapter hook --tool <name> <harness> <event> -- <command>`;
  your command speaks [the hook contract](/agent-harness-adapter/reference/hook-contract/).
- `command = "<template>"`: run as it is, in the harness's own protocol.
  `{harness}` and `{event}` are filled in; `{{` and `}}` are literal braces.

## Versioning

The manifest's `version` changes only in a release that says so. Pre-1.0,
minor versions may contain breaking changes.
