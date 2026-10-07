# PLAN-010: Support the common agent harnesses

| Meta               | Value |
| ------------------ | ----- |
| Status             | in progress: P1–P7 done (2026-10-07); P7's smoke checks inside each agent (N4) and P8 (consumers) remain |
| Workflow direction | top-down (architecture → requirements → design → code → docs → consumers → release) |
| Traces to          | ARCHITECTURE (harness core, harness modules), KIT-1, KIT-2, KIT-3, KIT-4, KIT-10, KIT-11, KIT-12; PLAN-009 D9-17 |

## 1. Goal

A tool declares its integration **once**, without naming a harness: an instructions block, skills, hooks, MCP servers, command permissions, agents and commands. The kit renders it for each harness the user installs, puts shared content in the most common location (D10-5), and translates hook input and answers both ways. Ships as the first release, 0.1.0 (D10-12).

## 2. Harnesses

| Harness id | Agent | Hooks | Notes |
| ---------- | ----- | ----- | ----- |
| `claude` | Claude Code | Claude family | exists; evolves freely (D10-2) |
| `codex` | OpenAI Codex CLI | Claude family | MCP in TOML; hooks need `/hooks` approval |
| `factory` | Factory Droid | Claude family | `hooks.json` with events at the top level; its own tool names |
| `gemini` | Gemini CLI | Claude family, renamed events | timeouts in ms; `GEMINI.md` unless `context.fileName` lists `AGENTS.md` |
| `copilot` | GitHub Copilot (CLI, VS Code agent, cloud agent) | flat, versioned | one hook file per tool, `.github/hooks/<tool>.json`; camelCase input |
| `cursor` | Cursor (editor and CLI) | flat, versioned | already reads Claude's hooks, skills and agents; rules are `.mdc` |
| `pi` | Pi (`@earendil-works/pi-coding-agent`) | code | hooks are a generated TypeScript extension |
| `agents` | Generic: any agent that reads `AGENTS.md` | none | `AGENTS.md` region and `.agents/skills` only |

The per-harness locations, formats and sources are in §10.

## 3. Decisions

- D10-1: The eight harnesses in §2. The rest (Windsurf, Cline, OpenCode, Kiro, Amp, Roo, Aider, Junie, Zed, Goose, Continue, Qwen, Crush, Warp) are left for later plans; `agents` reaches the ones that read `AGENTS.md`.
- D10-2: No backwards compatibility with today's API. The `claude` module and the hook types change as the shared model needs, and smllm and sokf adapt.
- D10-3: **Neutral integration.**
  - The tool declares one `Integration`: an instructions block, skills (whole directories), hooks (neutral event + command), MCP servers, allowed commands, agents and commands.
  - Each harness is a value implementing a public `Harness` trait. It renders an `Integration` at a scope into a `Profile` of today's `Part`s, and translates hook IO (D10-4).
  - The kit ships one `Harness` per §2 row, in one module each (`claude`, `codex`, …). A third party can add a harness without a kit change.
  - The low-level layer (`Part`, `MergeOp`, `Profile`) stays public, so a tool can add raw parts to any harness.
  - An item a harness cannot take at a scope (e.g. Cursor's user-level rules, Copilot's cloud MCP) is left out and reported as unsupported, never an error.
- D10-4: **Neutral hooks.**
  - Events: `SessionStart`, `PromptSubmit`, `PreTool`, `PostTool`, `Stop`, plus `SessionEnd` and `PreCompact` where they exist. Each harness maps them to its names and leaves out the ones it lacks.
  - Input: one `HookInput` (session, cwd, event, prompt, tool name and input, tool output, whether this is a repeat stop, the last assistant message), parsed per harness. The repeat-stop flag comes from `stop_hook_active` or Cursor's `loop_count > 0`.
  - Answers: `Allow`, `Deny { reason }` (before a tool), `Continue { reason }` (on stop: keep the agent going), `Context { text }`. Each harness writes them in its form: stdout JSON, exit code, stderr.
  - The command is the tool's own, any shape: `{harness}` and `{event}` placeholders go anywhere in it, in any order, or not at all (e.g. `mytool hook {harness} {event}`, `mytool-guard --event={event} --agent={harness}`, or one command per event). The tool parses its own arguments, then calls the kit with the harness id and event. D10-16.
  - Pi: the kit generates `.pi/extensions/<tool>.ts`. It calls the command with JSON on stdin through `node:child_process` and maps the answer to Pi's `{ block, reason }` / `{ continue }`. The extension is a file part, so it is recorded and checked like any other file.
  - `LoopGuard` keeps its behaviour and moves to `hook` (D10-13).
- D10-5: **Dedupe shared content.**
  - Each harness declares which locations it reads per item kind, including other harnesses' locations (e.g. Cursor reads `.claude/settings.json` hooks and `.claude/skills`).
  - `install` and `status` take a **set** of harnesses. The kit then picks, per item, the locations that cover every harness in the set with the fewest copies. On a tie it prefers the shared location (`AGENTS.md`, `.agents/skills`, `.mcp.json`).
  - A harness covered by another's location gets that item as the new state `shared` (with the location that covers it), and nothing is written for it.
  - A warning is a last resort, used only when no choice avoids a double load (e.g. a harness that loads both of two locations it cannot avoid).
  - The record keeps a table per harness, as now; a `shared` part records no hash of its own.
  - Installing a single harness is a set of one. The kit reads the record to learn which harnesses are already installed at the scope, and includes them in the set.
- D10-6: **Instructions.** One region (the tool's markers) per file. The file per harness: Claude as today; Codex, Factory, Copilot and `agents` use `AGENTS.md`; Pi uses the first of `AGENTS.override.md`, `AGENTS.md` or `CLAUDE.md` in the root; Gemini uses `AGENTS.md` when its `context.fileName` lists it, else `GEMINI.md`; Cursor gets a whole file, `.cursor/rules/<tool>.mdc` with `alwaysApply: true`, and `AGENTS.md` in a set that has it.
- D10-7: **MCP servers and TOML merges this round.**
  - A neutral `McpServer` (stdio: command, args, env; or http: url, headers), rendered per harness as `mcpServers.<name>`, or `servers.<name>` for VS Code.
  - `MergeOp` gains a TOML form for object members. It is used for Codex `[mcp_servers.<name>]` and uses `toml_edit` (already a dependency), keeping comments and order. The merge part's format comes from the file's extension.
- D10-8: **Allowed commands.** A neutral "allow my CLI" (a command prefix), rendered where a file holds it: Claude `permissions.allow`, Cursor `.cursor/cli.json` `permissions.allow` (`Shell(…)`), Gemini `tools.allowed`, Factory `permissionRules`. Unsupported for the others (Codex rule files are unverified, D10-11).
- D10-9: **Notes after install.** `HarnessResult` gains notes per harness, saying what the user still has to do: approve hooks (Codex `/hooks`), trust the folder (Codex, Gemini, Pi), reload (Pi `/reload`, Gemini `/memory reload`).
- D10-10: **No new dependency.** TOML uses `toml_edit`. The Pi extension is a template in the crate. YAML (Aider, Goose) stays out (D10-1).
- D10-11: **Unverified facts are checked before they are coded:**
  - Pi: that a `custom_message` entry reaches the model, and the `canContinue` guard.
  - Factory: whether `CLAUDE.md` is read when `AGENTS.md` exists.
  - Codex: the rule-file syntax (permissions) and the Windows home.
  - Gemini: how hooks from several scopes combine.

  Each is settled by the docs or by a manual run, and its row in §10 is updated. Until then the item is unsupported for that harness.

- D10-12: The version stays 0.1.0 and nothing is published until this plan is done. PLAN-009 P8 (publish) and P9 (switch consumers) run as this plan's P9.
- D10-13: Module layout, each module with one job:
  - `integration`: the neutral declaration, pure data. `Integration` and its items: `Instructions`, `Skill`, `Hook` (with `Event`), `McpServer`, `AllowedCommand`, `Agent`, `Command`, and raw parts for one harness (`Integration::part(harness_id, Part)`).
  - `hook`: the neutral hook runtime. `Event`, `HookInput`, `Answer`, `emit(harness, …)`. `LoopGuard` moves here.
  - `harness`: the install engine and the adapter contract. The `Harness` trait, `Profile`, `Part`, `MergeOp`, `install`, `status` and the result types, plus `builtin()` (every built-in harness) and `find(id)`.
  - One module per harness at the root (D9-17): `claude`, `codex`, `factory`, `gemini`, `copilot`, `cursor`, `pi`, `agents_md` (id `agents`). Each exposes a unit struct implementing `Harness` (`claude::Claude`, …) and its own extras (e.g. `claude::instructions_file`).
  - `report`, `cli`, `fs` are unchanged.
- D10-14: A tool lists the harnesses it supports in `Tool::harnesses()`; `harness::builtin()` means all of them. `install` / `status` refuse a harness the tool does not list (unknown profile, as now).
- D10-15: `Scope::Local`, where a harness has a local, git-ignored file: Claude `.claude/settings.local.json`, Factory `.factory/settings.local.json`, Copilot `.github/copilot/settings.local.json`. It takes hooks and allowed commands only, under the project root. Other harnesses, and other items, are unsupported at local scope (D10-3). Whether Factory reads hooks from `settings.local.json` is checked first (D10-11).

- D10-16: Hook commands are as flexible as possible: no required word (`hook`) or argument order; placeholders optional, anywhere, `{{` / `}}` for literal braces. Ownership defaults per hook to the command's text before its first placeholder (the whole command when it has none); the tool can set one entry match for all its hooks or one per hook. An empty default (a command starting with a placeholder) needs an explicit match, else an internal error.

## 4. Functional requirements

| ID | Requirement |
| -- | ----------- |
| F1 | `Integration` and its items (D10-3), declared once per tool and scope; `Tool` gives `integration(scope)` and `harnesses()` (D10-14) instead of `profile`. |
| F2 | `Harness` trait: id, the locations it reads per item kind, render to `Profile`, hook parse and answer; one implementation per §2 row. |
| F3 | `install` / `status` over a set of harnesses with the dedupe of D10-5; new state `shared`; notes (D10-9); `Scope::Local` (D10-15). |
| F4 | Neutral hooks (D10-4) in the `hook` module: events, `HookInput`, answers, `emit` per harness; `Harness` lookup by id for the hook command. |
| F5 | Pi extension template and file part (D10-4). |
| F6 | Instructions rules per harness (D10-6). |
| F7 | MCP servers per harness; TOML object-member merge (D10-7). |
| F8 | Allowed commands (D10-8). |
| F9 | Skills, agents and commands rendered per harness: skills as `<name>/SKILL.md` directories; agents as Markdown (Claude, Gemini, Factory, Cursor, Copilot `*.agent.md`) or TOML (Codex); commands as Markdown (Claude, Cursor, Factory, Pi prompts), TOML (Gemini) or skills where commands are deprecated (Codex, Copilot CLI). |
| F10 | Specs: ARCHITECTURE, REQ-KIT (new requirements; the "other harnesses" out-of-scope line goes), DESIGN-KIT, all before code. |
| F11 | README with a multi-harness example; the CHANGELOG 0.1.0 section rewritten for the new API. |
| F12 | smllm and sokf ported to the neutral API on branches, validated with `scripts/validate-consumers.sh`. |

## 5. Non-functional requirements

| ID | Requirement |
| -- | ----------- |
| N1 | No new runtime dependency (D10-10); MSRV stays 1.85. |
| N2 | Line coverage ≥ 90%. Each harness's rendered files are snapshot-tested (`insta`), and each hook parse and answer has a test per harness from documented examples. |
| N3 | Files ≤ 800 lines; one module per harness; `harness` stays neutral. |
| N4 | A manual smoke check per harness (install a test tool, run its stop and pre-tool hooks) is recorded in §8 before release. |

## 6. Phases

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P1 Specs | ARCHITECTURE, REQ-KIT, DESIGN-KIT for D10-3..D10-9 (F10). | Every new AC is assigned to a component |
| P2 Core | `Integration`, `Harness` trait, set install / status, `shared` state, dedupe, notes, TOML merge (F1–F3, F7 core). `claude` rebuilt on it. | Existing tests pass in their new form; dedupe properties hold |
| P3 Hooks | Neutral hook model, Claude on it (F4). | Claude hook tests pass |
| P4 Claude family | `codex`, `factory`, `gemini`, `agents` (F6–F9). | Snapshots and hook tests per harness |
| P5 Flat family | `cursor`, `copilot`. | As P4 |
| P6 Pi | `pi`, with the extension (F5). | As P4, plus the extension type-checks against the Pi package |
| P7 Docs | README, CHANGELOG (F11); D10-11 settled; smoke checks (N4). | `cargo publish --dry-run` passes |
| P8 Consumers | F12. | smllm and sokf pass on the checkout |
| P9 Release | PLAN-009 P8 and P9: tag `v0.1.0`, consumers move to `"0.1"`. | On crates.io; consumers' CI green |

## 7. Risks

- The harnesses change fast; §10 was checked on 2026-10-06. Each harness module names the doc pages it follows, so they can be rechecked.
- Dedupe depends on cross-reading behaviour that may change (Cursor reading Claude's files). It lives in data per harness, so a change is a one-line fix.
- Partly unverified harnesses (D10-11) may ship with fewer items supported.
- The Pi extension is TypeScript the kit cannot run in CI. Mitigation: P6 type-checks it once and records that in §8; smoke check (N4).

## 8. Implementation notes

- P2: one group per hook could not hold two hooks on one event (e.g. two matchers), so `MergeOp::group_entry` became `group_entries` (all the tool's groups for an event, placed by their other keys) and `owned_entries` holds all of an event's entries. Each harness emits an op for every event it has; an empty op only removes the tool's old entries, creates nothing and expects nothing, so a hook the tool dropped makes the part stale (KIT-10_AC-6). `EntryMatch::Any` joins the hooks' matches.
- P2: only the named harnesses are written and reported; harnesses from the record take part in the choice only. A chosen location is written by the earliest named harness that renders it (KIT-19_AC-3).
- P2: `Part::region_chosen` / `ChooseFile` went: a harness picks the file when it renders, from the tree. `Profile::new` is crate-private.
- P2: a TOML file of comments only keeps them first when a table is added (`toml_edit` would move them below).
- P2: `tests/shared.rs` drives `install` / `status` / `emit` through three harnesses defined in the test, as a third party would.
- P3: `LoopGuard` moved to `hook`; the README examples use the new API.
- P4: Gemini CLI loads every name `context.fileName` lists, so all count as read; the region goes in `AGENTS.md` when listed, else the first name (HAR-4_AC-2 sharpened). Factory's user skills stay in `~/.factory/skills` (its `~/.agents` support is unconfirmed). `builtin()` puts the generic `agents` last, so a shared location is written by a specific harness.
- P5: Copilot's parser reads both casings (the VS Code agent sends Claude-style fields, the CLI camelCase), and falls back to Claude's tool names. Cursor's tool names are classified by what they contain (unconfirmed). Claude's parser hands a payload with `cursor_version` to Cursor's (HAR-9_AC-2).
- P6, D10-11 for Pi settled from `@earendil-works/pi-coding-agent` 1.0.4 (`dist/core/extensions/types.d.ts`, `docs/extensions.md`, `docs/session-format.md`): `agent_before_settle` gives `context.canContinue` and takes `{ continue, entries }`; a `custom_message` entry takes part in the model's context. The extension also maps context answers (`before_agent_start` message, `tool_result` content) and `session_before_compact`.
- P6 check: a generated extension type-checks (`tsc --strict`, TypeScript 5) against the 1.0.4 package and `@types/node`, and a deliberate type error fails. Run under Node 26 with a stand-in `pi` and real shell commands as hooks: a tool call is blocked with the hook's reason and the hook gets the event JSON on stdin; a stop continues once with the reason as a `custom_message`, then allows when the hook sees `continuing: true`; nothing is asked when the context cannot continue; a hook that times out or exits non-zero allows. A run inside Pi itself is part of the smoke checks (N4).
- P7: README (harness table, multi-harness example) and CHANGELOG 0.1.0 rewritten; `cargo publish --dry-run` passes; line coverage 98.5%. D10-11 left open, so these stay unsupported: Codex allowed commands (rule-file syntax), Factory allowed commands (`permissionRules` shape) and local scope (hooks in `settings.local.json`), Copilot prompt deny and context answers. Gemini CLI's hooks merge across scopes or not: no effect on what the kit writes (one file per scope). The smoke checks inside each real agent (N4) need the agents installed and signed in.

## 9. Resolved questions

- Q1 → D10-12. Q2 → D10-13. Q3 → D10-14. Q4 → D10-15.

## 10. Harness facts (checked 2026-10-06)

| | Instructions | Hooks | Skills | MCP | Other |
| - | - | - | - | - | - |
| claude | `CLAUDE.md` / `AGENTS.md` rule (KIT-11_AC-1) | `.claude/settings.json` `hooks.<Event>[]` | `.claude/skills` | `.mcp.json` | agents `.claude/agents`, commands `.claude/commands` |
| codex | `AGENTS.override.md` / `AGENTS.md`, git root → cwd, 32 KiB cap | `.codex/hooks.json`, Claude shape and protocol; `stop_hook_active` | `.agents/skills` | `.codex/config.toml` `[mcp_servers.<n>]` | agents `.codex/agents/*.toml`; trust; `/hooks` approval |
| factory | `AGENTS.md` (also `.factory/`, `CLAUDE.md`) | `.factory/hooks.json`, events top level, Claude protocol; tools `Execute`, `Create`, … | `.factory/skills`, `.agents/skills` | `.factory/mcp.json` | `permissionRules`; droids `.factory/droids/*.md`; commands `.factory/commands` |
| gemini | `GEMINI.md`, or `context.fileName`; `@file` imports | `.gemini/settings.json` `hooks.<Event>[]`; `BeforeTool`, `AfterTool`, `BeforeAgent`, `AfterAgent`, `SessionStart`, `SessionEnd`, `PreCompress`; ms timeouts | `.gemini/skills`, `.agents/skills` | `.gemini/settings.json` `mcpServers` | `tools.allowed`; commands `.gemini/commands/*.toml`; agents `.gemini/agents/*.md`; folder trust |
| copilot | `AGENTS.md`, `.github/copilot-instructions.md`, `CLAUDE.md` | `.github/hooks/<file>.json` `{version:1,hooks:{preToolUse,agentStop,…}}`, `bash`/`powershell`, camelCase input, `permissionDecision`, `decision:block` | `.github/skills`, `.agents/skills`, `.claude/skills` | CLI `.mcp.json` / `.github/mcp.json`; VS Code `.vscode/mcp.json` `servers`; user `~/.copilot/mcp-config.json` | agents `.github/agents/*.agent.md`; user `~/.copilot/` |
| cursor | `.cursor/rules/*.mdc`, `AGENTS.md`; user rules not in files | `.cursor/hooks.json` `{version:1,hooks:{stop:[{command}]}}`; `loop_count`, `followup_message`, `permission`; reads Claude's hooks | `.cursor/skills`, `.agents/skills`, `.claude/skills` | `.cursor/mcp.json` | `.cursor/cli.json` `permissions.allow`; agents `.cursor/agents` (also `.claude/agents`) |
| pi | first of `AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md` per dir | TS extension `.pi/extensions/`; `tool_call` → `{block,reason}`; `agent_before_settle` → `{continue}` | `.pi/skills`, `.agents/skills` | `.pi/mcp.json` | prompts `.pi/prompts/*.md`; trust; `/reload` |
| agents | `AGENTS.md` | none | `.agents/skills` | none | none |

Sources: learn.chatgpt.com/docs (Codex), docs.factory.com, geminicli.com/docs, docs.github.com/copilot and code.visualstudio.com/docs/copilot, cursor.com/docs, github.com/earendil-works/pi (`packages/coding-agent/docs`).
