# agent-harness-adapter-core

[![crates.io](https://img.shields.io/crates/v/agent-harness-adapter-core.svg)](https://crates.io/crates/agent-harness-adapter-core)
[![docs.rs](https://img.shields.io/docsrs/agent-harness-adapter-core)](https://docs.rs/agent-harness-adapter-core)

One agent setup, every coding agent: the library behind
`agent-harness-adapter`. Write instructions, skills and hooks once, or read
them from the standard places (`AGENTS.md`, `.agents/skills`,
`.agents/hooks.json`, `.mcp.json`, `.agents/agents`), and install them into
Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid,
Pi, OpenCode, Kilo Code, Qwen Code, Devin and any agent that reads
`AGENTS.md`. Shared content is written once, sources are left where agents
read them, and what the user changed is never overwritten. Hooks speak one
contract in every agent: Claude Code's.

**Documentation:** <https://six5536.github.io/agent-harness-adapter/>: the
[Rust guide](https://six5536.github.io/agent-harness-adapter/use/rust/), what
[each harness](https://six5536.github.io/agent-harness-adapter/reference/harnesses/)
gets, and [the hook contract](https://six5536.github.io/agent-harness-adapter/reference/hook-contract/).

Tools in other languages use the same library through the
[`agent-harness-adapter`](https://crates.io/crates/agent-harness-adapter)
command, or the Node and Python bindings.

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
- `hook`: events, `HookInput`, `Answer`, `emit`, `LoopGuard`; `wire`, the hook contract (Claude Code's protocol); `file`, the hooks file; `claude_names`, tool calls in Claude Code's terms
- `manifest`: an integration declared in a TOML or JSON file (`Manifest`, `ManifestTool`), as the `agent-harness-adapter` command reads it; `find_config`, the config `.agents/agent-harness-adapter.toml`
- `claude`, `codex`, `factory`, `gemini`, `copilot`, `cursor`, `pi`, `opencode`, `kilo`, `qwen`, `devin`, `agents_md`: one harness each
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
