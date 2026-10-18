---
title: How it works
description: The words the rest of the docs use, and what install and the hooks actually do.
---

## Agents and harnesses

A **harness** is a coding agent the adapter knows how to write files for:
Claude Code, Codex, Cursor and the rest. The command line calls them that
(`--harness claude,codex`); the docs say "agent". Each has an id, listed on
[Harnesses](/agent-harness-adapter/reference/harnesses/). One id, `agents`,
stands for any other agent that reads `AGENTS.md`.

## Sources

A **source** is a file or folder you keep, that agents should use:
`AGENTS.md`, `.agents/skills`, `.agents/hooks.json`, `.mcp.json`,
`.agents/agents`. The config, `.agents/agent-harness-adapter.toml`, names
them (`instructions = { from = "AGENTS.md" }`, …). You edit sources; the
adapter never changes them.

```text
AGENTS.md           ┐                  ┌─▶ read where it is (in place)
.agents/skills/     │                  ├─▶ pointed at it
.agents/hooks.json  ├─▶  install  ─────┤
.mcp.json           │                  └─▶ a copy, in its own format
.agents/agents/     ┘
```

For each source and each agent, the install picks one of three:

| | When | What `install` prints |
| --- | --- | --- |
| **In place** | the agent already reads the source where it is (Codex reads `AGENTS.md`) | `shared  AGENTS.md (instructions, in place)` |
| **Pointed at** | the agent can be told to read it (Claude Code: `@AGENTS.md` in `CLAUDE.md`; Gemini CLI: `context.fileName`) | `created CLAUDE.md (instructions)` |
| **Copied** | neither (Claude Code's skills, every agent's subagents) | `created .claude/skills (skills)` |

In place and pointed-at items follow your edits at once. Copies follow on the
next `install`, and `status` tells you when one is out of date. When one
agent's copy also serves another (Kilo Code reads OpenCode's `opencode.json`),
the second prints `shared … (…, by opencode)`.

## What install writes, and what it leaves

The adapter only touches its own content:

- in an instructions file such as `CLAUDE.md`, a **region** between markers,
  `<!-- agents:harness -->` … `<!-- /agents:harness -->`; your text around
  it stays;
- in a settings file such as `.claude/settings.json`, its own entries; your
  settings and hooks stay;
- whole files and folders it created (copied skills, generated plugins).

It keeps a **record** of what it wrote, `.agents/harness.toml`, so it can
tell a file it wrote and that changed since (`stale`) from one you changed
by hand (`edited`), which it leaves alone. Commit the record with the rest.
Parts you turn off with `--without` are remembered in `.agents/config.toml`.

## Hooks

Agents differ in how they run hooks, so each agent gets one small entry per
event in its own hooks file, which runs the **dispatcher**:

```text
agent-harness-adapter hook --hooks .agents/hooks.json codex pre-tool
```

When the agent fires the event, the dispatcher translates the agent's input
into Claude Code's, runs your matching hooks from `.agents/hooks.json`, and
translates their answer back. Your hooks only ever see Claude Code's format.
See [Hooks](/agent-harness-adapter/write-once/hooks/).

## Tools

A tool (a linter, a test runner, …) can ship its own integration the same
way, from a manifest file or the Rust, Node or Python library. Its content
gets its own markers and record (`<!-- mytool:harness -->`,
`.mytool/harness.toml`), so several tools and your own setup sit side by side.
See [For tool authors](/agent-harness-adapter/use/command/).
