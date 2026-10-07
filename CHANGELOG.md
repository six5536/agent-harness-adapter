# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
agent-harness-adapter uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While it is pre-1.0, minor versions may contain breaking changes.

Every released tag needs its own section here. The release workflow refuses to
publish a version it cannot find a heading for, and that section becomes the
GitHub release notes.

## [Unreleased]

## [0.1.0]

First release: the library `agent-harness-adapter-core` and the
`agent-harness-adapter` command (crate `agent-harness-adapter`, npm
`@six5536/agent-harness-adapter`).

### Added

- `agent-harness-adapter` command: `install`, `uninstall` and `status` from a
  manifest file, `hook` (runs a harness's hook through a command that speaks the hook
  contract), `schema` (the contracts' JSON Schemas, also in `schema/`).
  Prebuilt binaries for Linux and macOS (x64, arm64) and Windows (x64)
  through npm and the GitHub release.
- Python binding `six5536-agent-harness-adapter` (module
  `agent_harness_adapter`): `install`, `uninstall`, `status`, `parse_hook`,
  `answer_hook`, `run_hook`, `schema`; abi3 wheels for CPython 3.9+.
- Node binding `@six5536/agent-harness-adapter-node`: `install`, `uninstall`, `status`,
  `parseHook`, `answerHook`, `runHook`, `schema`, with TypeScript types;
  prebuilt addons for Linux (glibc), macOS and Windows.
- `fs::home_dir`, `harness::expand` (`all`).
- `manifest`: `Manifest` (TOML or JSON, version 1; text inline or from
  files; per-scope tables; mistakes reported with their place) and
  `ManifestTool`, a `Tool` over it.
- `hook::wire`: the hook contract (version 1): `input_json`, `parse_answer`,
  `answer_json`; `HookInput`, `ToolCall`, `Event`, `ToolKind` and `Answer`
  are serde types.
- `harness::installed`: the harnesses recorded at a scope.
- `harness::uninstall` (`UninstallOptions`): takes a tool's integration back
  out, keeping the user's content and edits and whatever another installed
  harness still reads; `Action::Removed` / `Action::Kept`;
  `ExternalPart::removable` / `remove`. In the command and both bindings too.
- `integration`: `Integration`, declared once per scope without naming a
  harness: an instructions block, skills (`Skill`), hooks (`Hook`), MCP
  servers (`McpServer`), allowed commands and MCP tools (`allow_command`,
  `allow_mcp_tool`), subagents (`Agent`), slash commands (`Command`), and raw
  parts for one harness (a raw part may stand for an item the harness does
  not render). `Item` names each kind.
- `harness`: the `Harness` contract (render, reads, hook events, hook
  parsing and answers, notes), `builtin()` and `find()`; `install` / `status` over a set of
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
  and `{event}` placeholders, and may differ per harness (`Hook::command_for`).
- `report` (`Finding`, `Report`, `Severity`), `cli`, `fs`.
- `Error` with one variant per kind of refusal.
- Optional feature `schemars`: `JsonSchema` on the result types, and the
  schemas of the result, the manifest and the hook contract.
- MSRV 1.85.

[Unreleased]: https://github.com/six5536/agent-harness-adapter/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/six5536/agent-harness-adapter/releases/tag/v0.1.0
