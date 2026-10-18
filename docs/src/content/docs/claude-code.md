---
title: Coming from Claude Code
description: Move CLAUDE.md, .claude/skills, Claude Code's hooks and .claude/agents into the standard places, and keep Claude Code working.
---

The first `agent-harness-adapter install` in a repository with a Claude Code
setup offers to move it into the standard places, where every agent can use
it. Claude Code keeps working: `CLAUDE.md` gets the line `@AGENTS.md`, skills
and subagents are copied back into `.claude/`, and its settings get one entry
per event that runs your hooks from `.agents/hooks.json`.

```sh
agent-harness-adapter install                     # asks (answer no to leave it)
agent-harness-adapter install --migrate           # moves without asking
```

Without a terminal (in CI, or from a script) it never asks: nothing moves,
and the moves it would make are listed with a note. Run `install --migrate`
at any time to move what is still in Claude Code's places.

## What moves

| From | To | Claude Code then gets |
| --- | --- | --- |
| `CLAUDE.md`'s own text | `AGENTS.md` (under `## From CLAUDE.md` when `AGENTS.md` already has text) | `@AGENTS.md` in `CLAUDE.md` |
| `.claude/skills/<name>` | `.agents/skills/<name>` | a copy in `.claude/skills` |
| hooks in `.claude/settings.json` | `.agents/hooks.json` | one entry per event that runs them ([Hooks](/agent-harness-adapter/write-once/hooks/#how-it-runs)) |
| `.claude/agents/<name>.md` | `.agents/agents/<name>.md` | a copy in `.claude/agents` |

The hooks that move are command hooks on the seven events the adapter knows
(`SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`,
`PostToolUse`, `Stop`, `PreCompact`).

## What stays

- Text between a tool's markers in `CLAUDE.md` (`<!-- mytool:harness -->` …).
- Hooks on other events, hooks of another type than `command`, and hooks that
  already run `agent-harness-adapter`.
- Permissions, slash commands (`.claude/commands`), `.claude/settings.local.json`
  and every other setting. They have no standard place yet, so keep them in
  `.claude/`.
- `.mcp.json`: it is already the standard place.

Each is listed as `not migrated: …` before anything moves.

## Safety

- A file already in the standard place with the same content is left in both
  places. One with different content stops the migration, naming both: the
  first install notes it and installs without migrating; `--migrate` refuses
  until you merge them.
- At project scope, files with changes git has not committed stop the
  migration (`--force` moves them anyway), so `git diff` shows exactly what moved.
- At user scope (`--scope user`), it moves `~/.claude/CLAUDE.md`,
  `~/.claude/skills`, `~/.claude/settings.json`'s hooks and `~/.claude/agents`
  into `~/.agents/`, after backing them up to `~/.agents/backup-<time>/`.

## After migrating

`uninstall` takes out only what the adapter wrote: the `@AGENTS.md` line,
the copies and the hook entries. The moved text, skills, hooks and subagents
stay in `AGENTS.md` and `.agents/`.

## Undo it

At project scope, `git restore .` brings back the files that moved; then
delete the new, untracked files that `git status` lists. At user scope, copy
the backup back.
