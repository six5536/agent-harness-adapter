# agent-harness-adapter-core

[![crates.io](https://img.shields.io/crates/v/agent-harness-adapter-core.svg)](https://crates.io/crates/agent-harness-adapter-core)
[![docs.rs](https://img.shields.io/docsrs/agent-harness-adapter-core)](https://docs.rs/agent-harness-adapter-core)

Shared plumbing for command-line tools that plug into LLM agent harnesses:
Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor, Factory
Droid, Pi, and any agent that reads `AGENTS.md`. Your tool declares its
integration once (an instructions block, skills, hooks, MCP servers,
allowed commands, subagents, slash commands); the library writes it into each
harness's own files, puts shared content in one place, reports what it
found, and never overwrites what the user changed. It also turns each
harness's hook input and answers into one shape, and gives you a findings
report, CLI exit and output conventions, and atomic file writes. The library
embeds no content of its own.

Tools in other languages use the same library through the `agent-harness-adapter` command
([`agent-harness-adapter`](https://crates.io/crates/agent-harness-adapter)) and a
manifest file.

## Features

- One `Integration` for every harness; each `Harness` renders it into its
  own files, at `project`, `user` or `local` scope
- Shared content written once: `AGENTS.md`, `.agents/skills`, `.mcp.json`
  serve every harness that reads them; a warning when an agent could load
  something twice
- States per part: `skipped`, `shared`, `absent`, `current`, `stale`,
  `edited`; an edited part is only replaced with `--force`
- A refused install writes nothing; every write is atomic and only on change
- Instructions in a marked region, settings merged into JSON or TOML with
  the file's own style kept, whole files for skills, agents and commands
- One hook model: events, input and answers the same for every harness;
  hook commands in any shape (`{harness}` and `{event}` placeholders
  optional), one per harness when needed (`Hook::command_for`)
- Allowed commands and MCP tools (`allow_command`, `allow_mcp_tool`) in each
  harness's permission list where it has one
- Declined parts (`--without`) remembered in the tool's TOML config
- `LoopGuard` so a stop hook blocks only once on the same text
- Findings reports, CLI exit codes, broken-pipe handling, atomic writes

## Harnesses

| Harness | Id | Instructions | Hooks | Skills | MCP | Allowed commands | Agents | Commands |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Claude Code | `claude` | `CLAUDE.md` or `AGENTS.md` | `.claude/settings.json` | `.claude/skills` | `.mcp.json` | yes | yes | yes |
| OpenAI Codex | `codex` | `AGENTS.md` | `.codex/hooks.json` | `.agents/skills` | `.codex/config.toml` | – | yes | – |
| Factory Droid | `factory` | `AGENTS.md` | `.factory/hooks.json` | `.agents/skills` | `.factory/mcp.json` | – | yes | yes |
| Gemini CLI | `gemini` | `GEMINI.md`, or `AGENTS.md` when configured | `.gemini/settings.json` | `.agents/skills` | `.gemini/settings.json` | yes | yes | yes |
| GitHub Copilot | `copilot` | `AGENTS.md` | `.github/hooks/<tool>.json` | `.agents/skills` | `.mcp.json` | – | yes | – |
| Cursor | `cursor` | `AGENTS.md` | `.cursor/hooks.json` | `.agents/skills` | `.cursor/mcp.json` | yes | yes | yes |
| Pi | `pi` | `AGENTS.md` (first of `AGENTS.override.md`, `AGENTS.md`, `CLAUDE.md`) | a generated extension, `.pi/extensions/<tool>.ts` | `.agents/skills` | `.pi/mcp.json` | – | – | yes |
| Any `AGENTS.md` agent | `agents` | `AGENTS.md` | – | `.agents/skills` | – | – | – | – |

Paths are for project scope; user scope uses each harness's directory under
the home directory, and `local` scope (Claude Code, Copilot) its git-ignored
settings file. A dash means the harness has no file for the item, or its
format is not yet confirmed; the item is then listed as `unsupported`. When
several harnesses read one location (`AGENTS.md`, `.agents/skills`,
`.mcp.json`, Claude Code's skills and agents that Cursor and Copilot also
read), the content is written there once and the others report it as
`shared`.

## Installation

### Prerequisites

- Rust 1.85 or newer

### Install

```bash
cargo add agent-harness-adapter-core
```

## Usage

Implement `Tool` for your CLI and call `install` / `status`:

```rust
use std::{path::PathBuf, sync::Arc};

use agent_harness_adapter_core::{
    DeclinedStore, Harness, InstallOptions, Integration, Result, Scope, State, TomlDeclined, Tool,
    harness,
    hook::Event,
    install,
    integration::{Hook, McpServer, Skill},
    status,
};

struct MyTool {
    project: PathBuf,
}

impl Tool for MyTool {
    fn name(&self) -> &str {
        "mytool"
    }

    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        harness::builtin()
    }

    fn integration(&self, _scope: Scope) -> Integration {
        Integration::new()
            .instructions("Run `mytool check` before you stop.\n")
            .skill(Skill::new("mytool", "Checks the project with mytool.", "# mytool\n"))
            .hook(Hook::new(Event::Stop, "mytool hook {harness} {event}"))
            .mcp_server(McpServer::stdio("mytool", "mytool", ["mcp"]))
            .allow_command("mytool")
    }

    fn root(&self, _scope: Scope) -> Result<PathBuf> {
        Ok(self.project.clone())
    }

    fn record_path(&self, _scope: Scope) -> Result<PathBuf> {
        Ok(self.project.join(".mytool/harness.toml"))
    }

    fn declined_store(&self, _scope: Scope) -> Result<Box<dyn DeclinedStore + '_>> {
        Ok(Box::new(TomlDeclined::new(
            self.project.join(".mytool/config.toml"),
            ".mytool/config.toml",
        )))
    }
}

# fn main() -> Result<()> {
# let project = std::env::temp_dir().join(format!("agent-harness-adapter-readme-{}", std::process::id()));
# let _ = std::fs::remove_dir_all(&project);
let tool = MyTool { project };
let result = install(&tool, &InstallOptions::new(["claude", "codex", "pi"], Scope::Project))?;
print!("{}", result.to_text());
// claude:
//   created CLAUDE.md (instructions)
//   created .claude/skills (skills)
//   created .claude/settings.json (hooks)
//   ...
// codex:
//   created AGENTS.md (instructions)
//   created .agents/skills (skills)
//   created .codex/hooks.json (hooks)
//   created .codex/config.toml (mcp)
//   unsupported: permissions
//   note: new hooks run only once approved in Codex's /hooks
//   ...
// pi:
//   shared  AGENTS.md (instructions, by codex)
//   shared  .agents/skills (skills, by codex)
//   created .pi/extensions (hooks)
//   ...
let again = status(&tool, ["claude", "codex", "pi"], Scope::Project)?;
for h in &again.harnesses {
    assert!(h.parts.iter().all(|p| matches!(p.state, State::Current | State::Shared)));
}
# std::fs::remove_dir_all(&tool.project).unwrap();
# Ok(())
# }
```

The hook command answers through the harness that ran it:

```rust,no_run
use std::io::{self, Read};

use agent_harness_adapter_core::{
    harness,
    hook::{Answer, Event, HookInput, emit},
};

// mytool hook <harness> <event>
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(h) = harness::find(&args[2]) else { std::process::exit(2) };
    let Ok(event) = args[3].parse::<Event>() else { std::process::exit(2) };
    let mut stdin = String::new();
    let _ = io::stdin().read_to_string(&mut stdin);
    let input = HookInput::parse(h.as_ref(), event, &stdin).unwrap_or_default();
    let answer: Result<Answer, String> = Ok(if input.continuing {
        Answer::Allow { stderr: None }
    } else {
        Answer::Continue { reason: "Run `mytool check` first.".into() }
    });
    let code = emit(h.as_ref(), event, answer, &mut io::stdout(), &mut io::stderr()).unwrap_or(1);
    std::process::exit(code.into());
}
```

### Modules

- `integration`: `Integration` and its items (`Skill`, `Hook`, `McpServer`, `Agent`, `Command`)
- `harness`: the `Harness` contract, the parts, their states, `install` / `uninstall` / `status` / `installed`, the same for every harness
- `hook`: events, `HookInput`, `Answer`, `emit`, `LoopGuard`; `wire`, the hook contract's JSON
- `manifest`: an integration declared in a TOML or JSON file (`Manifest`, `ManifestTool`), as the `agent-harness-adapter` command reads it
- `claude`, `codex`, `factory`, `gemini`, `copilot`, `cursor`, `pi`, `agents_md`: one harness each
- `report`: `Finding`, `Report`, text and JSON forms
- `cli`: exit codes (0 ok, 1 errors found, 2 usage or internal error), stdout, broken pipes, `finish`
- `fs`: `read_text` and `write_atomic`

### Notes

- The library enables `serde_json`'s `preserve_order` feature, so a merge keeps the
  user's key order. Cargo turns features on for the whole build: every crate
  using `serde_json` beside the library gets insertion-ordered maps.
- MSRV is 1.85. Raising it is a minor release, made only when needed.
- Pre-1.0: minor versions may contain breaking changes.

## Contributing

See [CONTRIBUTING.md](https://github.com/six5536/agent-harness-adapter/blob/main/CONTRIBUTING.md).

## License

MIT. See [LICENSE](https://github.com/six5536/agent-harness-adapter/blob/main/LICENSE).
