//! `follow`: a tool hands over its name, a scope and its instructions, and
//! gets back a report and an exit code; the library decides every harness
//! and file (KIT-28).

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    Error, Result, cli, fs,
    harness::{
        self, Context, DeclinedStore, Harness, InstallOptions, InstallResult, Markers, Scope,
        TomlDeclined, Tool, UninstallOptions,
    },
    integration::{BlockOrder, Integration, Item},
    manifest,
};

/// What a tool hands [`follow`]: its name, a scope, its instructions or
/// none, a text to keep, and where its block stands; built with
/// [`Request::new`] and the methods below. It names no harness, file or
/// record.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Request {
    tool: String,
    scope: Scope,
    instructions: Option<String>,
    keep: Option<String>,
    order: BlockOrder,
    root: Option<PathBuf>,
    home: Option<PathBuf>,
}

impl Request {
    /// A request of the tool `tool` at `scope`, with no instructions: its
    /// block goes from every harness ([`follow`]).
    pub fn new(tool: impl Into<String>, scope: Scope) -> Self {
        Request {
            tool: tool.into(),
            scope,
            instructions: None,
            keep: None,
            order: BlockOrder::Installed,
            root: None,
            home: None,
        }
    }

    /// With the tool's instructions, its block in every harness the project
    /// uses.
    #[must_use]
    pub fn instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = Some(text.into());
        self
    }

    /// With a text the project keeps as its own beside the tool's block
    /// (KIT-27), kept before the block changes.
    #[must_use]
    pub fn keep(mut self, text: impl Into<String>) -> Self {
        self.keep = Some(text.into());
        self
    }

    /// With the tool's block in the order `order` among other tools'
    /// (KIT-26).
    #[must_use]
    pub fn block_order(mut self, order: BlockOrder) -> Self {
        self.order = order;
        self
    }

    /// With the project directory (default: the nearest CONFIG's project
    /// above the working directory, else the git repository's root, else
    /// the working directory).
    #[must_use]
    pub fn root(mut self, dir: impl Into<PathBuf>) -> Self {
        self.root = Some(dir.into());
        self
    }

    /// With the home directory (default: `HOME`, else `USERPROFILE`).
    #[must_use]
    pub fn home(mut self, dir: impl Into<PathBuf>) -> Self {
        self.home = Some(dir.into());
        self
    }
}

/// What [`follow`] gives back: a report to print as it stands, and the
/// command's exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Outcome {
    /// The report; `error: <message>` for a refusal.
    pub report: String,
    /// 0 done; 1 a block edited by hand left as found; 2 a refusal.
    pub exit: u8,
}

/// The tool a [`Request`] stands for: the built-in harnesses, the
/// instructions alone, and the manifest's default paths under the scope's
/// root.
struct Follower {
    name: String,
    harnesses: Vec<Arc<dyn Harness>>,
    integration: Integration,
    root: PathBuf,
    home: Option<PathBuf>,
}

impl Tool for Follower {
    fn name(&self) -> &str {
        &self.name
    }

    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        self.harnesses.clone()
    }

    fn integration(&self, _scope: Scope) -> Integration {
        self.integration.clone()
    }

    fn root(&self, scope: Scope) -> Result<PathBuf> {
        match scope {
            Scope::User => self
                .home
                .clone()
                .ok_or_else(|| Error::Refused("no home directory: set HOME".into())),
            _ => Ok(self.root.clone()),
        }
    }

    // sokf-implements: KIT-28_AC-3
    fn record_path(&self, scope: Scope) -> Result<PathBuf> {
        let file = match scope {
            Scope::Local => "harness.local.toml",
            _ => "harness.toml",
        };
        Ok(self.root(scope)?.join(format!(".{}", self.name)).join(file))
    }

    fn declined_store(&self, scope: Scope) -> Result<Box<dyn DeclinedStore + '_>> {
        let rel = format!(".{}/config.toml", self.name);
        let shown = match scope {
            Scope::User => format!("~/{rel}"),
            _ => rel.clone(),
        };
        Ok(Box::new(TomlDeclined::new(
            self.root(scope)?.join(rel),
            shown,
        )))
    }
}

/// Install, update or take out a tool's instructions in every harness the
/// project uses, as the library knows them (KIT-28), and keep a text as the
/// project's own first. Never fails: a refusal is the outcome's report,
/// `error: <message>`, with the exit code 2, and comes before any write;
/// a write that fails later reports what was done before it.
// sokf-implements: KIT-28_AC-6
pub fn follow(request: &Request) -> Outcome {
    let mut report = String::new();
    match run(request, &mut report) {
        Ok(exit) => Outcome { report, exit },
        Err(e) => {
            let _ = writeln!(report, "error: {e}");
            Outcome {
                report,
                exit: cli::EXIT_FAILURE,
            }
        }
    }
}

/// The project directory when none is given: the one the nearest CONFIG
/// above the working directory belongs to, as `agent-harness-adapter`
/// finds it (AHA-7); else the git repository's root above the working
/// directory (never at or above home); else the working directory.
fn project_root(home: Option<&Path>) -> Result<PathBuf> {
    let cwd = std::env::current_dir().map_err(|e| Error::io(".", e))?;
    // With no home, the search runs up to the file system's root.
    let top = home.unwrap_or(Path::new("/"));
    if let Ok(Some(found)) = manifest::find_config(Scope::Project, None, &cwd, top) {
        return Ok(found.root);
    }
    let git = cwd
        .ancestors()
        .take_while(|d| home.is_none_or(|h| !fs::same_dir(d, h)))
        .find(|d| d.join(".git").exists())
        .map(Path::to_path_buf);
    Ok(git.unwrap_or(cwd))
}

/// The CONFIG of the scope: its name, the harnesses its record holds, and
/// the record's path. Read as plain data, the record alone: a CONFIG or a
/// record that cannot be read gives none, since the tool can neither read
/// nor fix the project's own setup.
#[derive(Default)]
struct Configured {
    name: Option<String>,
    harnesses: Vec<String>,
    record: Option<PathBuf>,
}

/// The CONFIG of `scope` for the project `project`, whose files are under
/// `base`, the scope's root.
fn configured(project: &Path, base: &Path, home: Option<&Path>, scope: Scope) -> Configured {
    let found = manifest::find_config(scope, Some(project), project, home.unwrap_or(base));
    let Ok(Some(found)) = found else {
        return Configured::default();
    };
    let Ok(Some(text)) = fs::read_text(&found.path) else {
        return Configured::default();
    };
    let value: Option<serde_json::Value> = if found.path.extension().is_some_and(|e| e == "json") {
        serde_json::from_str(&text).ok()
    } else {
        toml_edit::de::from_str(&text).ok()
    };
    let Some(value) = value else {
        return Configured::default();
    };
    let name = value["name"].as_str().unwrap_or("agents");
    let rel = manifest::record_path(
        name,
        value["record"].as_str(),
        value["scopes"][scope.as_str()]["record"].as_str(),
        scope,
    );
    let path = base.join(&rel);
    let harnesses = harness::read_record(&path, &rel)
        .map(|r| r.harnesses.into_keys().collect())
        .unwrap_or_default();
    Configured {
        name: Some(name.to_string()),
        harnesses,
        record: Some(path),
    }
}

// sokf-implements: KIT-28_AC-1
// sokf-implements: KIT-28_AC-3
// sokf-implements: KIT-28_AC-4
// sokf-implements: KIT-28_AC-5
// sokf-implements: KIT-28_AC-7
// sokf-implements: KIT-28_AC-8
#[expect(
    clippy::too_many_lines,
    reason = "one pass over the request, whose steps share the report and the exit code"
)]
fn run(request: &Request, report: &mut String) -> Result<u8> {
    let name = &request.tool;
    if !harness::is_tool_name(name) {
        return Err(Error::Refused(format!(
            "`{name}` is no tool name: use letters, digits, `-`, `_` and `.`, not dots alone"
        )));
    }
    if let Some(line) = request.keep.as_deref().and_then(Markers::marker_line) {
        return Err(Error::MarkedText { line });
    }
    let scope = request.scope;
    let home = request.home.clone().or_else(fs::home_dir);
    let root = match (&request.root, scope, &home) {
        (Some(r), _, _) => r.clone(),
        // At user scope the project is not used: the home directory is.
        (None, Scope::User, Some(h)) => h.clone(),
        (None, _, _) => project_root(home.as_deref())?,
    };
    // Whitespace alone is no instructions: the block goes.
    let instructions = request
        .instructions
        .clone()
        .filter(|t| !t.trim().is_empty());
    let integration = Integration::new()
        .instructions(instructions.clone().unwrap_or_default())
        .block_order(request.order);
    let tool = Follower {
        name: name.clone(),
        harnesses: harness::builtin(),
        integration,
        root,
        home,
    };
    // The scope's root, where every file of the scope is.
    let base = tool.root(scope)?;
    let config = configured(&tool.root, &base, tool.home.as_deref(), scope);
    let record = tool.record_path(scope)?;
    let shown = fs::shown(&base, &record);
    // The CONFIG's markers and record, or a record that holds other parts,
    // belong to a tool with other items: following under them would take
    // over its blocks, or orphan its parts.
    let setups = config.name.as_deref() == Some(name.as_str())
        || config
            .record
            .as_deref()
            .is_some_and(|c| fs::same_dir(c, &record));
    if setups {
        return Err(Error::Refused(format!(
            "`{name}` is the name of the project's own setup: the tool needs a name of its own"
        )));
    }
    let held = harness::read_record(&record, &shown)?;
    if let Some(part) = held
        .harnesses
        .values()
        .flat_map(|t| t.keys())
        .find(|p| *p != Item::Instructions.as_str())
    {
        return Err(Error::Refused(format!(
            "{shown} holds the part `{part}` of another integration: `{name}` follows the project with its instructions alone"
        )));
    }
    let recorded: Vec<String> = tool
        .harnesses
        .iter()
        .map(|h| h.id().to_string())
        .filter(|id| held.harnesses.contains_key(id))
        .collect();
    // With instructions, the harnesses in use get the block, and the ones
    // the tool was in and the project no longer uses lose it; with none,
    // every recorded one loses it.
    let wanted = match instructions {
        Some(_) => used(&tool, scope, &base, &config.harnesses),
        None => Vec::new(),
    };
    let gone: Vec<String> = recorded
        .iter()
        .filter(|id| !wanted.contains(id))
        .cloned()
        .collect();
    // Every refusal of the steps below comes before any write: a status of
    // the same set reads and checks what they read.
    let all: Vec<&String> = wanted.iter().chain(&gone).collect();
    if !all.is_empty() {
        harness::status(&tool, all, scope)?;
    }
    // The kept text goes beside the block the tool has now, never into a
    // file of no block, nor, with instructions, of a harness that leaves,
    // whose file would then show use by the text.
    if let Some(text) = &request.keep {
        let into = if instructions.is_some() {
            &wanted
        } else {
            &recorded
        };
        let kept = if recorded.is_empty() {
            None
        } else {
            harness::keep_beside(&tool, text, scope, into)?
        };
        match kept {
            Some(kept) => report.push_str(&kept.to_text()),
            None => {
                let _ = writeln!(
                    report,
                    "no block of `{name}` at {scope} scope: nothing kept"
                );
            }
        }
    }
    let mut exit = cli::EXIT_OK;
    // The leaving harnesses lose the block first, also from a file that
    // only covers a harness that stays (`own_only`), which the install then
    // writes where it reads it; a file a staying harness writes itself, and
    // every other block, stays where it is.
    if !gone.is_empty() {
        let mut opts = UninstallOptions::new(gone.clone(), scope);
        opts.own_only = true;
        let result = harness::uninstall(&tool, &opts)?;
        report.push_str(&result.to_text());
        exit = exit.max(code(&result));
    } else if instructions.is_none() {
        let _ = writeln!(
            report,
            "no block of `{name}` at {scope} scope: nothing removed"
        );
    }
    if instructions.is_some() {
        if wanted.is_empty() {
            let rest = if gone.is_empty() {
                ": nothing written"
            } else {
                ""
            };
            let _ = writeln!(report, "no harness in use at {scope} scope{rest}");
        } else {
            let result = harness::install(&tool, &InstallOptions::new(wanted, scope))?;
            report.push_str(&result.to_text());
            exit = exit.max(code(&result));
        }
    }
    Ok(exit)
}

/// 1 when a part edited by hand was left as found, else 0.
fn code(result: &InstallResult) -> u8 {
    if result.edited_left() {
        cli::EXIT_ERRORS
    } else {
        cli::EXIT_OK
    }
}

/// The harnesses the project uses at `scope` (KIT-28_AC-2), under `base`,
/// the scope's root: those the project's own setup installed (`config`,
/// the CONFIG's record), and those whose own files show use, in the
/// library's order; only those that take instructions at the scope.
// sokf-implements: KIT-28_AC-2
fn used(tool: &Follower, scope: Scope, base: &Path, config: &[String]) -> Vec<String> {
    let cx = Context::new(&tool.name, scope, base.to_path_buf(), tool.home.clone());
    let probe = Integration::new().instructions("x");
    let takes = |h: &Arc<dyn Harness>| {
        h.render(&probe, &cx).is_ok_and(|parts| {
            parts
                .iter()
                .any(|p| p.name() == Item::Instructions.as_str())
        })
    };
    tool.harnesses
        .iter()
        .filter(|h| h.scopes().contains(&scope) && takes(h))
        .filter(|h| h.used(&cx) || config.iter().any(|id| id == h.id()))
        .map(|h| h.id().to_string())
        .collect()
}
