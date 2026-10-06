# agent-harness-kit

[![crates.io](https://img.shields.io/crates/v/agent-harness-kit.svg)](https://crates.io/crates/agent-harness-kit)
[![docs.rs](https://img.shields.io/docsrs/agent-harness-kit)](https://docs.rs/agent-harness-kit)

Shared plumbing for command-line tools that plug into LLM agent harnesses:
Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor, Factory
Droid, Pi, and any agent that reads `AGENTS.md`. Your tool declares its
integration once (an instructions block, skills, hooks, MCP servers,
allowed commands, subagents, slash commands); the kit writes it into each
harness's own files, puts shared content in one place, reports what it
found, and never overwrites what the user changed. It also turns each
harness's hook input and answers into one shape, and gives you a findings
report, CLI exit and output conventions, and atomic file writes. The kit
embeds no content of its own.

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
  optional)
- Declined parts (`--without`) remembered in the tool's TOML config
- `LoopGuard` so a stop hook blocks only once on the same text
- Findings reports, CLI exit codes, broken-pipe handling, atomic writes

## Installation

### Prerequisites

- Rust 1.85 or newer

### Install

```bash
cargo add agent-harness-kit
```

## Usage

Implement `Tool` for your CLI and call `install` / `status`:

```rust
use std::{path::PathBuf, sync::Arc};

use agent_harness_kit::{
    DeclinedStore, Harness, InstallOptions, Integration, Result, Scope, TomlDeclined, Tool,
    claude::Claude,
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
        vec![Arc::new(Claude)]
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
# let project = std::env::temp_dir().join(format!("ahk-readme-{}", std::process::id()));
# let _ = std::fs::remove_dir_all(&project);
let tool = MyTool { project };
let result = install(&tool, &InstallOptions::new(["claude"], Scope::Project))?;
print!("{}", result.to_text());
// claude:
//   created CLAUDE.md (instructions)
//   created .claude/skills (skills)
//   created .claude/settings.json (hooks)
//   created .mcp.json (mcp)
//   created .claude/settings.json (permissions)
let again = status(&tool, ["claude"], Scope::Project)?;
assert!(again.harnesses[0].parts.iter().all(|p| p.verb() == "current"));
# std::fs::remove_dir_all(&tool.project).unwrap();
# Ok(())
# }
```

The hook command answers through the harness that ran it:

```rust,no_run
use std::io::{self, Read};

use agent_harness_kit::{
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
- `harness`: the `Harness` contract, the parts, their states, `install` / `status`, the same for every harness
- `hook`: events, `HookInput`, `Answer`, `emit`, `LoopGuard`
- `claude`: Claude Code
- `report`: `Finding`, `Report`, text and JSON forms
- `cli`: exit codes (0 ok, 1 errors found, 2 usage or internal error), stdout, broken pipes, `finish`
- `fs`: `read_text` and `write_atomic`

### Notes

- The kit enables `serde_json`'s `preserve_order` feature, so a merge keeps the
  user's key order. Cargo turns features on for the whole build: every crate
  using `serde_json` beside the kit gets insertion-ordered maps.
- MSRV is 1.85. Raising it is a minor release, made only when needed.
- Pre-1.0: minor versions may contain breaking changes.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
