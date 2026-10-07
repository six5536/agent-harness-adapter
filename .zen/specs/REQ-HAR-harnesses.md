# Requirements Specification

## Introduction

What each built-in harness (KIT-18_AC-2) installs, reads and answers. Paths are relative to the scope's root (KIT-21): the project root, or the home directory at user scope. Facts were checked against each agent's documentation on 2026-10-06 (PLAN-010 §10), OpenCode's on 2026-10-07. An AC that rests on a fact not yet confirmed says so and is checked before it is coded (PLAN-010 D10-11).

## Glossary

- TOOL: the CLI that uses the library; `<tool>` is its name
- CLAUDE FAMILY: harnesses whose hooks use Claude Code's input fields, exit codes and JSON answers
- GROUP HOOK: a group `{"hooks":[{"type":"command","command":…}]}` in an event's array, owned by entry (KIT-10_AC-3)
- PATHS: user-scope paths are relative to the home directory, so `.agents/skills` there is `~/.agents/skills`
- SKILL DIR: `<name>/SKILL.md` with frontmatter `name` and `description`, plus the skill's extra files
- MCP JSON: the `mcpServers` object, a stdio server as `{command, args, env}`, an http one as `{url, headers}`
- NO MATCHER: a hook limited to a tool kind is installed for every tool, and the hook filters on the parsed kind (KIT-11_AC-6)
- MARKDOWN AGENT: `<name>.md` with frontmatter `name` and `description`, the prompt as the body

## Stakeholders

- TOOL AUTHOR: installs into these harnesses through one integration
- AGENT USER: uses one or more of these agents in the same project

## Requirements

### HAR-1: Claude Code (`claude`) [MUST]

AS A tool author, I WANT Claude Code's files and hooks, SO THAT my integration works in Claude Code.

ACCEPTANCE CRITERIA

- [ ] HAR-1_AC-1 [ubiquitous]: Scopes SHALL be project, user and local
- [ ] HAR-1_AC-2 [ubiquitous]: Instructions SHALL be a region in the file chosen at project scope by its rule (no `AGENTS.md` → `CLAUDE.md`; only `AGENTS.md` → it; both, with a line `@AGENTS.md` in `CLAUDE.md` → `AGENTS.md`; both without it → `CLAUDE.md`), and in `.claude/CLAUDE.md` at user scope; Claude Code SHALL be taken to load the chosen file only
- [ ] HAR-1_AC-3 [ubiquitous]: Hooks SHALL be group hooks under `hooks.<Event>` of `.claude/settings.json` (`.claude/settings.local.json` at local scope), events `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`, `PreCompact`, with the tool kind as the group's `matcher` (`Bash`; `Read|Grep|Glob|LS`; `Edit|Write|MultiEdit|NotebookEdit`; `mcp__.*`)
- [ ] HAR-1_AC-4 [ubiquitous]: Hook input SHALL be read from `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `prompt`, `tool_name`, `tool_input`, `tool_response`, `source`, `stop_hook_active`, `last_assistant_message`
- [ ] HAR-1_AC-5 [ubiquitous]: Answers SHALL be: allow `{}`; deny before a tool `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":…}}`; deny a prompt and continue at stop `{"decision":"block","reason":…}`; context `{"hookSpecificOutput":{"hookEventName":…,"additionalContext":…}}`; exit 0
- [ ] HAR-1_AC-6 [ubiquitous]: Skills SHALL be skill dirs under `.claude/skills`; agents markdown agents under `.claude/agents`; commands `<name>.md` under `.claude/commands` with frontmatter `description`
- [ ] HAR-1_AC-7 [ubiquitous]: MCP servers SHALL be MCP JSON in `.mcp.json` at project scope; unsupported at user scope (`~/.claude.json` is the agent's own state file)
- [ ] HAR-1_AC-8 [ubiquitous]: An allowed command SHALL be `Bash(<prefix> *)`, an allowed MCP tool `mcp__<server>__<tool>`, in `permissions.allow` of the settings file of HAR-1_AC-3

### HAR-2: OpenAI Codex CLI (`codex`) [MUST]

AS A tool author, I WANT Codex's files and hooks, SO THAT my integration works in Codex.

ACCEPTANCE CRITERIA

- [ ] HAR-2_AC-1 [ubiquitous]: Scopes SHALL be project and user
- [ ] HAR-2_AC-2 [ubiquitous]: Instructions SHALL be a region in `AGENTS.md` (`.codex/AGENTS.md` at user scope)
- [ ] HAR-2_AC-3 [ubiquitous]: Hooks SHALL be group hooks under `hooks.<Event>` of `.codex/hooks.json`, the Claude family's event names and protocol (HAR-1_AC-4, HAR-1_AC-5); matcher `^Bash$` for shell, none for other kinds (tool names not confirmed)
- [ ] HAR-2_AC-4 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`; agents `<name>.toml` under `.codex/agents` with `name`, `description`, `developer_instructions`; commands unsupported (deprecated in Codex)
- [ ] HAR-2_AC-5 [ubiquitous]: MCP servers SHALL be TOML tables `[mcp_servers.<name>]` in `.codex/config.toml`: `command`, `args`, `env`, or `url`, `http_headers`
- [ ] HAR-2_AC-6 [ubiquitous]: Allowed commands SHALL be unsupported — rule-file syntax not confirmed
- [ ] HAR-2_AC-7 [ubiquitous]: After an install that wrote a part, notes SHALL say that the project must be trusted (project scope) and that new hooks run only once approved in `/hooks` (when hooks were written); after any part was written or removed, that Codex must be restarted

### HAR-3: Factory Droid (`factory`) [MUST]

AS A tool author, I WANT Droid's files and hooks, SO THAT my integration works in Droid.

ACCEPTANCE CRITERIA

- [ ] HAR-3_AC-1 [ubiquitous]: Scopes SHALL be project and user; local once Droid's reading of hooks from `settings.local.json` is confirmed
- [ ] HAR-3_AC-2 [ubiquitous]: Instructions SHALL be a region in `AGENTS.md` (`.factory/AGENTS.md` at user scope); Droid MAY also load `CLAUDE.md` — not confirmed whether beside `AGENTS.md`
- [ ] HAR-3_AC-3 [ubiquitous]: Hooks SHALL be group hooks under `<Event>` at the top of `.factory/hooks.json`; WHERE that file is absent and `.factory/settings.json` has a `hooks` key, under `hooks.<Event>` of `.factory/settings.json`; Claude family event names and protocol; matchers `Execute`; `Read|LS|Glob|Grep`; `Edit|Create|ApplyPatch`; `mcp__.*`
- [ ] HAR-3_AC-4 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills` (`.factory/skills` at user scope); agents markdown agents under `.factory/droids` with `model: inherit`; commands `<name>.md` under `.factory/commands` with frontmatter `description`
- [ ] HAR-3_AC-5 [ubiquitous]: MCP servers SHALL be MCP JSON in `.factory/mcp.json`, each with `type` (`stdio` or `http`)
- [ ] HAR-3_AC-6 [ubiquitous]: Allowed commands SHALL be unsupported — `permissionRules` shape not confirmed

### HAR-4: Gemini CLI (`gemini`) [MUST]

AS A tool author, I WANT Gemini CLI's files and hooks, SO THAT my integration works in Gemini CLI.

ACCEPTANCE CRITERIA

- [ ] HAR-4_AC-1 [ubiquitous]: Scopes SHALL be project and user
- [ ] HAR-4_AC-2 [ubiquitous]: The context files SHALL be those `context.fileName` (a string or an array) names in the scope's `.gemini/settings.json`, else (at project scope) in the user's, else `GEMINI.md`, under `.gemini/` at user scope; Gemini CLI SHALL be taken to load them all; instructions SHALL be a region in `AGENTS.md` when listed, else in the first
- [ ] HAR-4_AC-3 [ubiquitous]: Hooks SHALL be group hooks under `hooks.<Event>` of `.gemini/settings.json`, events `SessionStart`, `SessionEnd`, `BeforeAgent` (prompt submit), `BeforeTool`, `AfterTool`, `AfterAgent` (stop), `PreCompress`; timeouts in milliseconds; matchers `run_shell_command`; `read_file|read_many_files|glob|search_file_content|list_directory`; `write_file|replace`; `mcp_.*`
- [ ] HAR-4_AC-4 [ubiquitous]: Hook input SHALL be read from `session_id`, `transcript_path`, `cwd`, `hook_event_name`, `prompt`, `prompt_response` (last message), `tool_name`, `tool_input`, `tool_response`, `source`, `stop_hook_active`
- [ ] HAR-4_AC-5 [ubiquitous]: Answers SHALL be: allow `{}`; deny and continue `{"decision":"deny","reason":…}`; context `{"hookSpecificOutput":{"additionalContext":…}}`; exit 0
- [ ] HAR-4_AC-6 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`; agents markdown agents under `.gemini/agents`; commands `<name>.toml` under `.gemini/commands` with `description` and `prompt`, `$ARGUMENTS` written as `{{args}}`
- [ ] HAR-4_AC-7 [ubiquitous]: MCP servers SHALL be MCP JSON under `mcpServers` of `.gemini/settings.json`, an http server's URL as `httpUrl`
- [ ] HAR-4_AC-8 [ubiquitous]: An allowed command SHALL be `run_shell_command(<prefix>)` in `tools.allowed` of `.gemini/settings.json`; allowed MCP tools SHALL be unsupported (form not confirmed)
- [ ] HAR-4_AC-9 [ubiquitous]: Notes SHALL say, when a part was written at project scope, that a trusted folder is needed when folder trust is on

### HAR-5: GitHub Copilot (`copilot`) [MUST]

AS A tool author, I WANT Copilot's files and hooks (CLI, VS Code agent, cloud agent), SO THAT my integration works in Copilot.

ACCEPTANCE CRITERIA

- [ ] HAR-5_AC-1 [ubiquitous]: Scopes SHALL be project, user and local
- [ ] HAR-5_AC-2 [ubiquitous]: Instructions SHALL be a region in `AGENTS.md` (`.copilot/copilot-instructions.md` at user scope); Copilot SHALL be taken to load `AGENTS.md`, `CLAUDE.md` and `.github/copilot-instructions.md`
- [ ] HAR-5_AC-3 [ubiquitous]: Hooks SHALL be the whole file `.github/hooks/<tool>.json` (`.copilot/hooks/<tool>.json` at user scope; owned entries under `hooks.<event>` of `.github/copilot/settings.local.json` at local scope): `{"version":1,"hooks":{<event>:[{"type":"command","bash":…,"powershell":…,"timeoutSec":…}]}}`, events `sessionStart`, `sessionEnd`, `userPromptSubmitted`, `preToolUse`, `postToolUse`, `agentStop`, `preCompact`; no matcher
- [ ] HAR-5_AC-4 [ubiquitous]: Hook input SHALL be read from `sessionId`, `cwd`, `prompt`, `toolName`, `toolArgs` (an object, or a string holding JSON), `toolResult`; the event SHALL come from the installed command
- [ ] HAR-5_AC-5 [ubiquitous]: Answers SHALL be: allow `{}`; deny before a tool `{"permissionDecision":"deny","permissionDecisionReason":…}`; continue at stop `{"decision":"block","reason":…}`; exit 0; deny a prompt and context SHALL be unsupported until confirmed
- [ ] HAR-5_AC-6 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`, Copilot being taken to load `.github/skills` and `.claude/skills` too; agents `<name>.agent.md` under `.github/agents` (`.copilot/agents`); commands unsupported
- [ ] HAR-5_AC-7 [ubiquitous]: MCP servers SHALL be MCP JSON in `.mcp.json` (`.copilot/mcp-config.json` at user scope)
- [ ] HAR-5_AC-8 [ubiquitous]: Allowed commands SHALL be unsupported (no file holds them)
- [ ] HAR-5_AC-9 [ubiquitous]: Copilot in VS Code MAY load Claude Code's hooks (`chat.useClaudeHooks`) and agents (`.claude/agents`)
- [ ] HAR-5_AC-10 [ubiquitous]: After an install that wrote the hooks at project or local scope, notes SHALL say that Copilot runs the repository's hooks only in a trusted folder (confirmed with Copilot CLI 1.0.92, PLAN-013)

### HAR-6: Cursor (`cursor`) [MUST]

AS A tool author, I WANT Cursor's files and hooks (editor and CLI), SO THAT my integration works in Cursor.

ACCEPTANCE CRITERIA

- [ ] HAR-6_AC-1 [ubiquitous]: Scopes SHALL be project and user
- [ ] HAR-6_AC-2 [ubiquitous]: Instructions SHALL be a region in `AGENTS.md` at project scope and unsupported at user scope (user rules are not files); the Cursor CLI MAY also load `CLAUDE.md`
- [ ] HAR-6_AC-3 [ubiquitous]: Hooks SHALL be owned entries `{"command":…,"timeout":…}` under `hooks.<event>` of `.cursor/hooks.json` with `"version": 1`, events `sessionStart`, `sessionEnd`, `beforeSubmitPrompt`, `preToolUse`, `postToolUse`, `stop`, `preCompact`; the Cursor CLI MAY also run Claude Code's hooks
- [ ] HAR-6_AC-4 [ubiquitous]: Hook input SHALL be read from `conversation_id` (session), `workspace_roots` (first, as the working directory), `transcript_path`, `hook_event_name`, `prompt`, `tool_name`, `tool_input`, `loop_count` (continuing when above 0); a payload with `cursor_version` SHALL be read this way whatever harness the command names
- [ ] HAR-6_AC-5 [ubiquitous]: Answers SHALL be: allow `{}`; deny before a tool `{"permission":"deny","user_message":…,"agent_message":…}`; deny a prompt `{"continue":false,"user_message":…}`; continue at stop `{"followup_message":…}`; context at session start `{"additional_context":…}`; exit 0
- [ ] HAR-6_AC-6 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`, Cursor being taken to load `.cursor/skills` and `.claude/skills` too; agents markdown agents under `.cursor/agents` with `model: inherit`, Cursor being taken to load `.claude/agents` too; commands `<name>.md` under `.cursor/commands`, the prompt as the body
- [ ] HAR-6_AC-7 [ubiquitous]: MCP servers SHALL be MCP JSON in `.cursor/mcp.json`
- [ ] HAR-6_AC-8 [ubiquitous]: An allowed command SHALL be `Shell(<first word of prefix>)`, an allowed MCP tool `Mcp(<server>:<tool>)`, in `permissions.allow` of `.cursor/cli.json` (`.cursor/cli-config.json` at user scope)

### HAR-7: Pi (`pi`) [MUST]

AS A tool author, I WANT Pi's files and hooks, SO THAT my integration works in Pi.

ACCEPTANCE CRITERIA

- [ ] HAR-7_AC-1 [ubiquitous]: Scopes SHALL be project and user; user-scope paths SHALL be under `.pi/agent`
- [ ] HAR-7_AC-2 [ubiquitous]: Instructions SHALL be a region in the first of `AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md` that exists in the directory (project root, or `.pi/agent`), else `AGENTS.md`; Pi SHALL be taken to load that file only
- [ ] HAR-7_AC-3 [ubiquitous]: Hooks SHALL be the whole file `.pi/extensions/<tool>.ts`, generated by the library, that on each installed event runs the installed commands with the event's JSON on stdin and maps the first deciding answer: deny before a tool (`tool_call`) → `{ block: true, reason }`; continue at stop (`agent_before_settle`, at every settle) → `{ continue: true }` with the reason as a `custom_message` entry, which reaches the model; context at session start (`session_start`) → kept and added to the first prompt's hidden custom message; context on prompt submit (`before_agent_start`) → a hidden custom message (after any kept session-start context); context after a tool (`tool_result`) → a text block added to the result; events `session_start`, `session_shutdown`, `before_agent_start`, `tool_call`, `tool_result`, `session_before_compact`, `agent_before_settle`; a command that fails, times out or gives no answer of the kind SHALL allow
- [ ] HAR-7_AC-4 [ubiquitous]: Hook input SHALL be the extension's JSON: `event`, `session_id`, `transcript_path` (from Pi's session manager), `cwd`, `prompt`, `source`, `tool_name`, `tool_input`, `tool_output`, `continuing` (the extension asked to continue at the last stop of the current prompt; a new prompt resets it); answers SHALL be the library's JSON `{"answer":"allow"|"deny"|"continue"|"context","reason"?,"text"?}`, exit 0
- [ ] HAR-7_AC-5 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`; commands prompt templates `<name>.md` under `.pi/prompts` with frontmatter `description`; agents unsupported
- [ ] HAR-7_AC-6 [ubiquitous]: MCP servers SHALL be MCP JSON in `.pi/mcp.json`; allowed commands unsupported
- [ ] HAR-7_AC-7 [ubiquitous]: Notes SHALL say that the project must be trusted (when a part was written at project scope) and that Pi must `/reload` (when a part was written or removed)

### HAR-10: OpenCode (`opencode`) [MUST]

AS A tool author, I WANT OpenCode's files and hooks, SO THAT my integration works in OpenCode.

ACCEPTANCE CRITERIA

- [ ] HAR-10_AC-1 [ubiquitous]: Scopes SHALL be project and user; OpenCode's own user-scope paths SHALL be under `.config/opencode`
- [ ] HAR-10_AC-2 [ubiquitous]: Instructions SHALL be a region in the first of `AGENTS.md`, `CLAUDE.md` that exists at project scope (`.config/opencode/AGENTS.md`, `.claude/CLAUDE.md` at user scope), else the first; OpenCode SHALL be taken to load that file only
- [ ] HAR-10_AC-3 [ubiquitous]: Hooks SHALL be the whole file `.opencode/plugins/<tool>.ts`, a plugin generated by the library that runs the installed commands with the event's JSON on stdin and maps the first deciding answer: session start and context at session start → run, and the context kept, at a session's first `chat.message`; context on prompt submit (`chat.message`) → a synthetic text part added to the message, after any kept session-start context; deny before a tool (`tool.execute.before`) → an error thrown, whose message the model sees; context after a tool (`tool.execute.after`) → appended to the tool's output; stop (`session.idle` event) → continue sends the reason as a new prompt (`promptAsync`), which the plugin does not treat as a user prompt; pre-compact → `experimental.session.compacting`; no session end; a subagent's session (one with a parent) SHALL have no session-start, prompt or stop hooks; a command that fails, times out or gives no answer of the kind SHALL allow
- [ ] HAR-10_AC-4 [ubiquitous]: Hook input and answers SHALL be those of HAR-7_AC-4 (the library's own format), without `transcript_path`
- [ ] HAR-10_AC-5 [ubiquitous]: MCP servers SHALL be members of `mcp` in `opencode.json` (`.config/opencode/opencode.json` at user scope): a stdio server `{"type":"local","command":[command, …args],"environment"?}`, an http one `{"type":"remote","url","headers"?}`
- [ ] HAR-10_AC-6 [ubiquitous]: An allowed command SHALL be `"<prefix> *": "allow"` under `permission.bash` of that file; allowed MCP tools SHALL be unsupported (matching not confirmed)
- [ ] HAR-10_AC-7 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`, OpenCode being taken to load `.claude/skills` and `.opencode/skills` (`.config/opencode/skills`) too; agents markdown agents under `.opencode/agents` with `mode: subagent`; commands `<name>.md` under `.opencode/commands` with frontmatter `description`
- [ ] HAR-10_AC-8 [ubiquitous]: After any part was written or removed, notes SHALL say that OpenCode must be restarted; no trust is needed (OpenCode loads a project's files without asking)

### HAR-8: Generic `AGENTS.md` agent (`agents`) [MUST]

AS A tool author, I WANT one harness for any agent that reads `AGENTS.md`, SO THAT agents without a module still get my instructions and skills.

ACCEPTANCE CRITERIA

- [ ] HAR-8_AC-1 [ubiquitous]: Scopes SHALL be project and user
- [ ] HAR-8_AC-2 [ubiquitous]: Instructions SHALL be a region in `AGENTS.md` at project scope; unsupported at user scope
- [ ] HAR-8_AC-3 [ubiquitous]: Skills SHALL be skill dirs under `.agents/skills`; every other item SHALL be unsupported

### HAR-9: Shared renderings [MUST]

AS AN agent user, I WANT one location to serve every harness that reads it, SO THAT deduplication (KIT-19) is safe.

ACCEPTANCE CRITERIA

- [ ] HAR-9_AC-1 [ubiquitous]: Every harness SHALL render a given item at a given location to the same part: the instructions region, skill dirs, MCP JSON in `.mcp.json`, and markdown agents under `.claude/agents`
- [ ] HAR-9_AC-2 [ubiquitous]: The hook command at a location another harness also runs SHALL name the harness that writes it; the hook input parser SHALL recognise the running harness from the payload where it differs (HAR-6_AC-4)

## Assumptions

- The facts in PLAN-010 §10 hold; each harness module names the documentation pages it follows

## Constraints

- No harness module depends on another's internals; shared renderings (HAR-9) come from shared functions

## Out of Scope

- Agents' system or managed (admin) scopes
- Plugins, extensions and marketplaces as a distribution format
- YAML-configured agents (Aider, Goose)

## Change Log

- 0.1.0 (2026-10-06): Initial requirements (PLAN-010)
- 0.1.0 (2026-10-07): HAR-10 OpenCode (checked against OpenCode 1.18.35's docs and source)
