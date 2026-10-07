# agent-harness-adapter

[![crates.io](https://img.shields.io/crates/v/agent-harness-adapter.svg)](https://crates.io/crates/agent-harness-adapter)

`agent-harness-adapter`: plug a command-line tool, written in any language, into LLM agent
harnesses: Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor,
Factory Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin, and any agent that reads `AGENTS.md`.

Your tool declares its integration once, in a manifest file. `agent-harness-adapter install`
writes it into each harness's own files, puts shared content in one place,
and never overwrites what the user changed. `agent-harness-adapter hook` turns every harness's
hook input into one JSON format for your tool, and your tool's answer back
into what that harness expects.

## Install

```sh
npm install -g @six5536/agent-harness-adapter   # prebuilt binaries
cargo install agent-harness-adapter             # from source
```

Both install the `agent-harness-adapter` command. Rust tools can use the library
[`agent-harness-adapter-core`](https://crates.io/crates/agent-harness-adapter-core)
directly instead.

## Quick start

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
run = "mytool guard"          # bridged: mytool reads the hook contract

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
agent-harness-adapter install --manifest <file> --harness <ids|all> [--scope project|user|local] [--root <dir>] [--force] [--without <parts>] [--json]
agent-harness-adapter uninstall --manifest <file> --harness <ids|all> [--scope ..] [--root <dir>] [--force] [--json]
agent-harness-adapter status  --manifest <file> [--harness <ids|all>] [--scope ..] [--root <dir>] [--json]
agent-harness-adapter hook [--tool <name>] [--tools <kind>] <harness> <event> -- <command> [args..]
agent-harness-adapter schema <manifest|hook-input|hook-answer|result>
```

- Harness ids: `claude`, `codex`, `factory`, `gemini`, `copilot`, `cursor`,
  `pi`, `opencode`, `kilo`, `qwen`, `devin`, `agents`; `all` is every harness of the manifest with files at the
  scope. `status` without `--harness` reports the installed ones.
- A part changed by hand is reported `edited` and left alone; `--force`
  overwrites it (install) or removes it (uninstall).
- `uninstall` takes the tool's content back out: its files, its block in
  the instructions file, its entries in settings files (the user's stay),
  and files and folders that are then empty. Content another installed
  harness still reads is kept (`kept … used by <harness>`). `all` is the
  installed harnesses. Run it with the manifest you installed with.
- Exit codes: 0, or 2 with `error: <message>` on stderr.

## The manifest

TOML, or JSON when the file name ends in `.json`. `agent-harness-adapter schema manifest`
prints its JSON Schema.

| Key | |
| --- | --- |
| `version` | `1` |
| `name` | the tool's name; region markers are `<!-- <name>:harness -->` |
| `harnesses` | `"all"` (default) or ids in preference order: the earliest writes a file several harnesses share |
| `adapter` | the command bridged hooks run (default `agent-harness-adapter`), e.g. an absolute path |
| `record`, `declined` | paths under the scope's root (defaults `.<name>/harness.toml`, `.<name>/harness.local.toml` at local scope, `.<name>/config.toml`) |
| `instructions` | text placed between the tool's markers in each harness's instructions file |
| `skills` | `dir` (a skill directory: its `SKILL.md` as it is and every other file in it, hidden ones aside), or `name`, `description`, `body`, `files` (path → text) |
| `hooks` | `event`, then `run` (bridged) or `command` (a template), `tools`, `timeout` (seconds), `commands` (harness → template), `owned` |
| `hook_match` | which hook entries are the tool's: `{ prefix }`, `{ contains }` or `{ any = [..] }` |
| `mcp_servers` | `name`, then `command`, `args`, `env`, or `url`, `headers` |
| `allow_commands`, `allow_mcp_tools` | what the agent may run without asking |
| `agents`, `commands` | `name`, `description`, `prompt` |
| `parts` | raw `file`, `region` or `merge` parts for one harness |
| `scopes.<scope>` | items, `record` and `declined` that replace the top level's at that scope |

Any text is a string or `{ file = "<path>" }`, relative to the manifest.

Events: `session-start`, `session-end`, `prompt-submit`, `pre-tool`,
`post-tool`, `stop`, `pre-compact`. A hook `command` template may use
`{harness}` and `{event}` anywhere; `{{` and `}}` are literal braces.

## The hook contract

A bridged hook runs `agent-harness-adapter hook --tool <name> <harness> <event> -- <run>`
(with `--tools <kind>` when the hook gives `tools`: on harnesses that cannot
match tools, such as Copilot, Cursor, Pi, OpenCode and Kilo Code, the bridge then skips your
command for other tools).
Your command gets one JSON object on stdin:

```json
{"v": 1, "harness": "claude", "event": "pre-tool", "session_id": "…", "cwd": "…",
 "tool": {"name": "Bash", "kind": "shell", "input": {"command": "ls"}}, "raw": {…}}
```

Other fields, when the harness sends them: `transcript_path`, `prompt`,
`tool_output`, `source`, `continuing` (a stop hook already asked the agent
to go on), `last_message`. `raw` is the harness's own JSON.

It writes one answer on stdout:

```json
{"answer": "allow"}
{"answer": "deny", "reason": "…"}        // pre-tool, prompt-submit
{"answer": "continue", "reason": "…"}    // stop: keep the agent going
{"answer": "context", "text": "…"}       // session-start, prompt-submit, post-tool
```

If your command cannot start, exits non-zero or writes no valid answer,
`agent-harness-adapter` allows and prints `<tool>: <reason>` on stderr: your tool failing
never blocks the agent. An answer a harness cannot express for the event
is an error (exit 1). `agent-harness-adapter schema hook-input` and `agent-harness-adapter schema hook-answer`
print the JSON Schemas.

## Versioning

The manifest's `version` and the contract's `v` change only in a release
that says so. Pre-1.0, minor versions may contain breaking changes.

## License

MIT. See [LICENSE](https://github.com/six5536/agent-harness-adapter/blob/main/LICENSE).
