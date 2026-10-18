# agent-harness-adapter

[![crates.io](https://img.shields.io/crates/v/agent-harness-adapter.svg)](https://crates.io/crates/agent-harness-adapter)

One agent setup, every coding agent. Keep your instructions, skills and
hooks once, in the standard places (`AGENTS.md` and `.agents/`), and
`agent-harness-adapter` makes Claude Code, OpenAI Codex, Gemini CLI, GitHub
Copilot, Cursor, Factory Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin
and any agent that reads `AGENTS.md` use them. Hooks are written once in
Claude Code's format and run in every agent.

Tools in any language can also ship their own integration from a manifest
file: the command writes it into each harness's own files, puts shared
content in one place, and never overwrites what the user changed.

## Install

```sh
npm install -g @six5536/agent-harness-adapter   # prebuilt binaries
cargo install agent-harness-adapter             # from source
```

Both install the `agent-harness-adapter` command. Rust tools can use the library
[`agent-harness-adapter-core`](https://crates.io/crates/agent-harness-adapter-core)
directly instead.

## Quick start: your own setup

```sh
cd my-repo
agent-harness-adapter install          # asks which agents you use; writes .agents/agent-harness-adapter.toml
```

Already set up for Claude Code? The first `install` offers to move
`CLAUDE.md`'s text, `.claude/skills`, Claude Code's hooks and
`.claude/agents` into the standard places (`--migrate` says yes without
asking), then points or copies them back for Claude Code. It refuses files
with uncommitted changes (`--force` overrides), and at user scope backs up
first.

Then keep everything in the standard places:

| What | Where | Each agent gets |
| --- | --- | --- |
| Instructions | `AGENTS.md` | read where it is; Claude Code gets `@AGENTS.md` in `CLAUDE.md`, Gemini CLI a `context.fileName` entry |
| Skills | `.agents/skills/<name>/SKILL.md` | read where they are; copied into `.claude/skills` for Claude Code |
| Hooks | `.agents/hooks.json` (Claude Code's format) | one entry per event that runs your hooks at hook time, in every agent |
| MCP servers | `.mcp.json` | read where it is, or copied into each agent's config |
| Subagents | `.agents/agents/<name>.md` (Claude Code's format) | copied into each agent's folder |

`agent-harness-adapter install` again (from anywhere in the repo) refreshes
the installed agents; `--harness <id>` adds one. `agent-harness-adapter status`
exits 1 when a copy is out of date, for CI.

`.agents/hooks.json`:

```json
{"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "scripts/guard.sh"}]}]}}
```

`scripts/guard.sh` gets Claude Code's hook input in every agent, and answers
the way Claude Code reads it, for example `exit 2` with a reason on stderr to
block.

## Quick start: a tool's integration

`mytool.harness.toml`:

```toml
version = 1
name = "mytool"
instructions = { file = "agent/instructions.md" }
allow_commands = ["mytool"]

[[skills]]
name = "mytool"
description = "Use mytool to check the project."
body = { file = "agent/skill.md" }

[[skills]]
dir = "agent/skills/review"   # an existing skill directory, copied as it is

[[hooks]]
event = "pre-tool"
tools = "shell"
run = "mytool guard"          # bridged: mytool speaks Claude Code's hook protocol

[[mcp_servers]]
name = "mytool"
command = "mytool"
args = ["mcp"]
```

```sh
agent-harness-adapter install --manifest mytool.harness.toml --harness claude,codex
agent-harness-adapter status  --manifest mytool.harness.toml
```

## Commands

```text
agent-harness-adapter install    # the config, or --manifest <file>; --harness <ids|all>, --scope, --force, --migrate
agent-harness-adapter status     # exit 1 when a copy is out of date
agent-harness-adapter uninstall  # takes out what it wrote; your sources stay
agent-harness-adapter hook ...   # what installed hooks run
agent-harness-adapter schema <manifest|hooks|hook-input|hook-answer|result>
```

## Documentation

<https://six5536.github.io/agent-harness-adapter/>:

- [Getting started](https://six5536.github.io/agent-harness-adapter/getting-started/) and [coming from Claude Code](https://six5536.github.io/agent-harness-adapter/claude-code/)
- [The command](https://six5536.github.io/agent-harness-adapter/use/command/): every option and exit code
- [The config and the manifest](https://six5536.github.io/agent-harness-adapter/reference/config/), key by key
- [The hook contract](https://six5536.github.io/agent-harness-adapter/reference/hook-contract/) and [what each agent can do](https://six5536.github.io/agent-harness-adapter/write-once/hooks/#what-each-agent-can-do)
- [Harnesses](https://six5536.github.io/agent-harness-adapter/reference/harnesses/): where each agent's files go

Pre-1.0, minor versions may contain breaking changes.

## License

MIT. See [LICENSE](https://github.com/six5536/agent-harness-adapter/blob/main/LICENSE).
