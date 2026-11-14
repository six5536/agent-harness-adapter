//! `agent-harness-adapter schema`: the JSON Schema of each contract.

use agent_harness_adapter_core::{
    InstallResult, cli,
    hook::{file, wire},
    manifest::Manifest,
};
use clap::ValueEnum;
use serde_json::Value;

// sokf:begin contract
/// A contract with a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Contract {
    /// The manifest file.
    Manifest,
    /// The hooks file (`.agents/hooks.json`).
    Hooks,
    /// The hook input a hook command reads (Claude Code's).
    HookInput,
    /// The answer JSON a hook command writes (Claude Code's).
    HookAnswer,
    /// `install` / `status --json`.
    Result,
}
// sokf:end contract

impl Contract {
    /// The schema.
    pub fn schema(self) -> Value {
        match self {
            Contract::Manifest => Manifest::schema(),
            Contract::Hooks => file::schema(),
            Contract::HookInput => wire::input_schema(),
            Contract::HookAnswer => wire::answer_schema(),
            Contract::Result => InstallResult::schema(),
        }
    }
}

/// Print the schema of `contract`, pretty, with a final newline: the text
/// kept in `schema/`.
// sokf-implements: AHA-5_AC-1
// sokf-implements: AHA-5_AC-2
pub fn run(contract: Contract) -> std::io::Result<u8> {
    let mut text = serde_json::to_string_pretty(&contract.schema()).expect("a schema serialises");
    text.push('\n');
    cli::write_stdout(text.as_bytes())?;
    Ok(cli::EXIT_OK)
}
