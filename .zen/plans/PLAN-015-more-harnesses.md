# PLAN-015: Qwen Code, Kilo Code and Devin

| Field              | Value |
| ------------------ | ----- |
| Status             | in-progress: harnesses done (2026-10-07); real-agent checks of Qwen Code and Devin wait for credentials |
| Workflow direction | top-down (research → requirements → design → code & tests → docs) |
| Traces to          | REQ-HAR (HAR-11, HAR-12, HAR-13), DESIGN-HAR (HAR-Devin, HAR-Kilo, HAR-Qwen, HAR-Common), REQ-AHA (AHA-2_AC-3, AHA-4_AC-5), PLAN-013 §3 |

## 1. Goal

Add the three agents ranked first by hook support (research 2026-10-07): Qwen Code, Kilo Code, Devin CLI / Devin Local. Facts from each agent's source (Qwen Code 0.25.0, Kilo Code 7.8.7) or docs and binary (Devin CLI 3000.11.3).

## 2. Decisions

- D15-1: Qwen Code (`qwen`): Claude Code's hook names, input and answers in `.qwen/settings.json`; Gemini CLI's context-file choice (default `QWEN.md`, `AGENTS.md`) and `httpUrl` MCP entries, shared with Gemini as `common::settings`.
- D15-2: Kilo Code (`kilo`): OpenCode's plugin and renderings under `.kilo` / `~/.config/kilo`, typed with `@kilocode/plugin`; one `common::plugin::Layout` renders OpenCode and Kilo. Kilo reads a root `opencode.json`, so MCP and permissions are shared with OpenCode when both are installed; the plugin is not (Kilo ignores `.opencode`).
- D15-3: Devin (`devin`): Claude-family group hooks in `.devin/hooks.v1.json` (user: `hooks` of `~/.config/devin/config.json`), no pre-compact; Devin's documented answer form (`decision: block`) for deny and continue. Devin reads Claude Code's `CLAUDE.md`, skills and `.mcp.json` by default, so those are shared with Claude Code; it also runs Claude Code's hooks, so the pair warns about hooks loaded twice.
- D15-4: A stop reported idle more than once (Kilo marks a session idle from two places) is decided once by the plugin (found by the Kilo agent check: the stop hook continued three times).
- D15-5: Agent checks: Kilo through `kilo serve` (its free model needs no account); Qwen Code with a provider key from the run's environment; Devin installed from its pinned, checksum-verified release; check A7 now requires exactly one continuation.

## 3. Results

- Unit and install tests for each harness; sets {opencode, kilo}, {claude, devin}, {codex, qwen}; the Kilo plugin type-checks against `@kilocode/plugin` 7.8.7.
- Kilo Code 7.8.7 (`kilo/kilo-auto/free`): passes A1–A10, A7b included. OpenCode re-run on the shared plugin: passes A1–A10.
- Devin CLI 3000.11.3 and Qwen Code 0.25.0 installed and inspected without an account: Devin lists the `AGENTS.md` rule, the skill, the MCP server and the subagent the adapter wrote; Qwen Code lists the MCP server as pending approval (its install note says so). Hooks need a live session.

## 4. Open

- Real-agent checks of Qwen Code (a provider key) and Devin (a Devin account).
