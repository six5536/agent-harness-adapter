# Changelog

This file records the notable changes of each release of agent-harness-adapter. git-cliff writes the notes of each release from the commits of the release, and the notes of 0.1.0 and 0.2.0 were written by hand. While agent-harness-adapter is pre-1.0, a minor version might contain breaking changes.

## [0.2.0] - 2026-10-18

This release is the first that the public repository publishes, from an export of the project's own repository, and it includes the fixes of 0.1.1, which was never released.

### Added

- One agent setup in the standard places: `agent-harness-adapter install`
  without `--manifest` uses the config `.agents/agent-harness-adapter.toml`
  (found from the working directory upwards; `~/.agents/` at user scope),
  and the first install writes it, with `AGENTS.md`, `.agents/skills`,
  `.agents/hooks.json`, `.mcp.json` and `.agents/agents` as the sources.
- Sources: `{ from = "<path>" }` for `instructions`, `skills`, `hooks`,
  `mcp_servers` and `agents` (library: `Integration::instructions_from`,
  `skills_from`, `hooks_from`, `mcp_servers_from`, `agents_from`). An agent
  that loads a source where it is gets nothing written (`shared … (…, in
  place)`); Claude Code gets `@AGENTS.md` in `CLAUDE.md` and Gemini CLI a
  `context.fileName` entry; the others a copy. Missing sources are listed
  (in text output, only those that are not an unused standard place).
- The hooks file `.agents/hooks.json`, in Claude Code's hooks format: each
  event becomes one hook that runs `agent-harness-adapter hook --hooks
  <path> <harness> <event>`, which runs the matching entries through the
  shell at hook time, in every agent. `agent-harness-adapter schema hooks`.
- Harness choice: `--harness`, else the installed harnesses, else (in a
  terminal) a question, with the agents found on `PATH` picked. Each harness
  names its program (`Harness::program`).
- `hook::file::tool_part`: a tool's hooks as entries of the hooks file, for
  a harness that loads `.agents/hooks.json` natively.
- Migration from Claude Code: the first `install` (or `install --migrate`)
  moves `CLAUDE.md`'s text, `.claude/skills`, Claude Code's hooks and
  `.claude/agents` into the standard places. A conflict with what is already
  there is a note on the first install, and refused with `--migrate`.
- A harness that does not run an event the hooks use says so in its notes;
  Devin (no pre-compact), OpenCode and Kilo Code (no session end) name the
  events they run (`Harness::hook_events`).
- `status` exits 1 when an install would change something
  (`InstallResult::in_sync`).
- The bindings find the config when no manifest is given (Python `None`,
  Node `null`).
- A new documentation site, <https://six5536.github.io/agent-harness-adapter/>
  (Astro Starlight, replacing the mdBook site): a landing page whose terminal
  shows the command's real output, a page per item (instructions, skills,
  hooks with what each agent can do, MCP servers and subagents, commands,
  scopes), Coming from Claude Code, a page per language, the reference
  (config, `.agents/hooks.json`, the hook contract, harnesses, schemas) and
  API references for Node (TypeDoc) and Python (pdoc). Its examples run in CI.
  It deploys on release tags.

### Changed

- **Breaking:** the hook contract is Claude Code's hook protocol, in every
  harness: Claude Code's input (each harness's tool calls named as Claude
  Code names them, `Harness::claude_tool`, plus `harness` and `tool_kind`),
  and Claude Code's answers (exit 2, `decision`, `hookSpecificOutput`). In
  Claude Code a hook's output passes through unchanged. `hook::wire`,
  `parse_hook`, `answer_hook` and `run_hook` speak it; the schemas are
  `schema/hook-input.json` and `schema/hook-answer.json`.
- `uninstall` without `--harness` removes every installed harness.
- `install` and `status` take `--manifest` optionally.
- The READMEs lead with the point and link to the site, which holds the
  only copy of the reference (manifest keys, hook contract, harness table).

### Removed

- **Breaking:** the v1 hook contract (`{"v": 1, …}` in, `{"answer": …}`
  out), `wire::VERSION` and the bindings' `HOOK_VERSION`.

### Fixed

- The library's module docs (`lib.rs`), the Python package's docstring and
  the Node binding's header list every built-in harness: Kilo Code, Qwen
  Code and Devin were missing (and OpenCode from the bindings).
- The Python and Node READMEs' examples are valid code (`import
  agent_harness_adapter as aha`, `const aha = require(…)`); the Python README
  says the package is not on PyPI yet. The library README's module list names
  every harness.

## [0.1.0] - 2026-10-07

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
  files; skills declared or read from a skill directory (`dir`); per-scope
  tables; mistakes reported with their place) and
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
  harness: an instructions block, skills (`Skill`, or `Skill::from_dir` for an
  existing skill directory, its `SKILL.md` kept as it is), hooks (`Hook`), MCP
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
  `cursor` (Cursor), `pi` (Pi, through a generated TypeScript extension),
  `opencode` (OpenCode, through a generated plugin), `kilo` (Kilo Code, the
  same plugin), `qwen` (Qwen Code), `devin` (Devin CLI and Devin Local) and
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
