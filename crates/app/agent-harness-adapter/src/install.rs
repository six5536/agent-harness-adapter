//! `agent-harness-adapter install`, `uninstall` and `status`: from a
//! manifest, or from the CONFIG, found or (on the first install) written.

use std::{
    io::Write,
    path::{Path, PathBuf},
};

use agent_harness_adapter_core::{
    InstallOptions, InstallResult, Scope, State, UninstallOptions, cli, fs, install,
    manifest::{
        CONFIG_DIR, CONFIG_NAME, Manifest, ManifestTool, config_text, find_config, standard_sources,
    },
    status, uninstall,
};
use clap::Args;

use crate::{
    error::{Error, Result},
    migrate,
    pick::{self, Choice},
};

/// The options every command shares.
#[derive(Debug, Args)]
pub struct Common {
    /// A tool's manifest (TOML, or JSON when it ends in `.json`); without
    /// it, the config `.agents/agent-harness-adapter.toml`, found from the
    /// working directory upwards.
    #[arg(long, value_name = "FILE")]
    pub manifest: Option<PathBuf>,
    /// project, user or local.
    #[arg(long, default_value = "project", value_name = "SCOPE")]
    pub scope: Scope,
    /// The project directory (default: the config's, else the working
    /// directory).
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
    /// The harnesses, comma-separated or repeated: ids, or `all` (default:
    /// the installed ones, else a question).
    #[arg(long, value_name = "IDS", value_delimiter = ',')]
    pub harness: Vec<String>,
    /// The name a new config gets: its markers and record (default `agents`).
    #[arg(long, value_name = "NAME")]
    pub name: Option<String>,
    /// Leave a part out (instructions, skills, hooks, mcp, …); repeatable.
    /// Given at all, it replaces the parts declined before.
    #[arg(long, value_name = "PART", value_delimiter = ',')]
    pub without: Vec<String>,
    /// Overwrite parts edited by hand; migrate files git has uncommitted
    /// changes in.
    #[arg(long)]
    pub force: bool,
    /// Move Claude Code's setup into the standard places without asking
    /// (also once the config exists).
    #[arg(long)]
    pub migrate: bool,
}

/// `agent-harness-adapter uninstall`.
#[derive(Debug, Args)]
pub struct UninstallArgs {
    #[command(flatten)]
    pub common: Common,
    /// The harnesses, comma-separated or repeated: ids, or `all` (default:
    /// all the installed ones).
    #[arg(long, value_name = "IDS", value_delimiter = ',')]
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

/// The manifest as a tool; the CONFIG it came from (its path and root),
/// and its text when it is still to be written.
struct Loaded {
    tool: ManifestTool,
    config: Option<(PathBuf, PathBuf)>,
    write: Option<String>,
}

fn home() -> Result<PathBuf> {
    fs::home_dir().ok_or_else(|| Error::Usage("no home directory: set HOME".into()))
}

/// The tool for `common`: `--manifest`, else the CONFIG found; else, when
/// `create` names one, a new CONFIG of that name (written by the caller),
/// else a refusal.
// @zen-impl: AHA-7_AC-4
// @zen-impl: AHA-7_AC-5
fn load(common: &Common, create: Option<&str>) -> Result<Loaded> {
    let cwd = std::env::current_dir()?;
    let scope = common.scope;
    if let Some(m) = &common.manifest {
        let manifest = Manifest::load(m)?;
        let project = common.root.clone().unwrap_or(cwd);
        return Ok(Loaded {
            tool: ManifestTool::new(manifest, project, home()?),
            config: None,
            write: None,
        });
    }
    let home = home()?;
    if let Some(found) = find_config(scope, common.root.as_deref(), &cwd, &home)? {
        let manifest = Manifest::load_in(&found.path, &found.root)?;
        let project = match scope {
            Scope::User => common.root.clone().unwrap_or(cwd),
            _ => found.root.clone(),
        };
        return Ok(Loaded {
            tool: ManifestTool::new(manifest, project, home),
            config: Some((found.path, found.root)),
            write: None,
        });
    }
    let Some(name) = create else {
        return Err(Error::Usage(
            "no config: run agent-harness-adapter install".into(),
        ));
    };
    if scope == Scope::Local {
        return Err(Error::Usage(
            "no config: run agent-harness-adapter install at project scope first".into(),
        ));
    }
    let root = match scope {
        Scope::User => home.clone(),
        _ => common
            .root
            .clone()
            .unwrap_or_else(|| repo_root(&cwd, &home)),
    };
    let path = root.join(CONFIG_DIR).join(format!("{CONFIG_NAME}.toml"));
    if path.exists() {
        return Err(Error::Usage(format!(
            "{} exists but was not found from here: run from its project, or pass --root",
            path.display()
        )));
    }
    let text = config_text(scope, name);
    let manifest = Manifest::from_text(&text, false, &root, &path.display().to_string())?;
    let project = if scope == Scope::User {
        cwd
    } else {
        root.clone()
    };
    Ok(Loaded {
        tool: ManifestTool::new(manifest, project, home),
        config: Some((path, root)),
        write: Some(text),
    })
}

/// `--root`, else the working directory.
fn common_root(common: &Common) -> Result<PathBuf> {
    Ok(match &common.root {
        Some(r) => r.clone(),
        None => std::env::current_dir()?,
    })
}

/// Where a new project CONFIG goes: the nearest directory from `cwd`
/// upwards holding `.git` (not past `home`), else `cwd`.
fn repo_root(cwd: &Path, home: &Path) -> PathBuf {
    cwd.ancestors()
        .take_while(|d| *d != home)
        .find(|d| d.join(".git").exists())
        .unwrap_or(cwd)
        .to_path_buf()
}

// @zen-impl: AHA-3_AC-1
// @zen-impl: AHA-8_AC-1
// @zen-impl: AHA-8_AC-4
pub fn run_install(args: &InstallArgs) -> Result<u8> {
    let name = args.name.as_deref().unwrap_or("agents");
    let loaded = load(&args.common, Some(name))?;
    let scope = args.common.scope;
    let choice = pick::choose(
        &args.harness,
        &loaded.tool,
        scope,
        pick::interactive(),
        pick::ask,
    )?;
    let mut err = std::io::stderr();
    if let Choice::Installed(ids) = &choice {
        writeln!(err, "updating {}", ids.join(", "))?;
    }
    let migration = match &loaded.config {
        Some((_, root)) if scope != Scope::Local && (loaded.write.is_some() || args.migrate) => {
            let plan = match migrate::plan(scope, root) {
                Ok(plan) => plan,
                // @zen-impl: AHA-9_AC-4
                // Only offered: a conflict does not stop the install.
                Err(e) if !args.migrate => {
                    writeln!(err, "note: {e}")?;
                    migrate::Plan::default()
                }
                Err(e) => return Err(e),
            };
            (!plan.is_empty() && wants_migration(args, &plan, root, &mut err)?).then_some(plan)
        }
        _ => None,
    };
    if let (Some(plan), Some((_, root)), Scope::Project, false) =
        (&migration, &loaded.config, scope, args.force)
    {
        migrate::check_git(plan, root)?;
    }
    if let (Some(text), Some((path, _))) = (&loaded.write, &loaded.config) {
        fs::write_atomic(path, text)?;
        writeln!(err, "wrote {}", path.display())?;
    }
    let mut loaded = loaded;
    if let (Some(plan), Some((path, root))) = (&migration, &loaded.config) {
        if let Some(backup) = migrate::apply(plan, scope, root)? {
            writeln!(
                err,
                "backed up the files it changed to {}",
                backup.display()
            )?;
        }
        writeln!(
            err,
            "migrated Claude Code's setup; edit AGENTS.md and .agents/ from now on"
        )?;
        // The sources changed: read them again.
        let manifest = Manifest::load_in(path, root)?;
        let project = match scope {
            Scope::User => common_root(&args.common)?,
            _ => root.clone(),
        };
        loaded.tool = ManifestTool::new(manifest, project, home()?);
    }
    let mut opts = InstallOptions::new(choice.ids().iter().cloned(), scope).force(args.force);
    if !args.without.is_empty() {
        opts = opts.without(args.without.iter().cloned());
    }
    let result = install(&loaded.tool, &opts)?;
    let standard = loaded.config.is_some();
    print(
        &args.common,
        &result,
        standard,
        args.force,
        INSTALL_NOTE,
        &mut err,
    )
}

/// Whether to migrate: `--migrate`, else the user's answer when there is a
/// terminal, else no, with a note naming `--migrate`.
// @zen-impl: AHA-9_AC-1
fn wants_migration(
    args: &InstallArgs,
    plan: &migrate::Plan,
    root: &Path,
    err: &mut impl Write,
) -> Result<bool> {
    write!(err, "{}", plan.describe(root))?;
    if args.migrate {
        return Ok(true);
    }
    if !pick::interactive() {
        writeln!(
            err,
            "note: not migrated; `agent-harness-adapter install --migrate` moves it"
        )?;
        return Ok(false);
    }
    dialoguer::Confirm::new()
        .with_prompt("Migrate your current Claude Code settings to agent-harness-adapter?")
        .default(true)
        .interact_on(&dialoguer::console::Term::stderr())
        .map_err(|e| Error::Usage(format!("could not ask: {e}")))
}

// @zen-impl: AHA-3_AC-5
pub fn run_uninstall(args: &UninstallArgs) -> Result<u8> {
    let loaded = load(&args.common, None)?;
    let ids = if args.harness.is_empty() {
        vec!["all".to_string()]
    } else {
        args.harness.clone()
    };
    let opts = UninstallOptions::new(ids, args.common.scope).force(args.force);
    let result = uninstall(&loaded.tool, &opts)?;
    print(
        &args.common,
        &result,
        loaded.config.is_some(),
        args.force,
        UNINSTALL_NOTE,
        &mut std::io::stderr(),
    )
}

const INSTALL_NOTE: &str = "note: parts marked edited were changed by hand and left alone; `agent-harness-adapter install --force` overwrites them";
const UNINSTALL_NOTE: &str = "note: parts marked edited were changed by hand and left alone; `agent-harness-adapter uninstall --force` removes them";

// @zen-impl: AHA-3_AC-2
pub fn run_status(args: &StatusArgs) -> Result<u8> {
    let loaded = load(&args.common, None)?;
    let tool = &loaded.tool;
    let scope = args.common.scope;
    let ids = if args.harness.is_empty() {
        agent_harness_adapter_core::installed(tool, scope)?
    } else {
        agent_harness_adapter_core::harness::expand(tool, &args.harness, scope)
    };
    let result = status(tool, ids, scope)?;
    print(
        &args.common,
        &result,
        loaded.config.is_some(),
        false,
        INSTALL_NOTE,
        &mut std::io::stderr(),
    )?;
    Ok(if result.in_sync() {
        cli::EXIT_OK
    } else {
        cli::EXIT_ERRORS
    })
}

/// The result as text or JSON on stdout, and with text the note about
/// edited parts on `stderr`. From the config (`standard`), the text leaves
/// out the standard sources the user does not use.
// @zen-impl: AHA-3_AC-3
// @zen-impl: AHA-3_AC-4
fn print(
    common: &Common,
    result: &InstallResult,
    standard: bool,
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
    // @zen-impl: AHA-3_AC-6
    let mut shown = result.clone();
    if standard {
        let unused = standard_sources(common.scope);
        shown
            .missing
            .retain(|m| !unused.iter().any(|(i, p)| *i == m.item && *p == m.path));
    }
    cli::write_stdout(shown.to_text().as_bytes())?;
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
