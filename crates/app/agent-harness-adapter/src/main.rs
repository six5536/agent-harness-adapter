//! `agent-harness-adapter`: one agent setup for every coding agent. Install
//! a setup kept in `AGENTS.md` and `.agents/` (or a tool's manifest) into
//! LLM agent harnesses, and run hooks in Claude Code's format in each.
// @zen-component: AHA-Cli

mod bridge;
mod dispatch;
mod error;
mod install;
mod migrate;
mod pick;
mod schema;

use std::process::ExitCode;

use agent_harness_adapter_core::cli;
use clap::{Parser, Subcommand};

/// One agent setup, every coding agent.
// @zen-impl: AHA-6_AC-1
#[derive(Debug, Parser)]
#[command(name = "agent-harness-adapter", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Install the setup (or a manifest's integration) into harnesses; the
    /// first time, write the config and ask which agents you use.
    Install(install::InstallArgs),
    /// Take it back out of harnesses; your own files stay.
    Uninstall(install::UninstallArgs),
    /// Report its state in harnesses; exits 1 when an install would change
    /// something.
    Status(install::StatusArgs),
    /// Run a harness's hook in Claude Code's hook protocol: through a
    /// command (after `--`), or the matching entries of a hooks file
    /// (`--hooks`).
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
