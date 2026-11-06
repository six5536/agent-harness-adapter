//! `agent-harness-adapter diff`: a unified diff of each part edited by hand,
//! from the text `install` would write to the text found.

use agent_harness_adapter_core::{Edit, cli, edits, harness::expand, installed};
use clap::Args;
use similar::TextDiff;

use crate::{
    error::Result,
    install::{Common, load},
};

/// `agent-harness-adapter diff`.
#[derive(Debug, Args)]
pub struct DiffArgs {
    #[command(flatten)]
    pub common: Common,
    /// The harnesses (default: the ones installed before).
    #[arg(long, value_name = "IDS", value_delimiter = ',')]
    pub harness: Vec<String>,
}

// @zen-impl: AHA-3_AC-7
pub fn run(args: &DiffArgs) -> Result<u8> {
    let loaded = load(&args.common, None)?;
    let tool = &loaded.tool;
    let scope = args.common.scope;
    let ids = if args.harness.is_empty() {
        installed(tool, scope)?
    } else {
        expand(tool, &args.harness, scope)
    };
    let text: String = edits(tool, ids, scope)?.iter().map(patch).collect();
    cli::write_stdout(text.as_bytes())?;
    Ok(cli::EXIT_OK)
}

/// `edit` as a unified diff with three lines of context.
fn patch(edit: &Edit) -> String {
    TextDiff::from_lines(&edit.expected, &edit.found)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{}", edit.path), &format!("b/{}", edit.path))
        .to_string()
}
