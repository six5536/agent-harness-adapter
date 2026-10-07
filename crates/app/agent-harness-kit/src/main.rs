//! `ahk`: install a tool's integration into LLM agent harnesses from a
//! manifest file, and bridge their hooks to a command in any language.
// @zen-component: AHK-Cli

mod error;
mod install;
mod schema;

use std::process::ExitCode;

use agent_harness_kit_core::cli;
use clap::{Parser, Subcommand};

/// Plug a command-line tool into LLM agent harnesses.
// @zen-impl: AHK-6_AC-1
#[derive(Debug, Parser)]
#[command(name = "ahk", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Install a manifest's integration into harnesses.
    Install(install::InstallArgs),
    /// Report the state of a manifest's integration in harnesses.
    Status(install::StatusArgs),
    /// Print the JSON Schema of a contract.
    Schema {
        /// The contract.
        contract: schema::Contract,
    },
}

fn main() -> ExitCode {
    let args = Cli::parse();
    cli::finish(match args.command {
        Cmd::Install(a) => install::run_install(&a),
        Cmd::Status(a) => install::run_status(&a),
        Cmd::Schema { contract } => schema::run(contract).map_err(error::Error::from),
    })
}
