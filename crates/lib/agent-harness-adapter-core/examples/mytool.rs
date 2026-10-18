//! The docs site's Rust example (PLAN-016 N2): a tool that installs its
//! integration and answers its hook.
//!
//! ```sh
//! mytool install <project>             # into Claude Code, Codex and Gemini CLI
//! mytool hook <harness> <event>        # what the installed hooks run
//! ```

use std::{
    io::{self, Read},
    path::PathBuf,
    sync::Arc,
};

use agent_harness_adapter_core::{
    DeclinedStore, Harness, InstallOptions, Integration, Result, Scope, TomlDeclined, Tool,
    harness,
    hook::{Answer, Event, HookInput, ToolKind, claude_names, emit},
    install,
    integration::{Hook, Skill},
};

struct MyTool {
    project: PathBuf,
}

// region: tool
impl Tool for MyTool {
    fn name(&self) -> &str {
        "mytool"
    }

    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        harness::builtin()
    }

    // region: integration
    fn integration(&self, _scope: Scope) -> Integration {
        Integration::new()
            .instructions("Run `mytool check` before you finish.\n")
            .skill(Skill::new(
                "mytool",
                "Check the project with mytool.",
                "Run `mytool check`; fix what it reports.\n",
            ))
            .hook(Hook::new(Event::PreTool, "mytool hook {harness} {event}").tools(ToolKind::Shell))
            .allow_command("mytool")
    }
    // endregion: integration

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
// endregion: tool

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
        ["install", project] => {
            // region: install
            let tool = MyTool {
                project: project.into(),
            };
            let opts = InstallOptions::new(["claude", "codex", "gemini"], Scope::Project);
            print!("{}", install(&tool, &opts)?.to_text());
            // endregion: install
            Ok(())
        }
        ["hook", harness, event] => std::process::exit(hook(harness, event)),
        _ => {
            eprintln!("usage: mytool install <project> | mytool hook <harness> <event>");
            std::process::exit(2)
        }
    }
}

// region: hook
/// `mytool hook <harness> <event>`: read the harness's input, answer in its form.
fn hook(harness: &str, event: &str) -> i32 {
    let (Some(h), Ok(event)) = (harness::find(harness), event.parse::<Event>()) else {
        // Exit 2 would block the agent: a hook that cannot work allows.
        eprintln!("mytool: unknown harness or event: {harness} {event}");
        return 0;
    };
    let mut stdin = String::new();
    let _ = io::stdin().read_to_string(&mut stdin);
    let input = HookInput::parse(h.as_ref(), event, &stdin).unwrap_or_default();
    let command = input
        .tool
        .and_then(|t| claude_names::shell_command(&t.input))
        .unwrap_or_default();
    let answer = if command.contains("rm -rf") {
        Answer::Deny {
            reason: "mytool: no rm -rf here".into(),
        }
    } else {
        Answer::Allow { stderr: None }
    };
    emit(
        h.as_ref(),
        event,
        Ok::<_, String>(answer),
        &mut io::stdout(),
        &mut io::stderr(),
    )
    .map_or(1, i32::from)
}
// endregion: hook
