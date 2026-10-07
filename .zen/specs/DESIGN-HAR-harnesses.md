# Design Specification

## Overview

Implements REQ-HAR: one module per harness, each a unit struct implementing `Harness` (DESIGN-KIT KIT-Adapter). A module is a table of facts (paths per scope, event names, tool names, matchers) plus three functions: `render`, `reads` and the hook IO. What several harnesses share lives in the crate-private `common` module (the Claude-family protocol, the instructions region, group hooks, skill dirs, MCP JSON), so equal locations get equal parts (HAR-9_AC-1, KIT-19_AC-5). Every module names the documentation pages it follows in its module doc.

## Architecture

AFFECTED LAYERS: harness modules

### High-Level Architecture

```mermaid
flowchart LR
    Core[harness core] -->|render, reads| Mod[claude / codex / factory / gemini / copilot / cursor / pi / opencode / kilo / qwen / devin / agents_md]
    Hook[hook::HookInput / emit] -->|parse_hook, answer| Mod
    Mod --> Common[common: claude protocol, extension format, OpenCode-family plugin, Gemini-family settings, instructions, group hooks, skills, MCP JSON]
    Mod --> Items[integration items: dir_files, to_json, to_markdown]
```

### Module Organization

```
crates/lib/agent-harness-adapter-core/src/
├── common/            crate-private
│   ├── mod.rs
│   ├── protocol.rs    Claude-family input fields and answers
│   ├── extension.rs   the generated extensions' input and answers (Pi, OpenCode, Kilo), template filling
│   ├── plugin.rs      the OpenCode family (OpenCode, Kilo): `Layout` (paths per agent → parts, reads), the plugin
│   ├── plugin.ts      the plugin template (include_str!)
│   ├── settings.rs    the Gemini family (Gemini CLI, Qwen Code): context files from `context.fileName`, `httpUrl` MCP entries
│   ├── parts.rs       instructions region, group hook ops, skills, MCP JSON, markdown agents and commands
│   └── toml_out.rs    small TOML documents (Codex agents, Gemini commands) via toml_edit
├── claude/   mod.rs, adapter.rs (Claude, instructions_file)
├── codex/    mod.rs, adapter.rs
├── factory/  mod.rs, adapter.rs
├── gemini/   mod.rs, adapter.rs
├── copilot/  mod.rs, adapter.rs
├── cursor/   mod.rs, adapter.rs
├── pi/       mod.rs, adapter.rs, extension.ts (template, include_str!)
├── opencode/ mod.rs, adapter.rs, tests.rs
├── kilo/     mod.rs, adapter.rs
├── qwen/     mod.rs, adapter.rs, tests.rs
├── devin/    mod.rs, adapter.rs, tests.rs
└── agents_md/ mod.rs, adapter.rs
```

### Architectural Decisions

- ONE LOCATION PER ITEM PER HARNESS: a harness renders each item at exactly one location, the most shared one it always loads (`AGENTS.md`, `.agents/skills`, `.mcp.json`), except where its own location is required; KIT-Shared then removes copies. Alternatives: a harness offering several candidate locations (bigger search, same results for the sets in REQ-HAR)
- CLAUDE-FAMILY PROTOCOL SHARED: Claude, Codex and Factory parse the same input fields and give the same answers through `common::protocol`; Gemini reuses the parser with its own field and answer differences
- CURSOR DETECTION IN CLAUDE: Claude's parser hands a payload with `cursor_version` to Cursor's (HAR-6_AC-4), because the Cursor CLI runs Claude's hooks; Cursor accepts Claude-format answers, so answers need no hand-off. Alternatives: detection in the core (the core would know harnesses)
- MATCHERS ONLY WHERE CONFIRMED: a hook's tool kind becomes a matcher only where the harness's tool names are confirmed (Claude, Factory, Gemini; Codex `Bash`); elsewhere no matcher, and the hook filters on `HookInput.tool.kind`
- PI EXTENSION OWNS TRANSLATION: Pi has no command hooks; the library writes one TypeScript extension per tool that maps Pi events to the tool's command (JSON on stdin through `node:child_process`, async with the hook's timeout) and maps the library's neutral answer JSON back. Failures and bad output allow, so a broken tool never blocks Pi
- OPENCODE PLUGIN LIKE PI'S EXTENSION: OpenCode has no command hooks either; the library writes one plugin per tool in the same way, with the same input and answer format (`common::extension`). OpenCode has no hook that can refuse the end of a turn, so a stop runs on the `session.idle` event and a continue sends a new prompt; `opencode run` exits at the first idle, so this reaches the model in OpenCode's TUI and server, not in `opencode run`. A stop can be reported idle more than once (Kilo Code marks a session idle from its runner and its stream processor): the plugin decides each stop once, ignoring idle events while a stop is decided or its continuation has not arrived. Alternatives: guessing the last step in `experimental.text.complete` (it fires for every text part, so stop would run mid-turn)
- FORKS SHARE THEIR PARENT'S RENDERING: Kilo Code (an OpenCode fork) and OpenCode differ only in paths, names and the types package: `common::plugin::Layout` renders both. Qwen Code (a Gemini CLI fork) shares Gemini's context-file choice and `httpUrl` MCP entries (`common::settings`), with Claude Code's hook names and answers (`common::protocol`)
- DEVIN READS CLAUDE CODE'S FILES: Devin loads `CLAUDE.md`, `.claude/skills` and `.mcp.json` by default, so its `reads` list them (always) and those items are shared with Claude Code; Claude Code's hooks would also run in Devin, with a different tool vocabulary, so `.claude/settings.json` is a maybe-read and a set with both warns
- UNCONFIRMED FACTS ARE UNSUPPORTED: an item or answer resting on an unconfirmed fact (REQ-HAR notes) is not rendered, or is `Error::Unsupported`, until confirmed (PLAN-010 D10-11)

## Components and Interfaces

Common shape of every module:

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct Claude;              // likewise Codex, Factory, Gemini, Copilot, Cursor, Pi, OpenCode, Kilo, Qwen, Devin, AgentsMd
impl Harness for Claude { /* KIT-Adapter */ }
```

Tool kinds (`ToolKind` from the tool name; unlisted names are `Other`):

| Harness | Shell | Read | Write | Mcp |
| ------- | ----- | ---- | ----- | --- |
| claude | `Bash` | `Read`, `Grep`, `Glob`, `LS` | `Edit`, `Write`, `MultiEdit`, `NotebookEdit` | `mcp__*` |
| codex | `Bash`, `shell` | — | `apply_patch` | `mcp__*` |
| factory | `Execute` | `Read`, `LS`, `Glob`, `Grep` | `Edit`, `Create`, `ApplyPatch` | `mcp__*` |
| gemini | `run_shell_command` | `read_file`, `read_many_files`, `glob`, `search_file_content`, `list_directory` | `write_file`, `replace` | `mcp_*` |
| copilot | `bash`, `powershell` | `view` | `edit`, `create` | `*/*` (unconfirmed) |
| cursor | `Shell` | `Read` | `Write`, `Edit` | `MCP:*` (unconfirmed) |
| pi | `bash` | `read`, `grep`, `find`, `ls` | `edit`, `write` | — |
| opencode, kilo | `bash` | `read`, `glob`, `grep`, `list` | `edit`, `write`, `apply_patch` | — (`<server>_<tool>`, not told from its own) |
| qwen | `run_shell_command` | `read_file`, `grep_search`, `glob`, `list_directory` | `write_file`, `edit`, `notebook_edit` | `mcp__*` |
| devin | `exec` | `read`, `grep`, `glob`, `notebook_read` | `write`, `edit`, `apply_patch`, `notebook_edit` | `mcp__*` |

### HAR-Common

`protocol::parse(raw, fields)` fills `HookInput` from snake_case Claude-family fields (`stop_hook_active` → `continuing`; `last_assistant_message` or Gemini's `prompt_response` → `last_message`). `protocol::answer(event_name, answer)` gives HAR-1_AC-5's forms. `parts::instructions(file, block)` is `Part::region("instructions", file, block)`; `parts::group_hooks(layout, integration)` is one `MergeOp::group_entries` per event the harness has (`layout`: path, event names, matchers, timeout unit), the event's hooks grouped by matcher as `{"matcher"?, "hooks":[{"type":"command","command","timeout"?}]}`, owned by `EntryMatch::Any` of the hooks' matches (KIT-11_AC-7); Cursor's and Copilot's local owned entries use the same owner; `parts::skills(dir, skills)` is `Part::files("skills", dir, all dir_files)`; `parts::mcp_json(file, key, servers, to_json)` is one `object_member([key], name, json)` per server.

IMPLEMENTS: HAR-9_AC-1

### HAR-Claude

Scopes project, user, local. Instructions: `instructions_file(root)` at project scope (the 0.1 rule, kept public), `.claude/CLAUDE.md` at user scope; `reads(Instructions)` always that file. Hooks: group hooks under `["hooks", Event]` of `.claude/settings.json` / `.claude/settings.local.json`, matcher from the tool-kind table (`Bash`, `Read|Grep|Glob|LS`, `Edit|Write|MultiEdit|NotebookEdit`, `mcp__.*`), `timeout` in seconds. Permissions: `array_entry(["permissions","allow"], "Bash(<prefix> *)")` in the same file. Skills `.claude/skills`; agents `.claude/agents` (`Agent::to_markdown(&[])`); commands `.claude/commands/<name>.md` (frontmatter `description`, the prompt as body). MCP `.mcp.json` at project scope only. `parse_hook` hands `cursor_version` payloads to `Cursor`.

IMPLEMENTS: HAR-1_AC-1, HAR-1_AC-2, HAR-1_AC-3, HAR-1_AC-4, HAR-1_AC-5, HAR-1_AC-6, HAR-1_AC-7, HAR-1_AC-8, HAR-9_AC-2

```rust
pub struct Claude;
pub fn instructions_file(root: &Path) -> Result<&'static str>;
```

### HAR-Codex

Scopes project, user. Instructions `AGENTS.md` / `.codex/AGENTS.md`. Hooks: group hooks under `["hooks", Event]` of `.codex/hooks.json`, Claude names, matcher `^Bash$` for shell only, the Claude-family protocol. Skills `.agents/skills`. Agents `.codex/agents/<name>.toml` (`name`, `description`, `developer_instructions`) via `toml_out`. MCP: `object_member(["mcp_servers"], name, {command, args, env} | {url, http_headers})` in `.codex/config.toml` (TOML merge). Notes: trust, `/hooks` approval when a hooks part was written, restart.

IMPLEMENTS: HAR-2_AC-1, HAR-2_AC-2, HAR-2_AC-3, HAR-2_AC-4, HAR-2_AC-5, HAR-2_AC-6, HAR-2_AC-7

### HAR-Factory

Scopes project, user. Instructions `AGENTS.md` / `.factory/AGENTS.md`; `reads` always `AGENTS.md`, maybe `CLAUDE.md`. Hooks: when `.factory/hooks.json` exists, or `.factory/settings.json` has no `hooks` key: group hooks under `[Event]` of `.factory/hooks.json`; else under `["hooks", Event]` of `.factory/settings.json`; matchers from the table. Skills `.agents/skills` (`.factory/skills` at user scope); droids `.factory/droids` (`to_markdown(&[("model","inherit")])`); commands `.factory/commands/<name>.md`. MCP `.factory/mcp.json` with `type`.

IMPLEMENTS: HAR-3_AC-1, HAR-3_AC-2, HAR-3_AC-3, HAR-3_AC-4, HAR-3_AC-5, HAR-3_AC-6

### HAR-Gemini

Scopes project, user. Instructions: the context files are the names `context.fileName` (a string or an array) lists in `.gemini/settings.json` under the root, else under `user_root` at project scope, else `GEMINI.md` (under `.gemini/` at user scope); `reads` always all of them; the region goes in `AGENTS.md` when listed, else the first. Hooks: group hooks under `["hooks", Event]` of `.gemini/settings.json`, Gemini names, `timeout` in ms, matchers from the table. Answers: allow `{}`; deny / continue `{"decision":"deny","reason":…}`; context `{"hookSpecificOutput":{"additionalContext":…}}`. Skills `.agents/skills`; agents `.gemini/agents`; commands `.gemini/commands/<name>.toml` (`description`, `prompt` with `$ARGUMENTS` → `{{args}}`). MCP `mcpServers` in `.gemini/settings.json` (`httpUrl` for http). Permissions `array_entry(["tools","allowed"], "run_shell_command(<prefix>)")`. Notes: folder trust.

IMPLEMENTS: HAR-4_AC-1, HAR-4_AC-2, HAR-4_AC-3, HAR-4_AC-4, HAR-4_AC-5, HAR-4_AC-6, HAR-4_AC-7, HAR-4_AC-8, HAR-4_AC-9

### HAR-Copilot

Scopes project, user, local. Instructions `AGENTS.md` / `.copilot/copilot-instructions.md`; `reads` always `AGENTS.md`, `CLAUDE.md`, `.github/copilot-instructions.md`. Hooks: `Part::files("hooks", ".github/hooks" | ".copilot/hooks", [("<tool>.json", doc)])`, `doc = {"version":1,"hooks":{event:[{"type":"command","bash":cmd,"powershell":cmd,"timeoutSec":n}]}}`; at local scope `owned_entry(["hooks", event], "bash", owner, entry)` in `.github/copilot/settings.local.json`; `reads(Hooks)` maybe `.claude/settings.json`. Input: camelCase fields, `toolArgs` parsed when it is a string. Answers: allow `{}`; deny before a tool `{"permissionDecision":"deny","permissionDecisionReason":…}`; continue at `agentStop` `{"decision":"block","reason":…}`; others `Unsupported`. Skills `.agents/skills` (`reads` always also `.claude/skills`, `.github/skills`); agents `.github/agents/<name>.agent.md` (`reads` maybe `.claude/agents`); MCP `.mcp.json` / `.copilot/mcp-config.json`.

IMPLEMENTS: HAR-5_AC-1, HAR-5_AC-2, HAR-5_AC-3, HAR-5_AC-4, HAR-5_AC-5, HAR-5_AC-6, HAR-5_AC-7, HAR-5_AC-8, HAR-5_AC-9, HAR-5_AC-10

### HAR-Cursor

Scopes project, user. Instructions `AGENTS.md` at project scope; `reads` always `AGENTS.md`, maybe `CLAUDE.md`; none at user scope. Hooks: `object_member([], "version", 1)` and one `owned_entry(["hooks", event], "command", owner, {"command", "timeout"?})` per hook in `.cursor/hooks.json`; `reads(Hooks)` maybe `.claude/settings.json`. Input: `conversation_id`, first of `workspace_roots`, `loop_count > 0` → `continuing`. Answers per HAR-6_AC-5; context other than at session start `Unsupported`. Skills `.agents/skills` (`reads` always also `.cursor/skills`, `.claude/skills`); agents `.cursor/agents` (`model: inherit`; `reads` always also `.claude/agents`); commands `.cursor/commands/<name>.md` (body only). MCP `.cursor/mcp.json`. Permissions `array_entry(["permissions","allow"], "Shell(<first word>)")` in `.cursor/cli.json` / `.cursor/cli-config.json`.

IMPLEMENTS: HAR-6_AC-1, HAR-6_AC-2, HAR-6_AC-3, HAR-6_AC-4, HAR-6_AC-5, HAR-6_AC-6, HAR-6_AC-7, HAR-6_AC-8

### HAR-Pi

Scopes project, user (paths under `.pi/agent`). Instructions: the first existing of `AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md` in the directory, else `AGENTS.md`; `reads` always that file only. Hooks: `Part::files("hooks", ".pi/extensions", [("<tool>.ts", ts)])`, `ts` = `extension.ts` with the tool name, header and a `HOOKS` table (Pi event, command, timeout) filled in; Pi events `session_start`, `session_shutdown`, `before_agent_start`, `tool_call`, `tool_result`, `agent_before_settle` (no `PreCompact`). The extension writes `{"event","cwd","prompt"?,"tool_name"?,"tool_input"?,"tool_output"?,"continuing"}` to the command's stdin and reads `{"answer","reason"?,"text"?}`: `deny` on `tool_call` → `{ block: true, reason }`; `continue` on `agent_before_settle` → `{ continue: true, entries: [{ type: "custom_message", customType: <tool>, content: reason, display: true }] }` at every settle (custom messages take part in the model's context per `session-format.md`; `event.context.canContinue` is not a guard: it is false whenever the model's answer is last, i.e. at every stop, and Pi accepts the continuation once the entry is added, checked against 1.0.4's `agent-session.js` and a real run, PLAN-013); `context` on `session_start` → kept, on `before_agent_start` → `{ message }` holding the kept text then the prompt's, on `tool_result` → the content plus a text block; else nothing. The extension remembers whether it asked to continue and sends that as `continuing` at the next stop; `before_agent_start` (a new prompt) resets it. `answer` gives that JSON for every answer the extension can act on, `Unsupported` otherwise. Skills `.agents/skills`; commands `.pi/prompts/<name>.md` (frontmatter `description`); MCP `.pi/mcp.json`. Notes: trust, `/reload`.

IMPLEMENTS: HAR-7_AC-1, HAR-7_AC-2, HAR-7_AC-3, HAR-7_AC-4, HAR-7_AC-5, HAR-7_AC-6, HAR-7_AC-7

### HAR-OpenCode

Scopes project, user (OpenCode's own paths under `.config/opencode`). Instructions: the first existing of `AGENTS.md`, `CLAUDE.md` (`.config/opencode/AGENTS.md`, `.claude/CLAUDE.md` at user scope), else the first; `reads` always that file only. Hooks: `Part::files("hooks", ".opencode/plugins" | ".config/opencode/plugins", [("<tool>.ts", ts)])`, `ts` = `plugin.ts` filled by `extension::fill`; its default export is the only export (OpenCode refuses a plugin file with any other). The plugin keeps per session: whether it is a subagent's (`session.created` with `parentID`), whether it started, whether the last stop continued, and whether the next message is its own continuation. `chat.message` (not the plugin's own, not a subagent's): resets `continuing`; at a session's first message runs session-start (`source: "startup"`) and keeps its context; runs prompt-submit with the message's text; pushes the kept and the prompt's context as one text part `{id: "prt_<time><random>", sessionID, messageID, type: "text", text, synthetic: true}`. `tool.execute.before`: `deny` → `throw new Error(reason)`. `tool.execute.after`: `context` → `output.output += "\n\n" + text`. `session.idle`: stop with `continuing`; `continue` → `client.session.promptAsync({path: {id}, body: {agent, model, parts: [{type: "text", text: reason}]}})` with the session's last agent and model. `experimental.session.compacting`: pre-compact. Input and answers: `extension::parse` / `extension::answer`. Skills `.agents/skills` (`reads` always also `.claude/skills`, `<base>/skills`); agents `<base>/agents/<name>.md` (`to_markdown(&[("mode","subagent")])`); commands `<base>/commands/<name>.md`. MCP and permissions in `opencode.json` / `.config/opencode/opencode.json`: `object_member(["mcp"], name, {type: "local", command: [..], environment?} | {type: "remote", url, headers?})`; `object_member(["permission","bash"], "<prefix> *", "allow")`, no part without allowed commands. Notes: restart.

IMPLEMENTS: HAR-10_AC-1, HAR-10_AC-2, HAR-10_AC-3, HAR-10_AC-4, HAR-10_AC-5, HAR-10_AC-6, HAR-10_AC-7, HAR-10_AC-8, AHA-2_AC-3

### HAR-Kilo

`common::plugin::Layout { harness: "kilo", agent: "Kilo Code", types: "@kilocode/plugin", project_dir: ".kilo", user_dir: ".config/kilo", config: "kilo.json", also_config: ["opencode.json"] }`: everything as HAR-OpenCode under Kilo's paths. `reads(Mcp | Permissions)` always `kilo.json` and `opencode.json`, so with OpenCode in the set they are shared in OpenCode's `opencode.json`; Kilo does not read `.opencode`, so the plugin is written for each. Notes: restart.

IMPLEMENTS: HAR-12_AC-1, HAR-12_AC-2, HAR-12_AC-3, HAR-12_AC-4, HAR-12_AC-5, HAR-12_AC-6, AHA-2_AC-3

### HAR-Qwen

Scopes project, user (paths under `.qwen`). Instructions: `common::settings::context_files` with `.qwen/settings.json`, default `QWEN.md`, `AGENTS.md`; `reads` always all of them. Hooks: group hooks under `["hooks", Event]` of `.qwen/settings.json`, Claude names, `timeout` in seconds, matchers from the table; the Claude-family protocol. MCP `mcpServers` with `common::settings::mcp_entry`; permissions `array_entry(["permissions","allow"], "Bash(<prefix> *)" | "mcp__<server>__<tool>")`, all in `.qwen/settings.json`. Skills `.agents/skills` (`reads` always also `.qwen/skills`); agents `.qwen/agents` (`to_markdown(&[])`); commands `.qwen/commands/<name>.md` (`Command::to_markdown` with `$ARGUMENTS` → `{{args}}`). Notes: folder trust, MCP approval, restart.

IMPLEMENTS: HAR-13_AC-1, HAR-13_AC-2, HAR-13_AC-3, HAR-13_AC-4, HAR-13_AC-5, HAR-13_AC-6, HAR-13_AC-7, HAR-13_AC-8

### HAR-Devin

Scopes project, user (Devin's paths under `.config/devin`). Instructions `AGENTS.md` / `.config/devin/AGENTS.md`; `reads` always `AGENTS.md`, `CLAUDE.md` (maybe `.claude/CLAUDE.md` at user scope). Hooks: group hooks under `[Event]` of `.devin/hooks.v1.json` (`["hooks", Event]` of `.config/devin/config.json`), Claude names without `PreCompact`, `timeout` in seconds, matchers from the table (unanchored regular expressions, so anchored); `reads(Hooks)` maybe `.claude/settings.json`. Input: `protocol::parse`, `cwd` from `DEVIN_PROJECT_DIR` when absent. Answers: allow `{}`; deny (tool, prompt) and continue `{"decision":"block","reason"}` (Devin's documented form); context `hookSpecificOutput.additionalContext`. Skills `.agents/skills` (`reads` always also `.devin/skills`, `.claude/skills`, `.github/skills`; user `.config/devin/skills`, `.claude/skills`); agents `.devin/agents` (`to_markdown(&[])`; `reads` maybe `.agents/agents`, `.claude/agents`); MCP `mcpServers` in `.devin/mcp_config.json` (`reads` always also `.mcp.json` at project scope); permissions `array_entry(["permissions","allow"], "Exec(<prefix>)" | "mcp__<server>__<tool>")` in `.devin/config.json`. Commands unsupported. Notes: trust, new session.

IMPLEMENTS: HAR-11_AC-1, HAR-11_AC-2, HAR-11_AC-3, HAR-11_AC-4, HAR-11_AC-5, HAR-11_AC-6, HAR-11_AC-7, HAR-11_AC-8, HAR-11_AC-9

### HAR-AgentsMd

Id `agents`. Scopes project, user. Instructions `AGENTS.md` at project scope; skills `.agents/skills`; nothing else. `parse_hook` and `answer` give `Unsupported` (it installs no hooks).

IMPLEMENTS: HAR-8_AC-1, HAR-8_AC-2, HAR-8_AC-3

## Data Models

### Core Types

- EXTENSION ANSWER: `{"answer":"allow"|"deny"|"continue"|"context","reason"?:string,"text"?:string}` — the library's own wire format between a generated extension (Pi's extension, OpenCode's plugin) and the tool

## Correctness Properties

- HAR_P-1 [Answers well formed]: for every built-in harness, event and answer, `answer` is `Unsupported` or exit 0 with stdout that parses as one JSON object
  VALIDATES: KIT-11_AC-4, HAR-1_AC-5, HAR-4_AC-5, HAR-5_AC-5, HAR-6_AC-5, HAR-7_AC-4, HAR-10_AC-4, HAR-11_AC-5, HAR-12_AC-4, HAR-13_AC-4
- HAR_P-2 [Shared renders equal]: for any integration and scope, every harness that renders an item at the same location renders an equal part
  VALIDATES: HAR-9_AC-1
- HAR_P-3 [Parse total]: for every harness and event, any JSON object parses without error, and unknown fields are ignored
  VALIDATES: KIT-11_AC-3

## Error Handling

### Error

- UNSUPPORTED: an answer the harness cannot express for the event (KIT-Error)
- FILE: a settings file read to decide a location (Gemini `context.fileName`, Factory `hooks`) does not parse; refused before any write

### Strategy

PRINCIPLES:

- Unconfirmed facts make an item unsupported, never a guess
- The Pi extension and the OpenCode plugin fail open

## Testing Strategy

### Property-Based Testing

- FRAMEWORK: proptest
- MINIMUM_ITERATIONS: 64
- TAG_FORMAT: @zen-test: HAR_P-{n}

### Unit Testing

Per module: rendered parts of a full integration on an empty tree, per scope (`insta` snapshots); trees that change a choice (Claude and Pi instructions, Gemini `context.fileName`, Factory hooks file); `reads` per item; input parsing from each harness's documented example payloads; every answer form; tool kinds from the table.
- AREAS: render, reads, parse_hook, answer, notes, the Pi extension and OpenCode plugin text

### Integration Testing

`crates/lib/agent-harness-adapter-core/tests/` installs a full integration into each harness and into the sets {claude, codex}, {claude, cursor}, {codex, gemini, pi, agents}, {claude, codex, copilot}, {claude, opencode}, {opencode, kilo}, {claude, devin}, {codex, qwen}, checking shared parts and warnings. The Pi extension is type-checked against `@earendil-works/pi-coding-agent` once by hand (PLAN-010 P6), the OpenCode / Kilo plugin against `@opencode-ai/plugin` 1.18.35 and `@kilocode/plugin` 7.8.7 the same way; both run in Node against stand-ins for their APIs (`tests/extensions/`). Each harness is smoke-tested by hand (PLAN-010 N4), and the real-agent checks (`tests/agents`) drive Claude Code, Codex, Gemini CLI, Copilot CLI, Pi, OpenCode, Kilo Code, Qwen Code and Devin CLI.
- SCENARIOS: each harness alone; sets sharing `AGENTS.md`, `.agents/skills` and `.mcp.json`; Cursor's cross-reads warn; Copilot double-loads `AGENTS.md` and `CLAUDE.md` warn; OpenCode shares Claude Code's skills

## Requirements Traceability

SOURCE: .zen/specs/REQ-HAR-harnesses.md

- HAR-1_AC-1..AC-8 → HAR-Claude (HAR_P-1 for AC-5)
- HAR-2_AC-1..AC-7 → HAR-Codex
- HAR-3_AC-1..AC-6 → HAR-Factory
- HAR-4_AC-1..AC-9 → HAR-Gemini (HAR_P-1 for AC-5)
- HAR-5_AC-1..AC-10 → HAR-Copilot (HAR_P-1 for AC-5)
- HAR-6_AC-1..AC-8 → HAR-Cursor (HAR_P-1 for AC-5)
- HAR-7_AC-1..AC-7 → HAR-Pi (HAR_P-1 for AC-4)
- HAR-10_AC-1..AC-8 → HAR-OpenCode (HAR_P-1 for AC-4)
- HAR-11_AC-1..AC-9 → HAR-Devin (HAR_P-1 for AC-5)
- HAR-12_AC-1..AC-6 → HAR-Kilo (HAR_P-1 for AC-4)
- HAR-13_AC-1..AC-8 → HAR-Qwen (HAR_P-1 for AC-4)
- HAR-8_AC-1..AC-3 → HAR-AgentsMd
- HAR-9_AC-1 → HAR-Common (HAR_P-2)
- HAR-9_AC-2 → HAR-Claude

## Change Log

- 0.1.0 (2026-10-06): Initial design (PLAN-010)
- 0.1.0 (2026-10-07): HAR-OpenCode; the extension format shared as `common::extension`
- 0.1.0 (2026-10-07): HAR-Kilo, HAR-Qwen, HAR-Devin (PLAN-015); `common::plugin` and `common::settings` for the forks
