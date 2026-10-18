---
title: Harnesses
description: What each built-in harness installs, item by item.
---

Pass the id to `--harness` (or `all`). What each harness does with hooks is
on [Hooks](/agent-harness-adapter/write-once/hooks/#what-each-agent-can-do).

| Harness | Id | Instructions | Hooks | Skills | MCP | Allowed commands | Agents | Commands |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code | `claude` | `CLAUDE.md` or `AGENTS.md` | `.claude/settings.json` | `.claude/skills` | `.mcp.json` | yes | yes | yes |
| OpenAI Codex | `codex` | `AGENTS.md` | `.codex/hooks.json` | `.agents/skills` | `.codex/config.toml` | – | yes | – |
| Factory Droid | `factory` | `AGENTS.md` | `.factory/hooks.json` | `.agents/skills` | `.factory/mcp.json` | – | yes | yes |
| Gemini CLI | `gemini` | `GEMINI.md`, or `AGENTS.md` when configured | `.gemini/settings.json` | `.agents/skills` | `.gemini/settings.json` | yes | yes | yes |
| GitHub Copilot | `copilot` | `AGENTS.md` | `.github/hooks/<tool>.json` | `.agents/skills` | `.mcp.json` | – | yes | – |
| Cursor | `cursor` | `AGENTS.md` | `.cursor/hooks.json` | `.agents/skills` | `.cursor/mcp.json` | yes | yes | yes |
| Pi | `pi` | `AGENTS.md` (first of `AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md`) | a generated extension, `.pi/extensions/<tool>.ts` | `.agents/skills` | `.pi/mcp.json` | – | – | yes |
| OpenCode | `opencode` | `AGENTS.md` (first of `AGENTS.md`, `CLAUDE.md`) | a generated plugin, `.opencode/plugins/<tool>.ts` | `.agents/skills` | `opencode.json` | yes | yes | yes |
| Kilo Code | `kilo` | as OpenCode | a generated plugin, `.kilo/plugins/<tool>.ts` | `.agents/skills` | `kilo.json` (or a shared `opencode.json`) | yes | yes | yes |
| Qwen Code | `qwen` | `AGENTS.md` (or what `context.fileName` names) | `.qwen/settings.json` | `.agents/skills` | `.qwen/settings.json` | yes | yes | yes |
| Devin CLI / Devin Local | `devin` | `AGENTS.md` | `.devin/hooks.v1.json` | `.agents/skills` | `.devin/mcp_config.json` | yes | yes | – |
| Any `AGENTS.md` agent | `agents` | `AGENTS.md` | – | `.agents/skills` | – | – | – | – |

Paths are for project scope; user scope uses each harness's directory under
the home directory, and `local` scope (Claude Code, Copilot) its git-ignored
settings file. A dash means the harness has no file for the item, or its
format is not yet confirmed; the item is then listed as `unsupported`. When
several harnesses read one location (`AGENTS.md`, `.agents/skills`,
`.mcp.json`, Claude Code's skills and agents that Cursor and Copilot also
read, Claude Code's files that Devin also reads, OpenCode's `opencode.json`
that Kilo Code also reads), the content is written there once and the others report it as
`shared`.

## Programs

The question `install` asks picks the agents whose program is on `PATH`:
`claude`, `codex`, `droid` (Factory Droid), `gemini`, `copilot`,
`cursor-agent`, `pi`, `opencode`, `kilo`, `qwen`, `devin`.

## After installing

Some agents need a step before they load what was written; `install` notes it:

| Agent | Step after an install |
| --- | --- |
| Codex | trust the project; approve new hooks in `/hooks`; restart |
| Gemini CLI, Qwen Code | a trusted folder, when folder trust is on; Qwen Code asks to approve the project's MCP servers; restart Qwen Code |
| Copilot | the repository's hooks run only in a trusted folder |
| Pi | trust the project; `/reload` |
| OpenCode, Kilo Code | restart |
| Devin | trust the folder; a new session loads the changes |

Every fact on this page, per harness (event names, input fields, answers,
and which are not yet confirmed), comes from the project's harness
requirements, a specification kept with the project's development.
