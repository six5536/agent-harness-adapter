//! `agent-harness-adapter schema`: the JSON Schema of each contract.

use agent_harness_adapter_core::{InstallResult, cli, hook::wire, manifest::Manifest};
use clap::ValueEnum;
use serde_json::Value;

/// A contract with a schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Contract {
    /// The manifest file.
    Manifest,
    /// The hook input a bridged command reads.
    HookInput,
    /// The answer a bridged command writes.
    HookAnswer,
    /// `install` / `status --json`.
    Result,
}

impl Contract {
    /// The schema.
    pub fn schema(self) -> Value {
        match self {
            Contract::Manifest => Manifest::schema(),
            Contract::HookInput => wire::input_schema(),
            Contract::HookAnswer => wire::answer_schema(),
            Contract::Result => InstallResult::schema(),
        }
    }
}

/// Print the schema of `contract`, pretty, with a final newline: the text
/// kept in `schema/<contract>.v1.json`.
// @zen-impl: AHA-5_AC-1
// @zen-impl: AHA-5_AC-2
pub fn run(contract: Contract) -> std::io::Result<u8> {
    let mut text = serde_json::to_string_pretty(&contract.schema()).expect("a schema serialises");
    text.push('\n');
    cli::write_stdout(text.as_bytes())?;
    Ok(cli::EXIT_OK)
}
