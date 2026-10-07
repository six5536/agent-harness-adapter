# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
agent-harness-kit uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While it is pre-1.0, minor versions may contain breaking changes.

Every released tag needs its own section here. The release workflow refuses to
publish a version it cannot find a heading for, and that section becomes the
GitHub release notes.

## [Unreleased]

## [0.1.0]

First release as a crate of its own.

### Added

- `integration`: `Integration`, declared once per scope without naming a
  harness: an instructions block, skills (`Skill`), hooks (`Hook`), MCP
  servers (`McpServer`), allowed commands, subagents (`Agent`), slash
  commands (`Command`), and raw parts for one harness. `Item` names each kind.
- `harness`: the `Harness` contract (render, reads, hook parsing and answers,
  notes), `builtin()` and `find()`; `install` / `status` over a set of
  harnesses with `InstallOptions`, `InstallResult`, `HarnessResult`,
  `PartResult` (`State`, `Option<Action>`, `by`); `Part` (`files`, `region`,
  `merge`, `external`); `MergeOp` (`array_entry`, `object_member`,
  `owned_entries`, `group_entries`) with `EntryMatch` (`Prefix`, `Contains`,
  `Any`); JSON and TOML merges that keep the file's style; `DeclinedStore` /
  `TomlDeclined`; `Markers`; scopes `project`, `user` and `local`.
- Shared locations: per item, the fewest locations every harness in the set
  loads; the others report the part as `shared`, and a warning names any
  agent that may load something twice.
- Harnesses: `claude` (Claude Code), `codex` (OpenAI Codex CLI), `factory`
  (Factory Droid), `gemini` (Gemini CLI), `copilot` (GitHub Copilot),
  `cursor` (Cursor), `pi` (Pi, through a generated TypeScript extension) and
  `agents` (any agent that reads `AGENTS.md`).
- `hook`: `Event`, `HookInput`, `ToolCall` / `ToolKind`, `Answer` (`Allow`,
  `Deny`, `Continue`, `Context`), `Output`, `emit` through a harness, and
  `LoopGuard`. Hook commands are any command line, with optional `{harness}`
  and `{event}` placeholders.
- `report` (`Finding`, `Report`, `Severity`), `cli`, `fs`.
- `Error` with one variant per kind of refusal.
- Optional feature `schemars`: `JsonSchema` on the result types.
- MSRV 1.85.

[Unreleased]: https://github.com/six5536/agent-harness-kit/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/six5536/agent-harness-kit/releases/tag/v0.1.0
