# agent-harness-kit

[![crates.io](https://img.shields.io/crates/v/agent-harness-kit.svg)](https://crates.io/crates/agent-harness-kit)

`ahk`: plug a command-line tool, written in any language, into LLM agent
harnesses: Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor,
Factory Droid, Pi, and any agent that reads `AGENTS.md`.

Your tool declares its integration once, in a manifest file. `ahk install`
writes it into each harness's own files, puts shared content in one place,
and never overwrites what the user changed. `ahk hook` turns every harness's
hook input into one JSON format for your tool, and your tool's answer back
into what that harness expects.

## Install

```sh
npm install -g @six5536/agent-harness-kit   # prebuilt binaries
cargo install agent-harness-kit             # from source
```

Both install the `ahk` command. Rust tools can use the library
[`agent-harness-kit-core`](https://crates.io/crates/agent-harness-kit-core)
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
ahk install --manifest mytool.harness.toml --harness claude,codex
ahk status  --manifest mytool.harness.toml
```

## Commands

```text
ahk install --manifest <file> --harness <ids|all> [--scope project|user|local] [--root <dir>] [--force] [--without <parts>] [--json]
ahk status  --manifest <file> [--harness <ids|all>] [--scope ..] [--root <dir>] [--json]
ahk hook [--tool <name>] <harness> <event> -- <command> [args..]
ahk schema <manifest|hook-input|hook-answer|result>
```

- Harness ids: `claude`, `codex`, `factory`, `gemini`, `copilot`, `cursor`,
  `pi`, `agents`; `all` is every harness of the manifest with files at the
  scope. `status` without `--harness` reports the installed ones.
- A part changed by hand is reported `edited` and left alone; `--force`
  overwrites it.
- Exit codes: 0, or 2 with `error: <message>` on stderr.

## The manifest

TOML, or JSON when the file name ends in `.json`. `ahk schema manifest`
prints its JSON Schema.

| Key | |
| --- | --- |
| `version` | `1` |
| `name` | the tool's name; region markers are `<!-- <name>:harness -->` |
| `harnesses` | `"all"` (default) or ids in preference order: the earliest writes a file several harnesses share |
| `ahk` | the command bridged hooks run (default `ahk`), e.g. an absolute path |
| `record`, `declined` | paths under the scope's root (defaults `.<name>/harness.toml`, `.<name>/harness.local.toml` at local scope, `.<name>/config.toml`) |
| `instructions` | text placed between the tool's markers in each harness's instructions file |
| `skills` | `name`, `description`, `body`, `files` (path → text) |
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

A bridged hook runs `ahk hook --tool <name> <harness> <event> -- <run>`.
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
`ahk` allows and prints `<tool>: <reason>` on stderr: your tool failing
never blocks the agent. An answer a harness cannot express for the event
is an error (exit 1). `ahk schema hook-input` and `ahk schema hook-answer`
print the JSON Schemas.

## Versioning

The manifest's `version` and the contract's `v` change only in a release
that says so. Pre-1.0, minor versions may contain breaking changes.

## License

MIT. See [LICENSE](https://github.com/six5536/agent-harness-kit/blob/main/LICENSE).
