//! `ahk`: install a tool's integration into LLM agent harnesses from a
//! manifest file, and bridge their hooks to a command in any language.
// @zen-component: AHK-Cli

use clap::Parser;

/// Plug a command-line tool into LLM agent harnesses.
#[derive(Debug, Parser)]
#[command(name = "ahk", version, about)]
struct Cli {}

fn main() {
    Cli::parse();
}
