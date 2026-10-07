//! `agent-harness-adapter`: install a tool's integration into LLM agent harnesses from a
//! manifest file, and bridge their hooks to a command in any language.
// @zen-component: AHA-Cli

mod bridge;
mod error;
mod install;
mod schema;

use std::process::ExitCode;

use agent_harness_adapter_core::cli;
use clap::{Parser, Subcommand};

/// Plug a command-line tool into LLM agent harnesses.
// @zen-impl: AHA-6_AC-1
#[derive(Debug, Parser)]
#[command(name = "agent-harness-adapter", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Install a manifest's integration into harnesses.
    Install(install::InstallArgs),
    /// Take a manifest's integration back out of harnesses.
    Uninstall(install::UninstallArgs),
    /// Report the state of a manifest's integration in harnesses.
    Status(install::StatusArgs),
    /// Run a harness's hook through a command that speaks the hook contract:
    /// the hook input as JSON on its stdin, one answer as JSON on its stdout.
    Hook(bridge::HookArgs),
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
        Cmd::Uninstall(a) => install::run_uninstall(&a),
        Cmd::Status(a) => install::run_status(&a),
        Cmd::Hook(a) => bridge::run(&a).map_err(error::Error::from),
        Cmd::Schema { contract } => schema::run(contract).map_err(error::Error::from),
    })
}
