//! `agent-harness-adapter install`, `uninstall` and `status`.

use std::{io::Write, path::PathBuf};

use agent_harness_adapter_core::{
    InstallOptions, InstallResult, Scope, State, UninstallOptions, cli, fs,
    harness::expand,
    install, installed,
    manifest::{Manifest, ManifestTool},
    status, uninstall,
};
use clap::Args;

use crate::error::{Error, Result};

/// The options `install` and `status` share.
#[derive(Debug, Args)]
pub struct Common {
    /// The tool's manifest (TOML, or JSON when it ends in `.json`).
    #[arg(long, value_name = "FILE")]
    pub manifest: PathBuf,
    /// project, user or local.
    #[arg(long, default_value = "project", value_name = "SCOPE")]
    pub scope: Scope,
    /// The project directory (default: the working directory).
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,
    /// Print the result as JSON.
    #[arg(long)]
    pub json: bool,
}

/// `agent-harness-adapter install`.
#[derive(Debug, Args)]
pub struct InstallArgs {
    #[command(flatten)]
    pub common: Common,
    /// The harnesses, comma-separated or repeated: ids, or `all`.
    #[arg(long, value_name = "IDS", value_delimiter = ',', required = true)]
    pub harness: Vec<String>,
    /// Leave a part out (instructions, skills, hooks, mcp, …); repeatable.
    /// Given at all, it replaces the parts declined before.
    #[arg(long, value_name = "PART", value_delimiter = ',')]
    pub without: Vec<String>,
    /// Overwrite parts edited by hand.
    #[arg(long)]
    pub force: bool,
}

/// `agent-harness-adapter uninstall`.
#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub common: Common,
    /// The harnesses, comma-separated or repeated: ids, or `all` (the
    /// installed ones).
    #[arg(long, value_name = "IDS", value_delimiter = ',', required = true)]
    pub harness: Vec<String>,
    /// Also remove parts edited by hand.
    #[arg(long)]
    pub force: bool,
}

/// `agent-harness-adapter status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    #[command(flatten)]
    pub common: Common,
    /// The harnesses (default: the ones installed before).
    #[arg(long, value_name = "IDS", value_delimiter = ',')]
    pub harness: Vec<String>,
}

/// The manifest as a tool rooted at `--root` (or the working directory)
/// and the home directory.
fn tool(common: &Common) -> Result<ManifestTool> {
    let manifest = Manifest::load(&common.manifest)?;
    let project = match &common.root {
        Some(r) => r.clone(),
        None => std::env::current_dir()?,
    };
    let home = fs::home_dir().ok_or_else(|| Error::Usage("no home directory: set HOME".into()))?;
    Ok(ManifestTool::new(manifest, project, home))
}

// @zen-impl: AHA-3_AC-1
pub fn run_install(args: &InstallArgs) -> Result<u8> {
    let tool = tool(&args.common)?;
    let scope = args.common.scope;
    let mut opts =
        InstallOptions::new(expand(&tool, &args.harness, scope), scope).force(args.force);
    if !args.without.is_empty() {
        opts = opts.without(args.without.iter().cloned());
    }
    let result = install(&tool, &opts)?;
    print(
        &args.common,
        &result,
        args.force,
        INSTALL_NOTE,
        &mut std::io::stderr(),
    )
}

// @zen-impl: AHA-3_AC-5
pub fn run_uninstall(args: &UninstallArgs) -> Result<u8> {
    let tool = tool(&args.common)?;
    let opts =
        UninstallOptions::new(args.harness.iter().cloned(), args.common.scope).force(args.force);
    let result = uninstall(&tool, &opts)?;
    print(
        &args.common,
        &result,
        args.force,
        UNINSTALL_NOTE,
        &mut std::io::stderr(),
    )
}

const INSTALL_NOTE: &str = "note: parts marked edited were changed by hand and left alone; `agent-harness-adapter install --force` overwrites them";
const UNINSTALL_NOTE: &str = "note: parts marked edited were changed by hand and left alone; `agent-harness-adapter uninstall --force` removes them";

// @zen-impl: AHA-3_AC-2
pub fn run_status(args: &StatusArgs) -> Result<u8> {
    let tool = tool(&args.common)?;
    let scope = args.common.scope;
    let ids = if args.harness.is_empty() {
        installed(&tool, scope)?
    } else {
        expand(&tool, &args.harness, scope)
    };
    let result = status(&tool, ids, scope)?;
    print(
        &args.common,
        &result,
        false,
        INSTALL_NOTE,
        &mut std::io::stderr(),
    )
}

/// The result as text or JSON on stdout, and with text the note about
/// edited parts on `stderr`.
// @zen-impl: AHA-3_AC-3
// @zen-impl: AHA-3_AC-4
fn print(
    common: &Common,
    result: &InstallResult,
    force: bool,
    note: &str,
    stderr: &mut impl Write,
) -> Result<u8> {
    if common.json {
        let line = cli::json_line(result)
            .map_err(|e| agent_harness_adapter_core::Error::Internal(e.to_string()))?;
        cli::write_stdout(&line)?;
        return Ok(cli::EXIT_OK);
    }
    cli::write_stdout(result.to_text().as_bytes())?;
    let edited = result
        .harnesses
        .iter()
        .flat_map(|h| &h.parts)
        .any(|p| p.state == State::Edited && p.action.is_none());
    if edited && !force {
        writeln!(stderr, "{note}")?;
    }
    Ok(cli::EXIT_OK)
}
