# agent-harness-kit

[![crates.io](https://img.shields.io/crates/v/agent-harness-kit.svg)](https://crates.io/crates/agent-harness-kit)
[![docs.rs](https://img.shields.io/docsrs/agent-harness-kit)](https://docs.rs/agent-harness-kit)

Shared plumbing for command-line tools that plug into LLM agent harnesses
(Claude Code first). Your tool declares the files a harness reads (an
instructions block, settings entries, skill files) as data; the kit installs
them, reports their state, and never overwrites what the user changed. It
also gives you the harness's hook input and answers, a findings report, CLI
exit and output conventions, and atomic file writes. The kit embeds no
content of its own.

## Features

- `install` / `status` of a tool's harness parts, by scope (`project`, `user`)
- Four part kinds: whole files, a marked region in the user's file, entries
  merged into a JSON file (key order and indent kept), and parts the tool
  manages itself
- States per part: `skipped`, `absent`, `current`, `stale`, `edited`; an
  edited part is only replaced with `--force`
- A refused install writes nothing; every write is atomic and only on change
- Declined parts (`--without`) remembered in the tool's TOML config
- Claude Code: hook input, answers and `emit`, the `CLAUDE.md` / `AGENTS.md`
  rule, `settings.json` hook groups
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
use std::path::PathBuf;

use agent_harness_kit::{
    DeclinedStore, InstallOptions, MergeOp, Part, Profile, Result, Scope, TomlDeclined, Tool,
    claude, install, status,
};

struct MyTool {
    project: PathBuf,
}

impl Tool for MyTool {
    fn name(&self) -> &str {
        "mytool"
    }

    fn profile(&self, harness: &str, _scope: Scope) -> Option<Profile> {
        (harness == "claude").then(|| {
            Profile::new(
                "claude",
                vec![
                    claude::instructions("instructions", "Run `mytool check` before you stop.\n"),
                    Part::merge(
                        "hooks",
                        ".claude/settings.json",
                        vec![claude::hook_command("Stop", "mytool hook ", "mytool hook stop")],
                    ),
                    Part::merge(
                        "permissions",
                        ".claude/settings.json",
                        vec![MergeOp::array_entry(["permissions", "allow"], "Bash(mytool *)")],
                    ),
                ],
            )
        })
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
let result = install(&tool, &InstallOptions::new("claude", Scope::Project))?;
print!("{}", result.to_text());
// created CLAUDE.md (instructions)
// created .claude/settings.json (hooks)
// created .claude/settings.json (permissions)
assert!(status(&tool, "claude", Scope::Project)?
    .parts
    .iter()
    .all(|p| p.verb() == "current"));
# std::fs::remove_dir_all(&tool.project).unwrap();
# Ok(())
# }
```

A hook command answers through the `claude` module:

```rust,no_run
use std::io::{self, Read};

use agent_harness_kit::claude::{Answer, HookInput, emit};

fn main() {
    let mut stdin = String::new();
    let _ = io::stdin().read_to_string(&mut stdin);
    let input = HookInput::parse(&stdin).unwrap_or_default();
    let answer: Result<Answer, String> = Ok(match input.stop_hook_active {
        Some(true) => Answer::Allow { stderr: None },
        _ => Answer::Block { reason: "Run `mytool check` first.".into() },
    });
    let code = emit(answer, &mut io::stdout(), &mut io::stderr()).unwrap_or(1);
    std::process::exit(code.into());
}
```

### Modules

- `harness`: the parts, their states, `install` / `status`, the same for every harness
- `claude`: Claude Code's hook input and answers, instructions file and hook groups
- `report`: `Finding`, `Report`, text and JSON forms
- `cli`: exit codes (0 ok, 1 errors found, 2 usage or internal error), stdout, broken pipes, `finish`
- `fs`: `read_text` and `write_atomic`
- `LoopGuard`: block a stop hook once per text

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
