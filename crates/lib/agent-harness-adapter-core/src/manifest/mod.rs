//! A tool's integration declared in a file (AHA-1), for tools not written in
//! Rust: `agent-harness-adapter` and the language bindings load it, and [`ManifestTool`] makes
//! it a [`Tool`].
// @zen-component: AHA-Manifest

mod build;
mod file;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use serde_json::Value;

use crate::{
    Error, Result,
    fs::read_text,
    harness::{self, DeclinedStore, Harness, Scope, TomlDeclined, Tool},
    integration::Integration,
};
use file::{Harnesses, ManifestFile, ScopeFile, VersionOnly};

/// The manifest format's version this library reads.
pub const VERSION: u64 = 1;

const SCOPES: [Scope; 3] = [Scope::Project, Scope::User, Scope::Local];

/// One scope's integration and paths.
#[derive(Debug, Clone)]
struct ScopeData {
    integration: Integration,
    record: String,
    declined: String,
}

/// A loaded manifest: the tool's name, its harnesses, and per scope its
/// integration and where it keeps its record and declined parts. Every TEXT
/// file is read when it loads.
#[derive(Debug, Clone)]
pub struct Manifest {
    name: String,
    harnesses: Vec<String>,
    scopes: Vec<ScopeData>,
}

impl Manifest {
    /// The manifest at `path`: TOML, or JSON when the name ends in `.json`.
    // @zen-impl: AHA-1_AC-1
    pub fn load(path: &Path) -> Result<Manifest> {
        let display = path.display().to_string();
        let text = read_text(path)?.ok_or_else(|| Error::file(&display, "no such file"))?;
        let dir = path.parent().unwrap_or(Path::new("."));
        let json = path.extension().is_some_and(|e| e == "json");
        let manifest = if json {
            parse(&display, serde_json::from_str(&text), || {
                serde_json::from_str(&text)
            })?
        } else {
            parse(&display, toml_edit::de::from_str(&text), || {
                toml_edit::de::from_str(&text)
            })?
        };
        Self::build(manifest, dir, &display)
    }

    /// The manifest in `value` (e.g. from a binding), its TEXT files relative
    /// to `dir`, named `display` in errors.
    pub fn from_json(value: Value, dir: &Path, display: &str) -> Result<Manifest> {
        let manifest = parse(display, serde_json::from_value(value.clone()), || {
            serde_json::from_value(value)
        })?;
        Self::build(manifest, dir, display)
    }

    // @zen-impl: AHA-1_AC-3
    // @zen-impl: AHA-1_AC-6
    fn build(m: ManifestFile, dir: &Path, display: &str) -> Result<Manifest> {
        let err = |message: String| Error::file(display, message);
        if m.name.is_empty()
            || !m
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        {
            return Err(err(format!(
                "`name`: `{}` is not letters, digits, `-`, `_` and `.`",
                m.name
            )));
        }
        let builtin: Vec<String> = harness::builtin()
            .iter()
            .map(|h| h.id().to_string())
            .collect();
        let harnesses = match &m.harnesses {
            None => builtin.clone(),
            Some(Harnesses::All(all)) if all == "all" => builtin.clone(),
            Some(Harnesses::All(other)) => {
                return Err(err(format!(
                    "`harnesses`: `{other}` is not `all` or a list"
                )));
            }
            Some(Harnesses::List(ids)) => {
                if let Some(bad) = ids.iter().find(|id| !builtin.contains(id)) {
                    return Err(err(format!("`harnesses`: no harness `{bad}`")));
                }
                ids.clone()
            }
        };
        let adapter = m.adapter.as_deref().unwrap_or("agent-harness-adapter");
        let cx = build::Ctx {
            dir,
            display,
            name: &m.name,
            adapter,
            harnesses: &harnesses,
        };
        let scopes = m.scopes.clone().unwrap_or_default();
        let mut out = Vec::new();
        for scope in SCOPES {
            let table = match scope {
                Scope::User => scopes.user.as_ref(),
                Scope::Local => scopes.local.as_ref(),
                _ => scopes.project.as_ref(),
            };
            let empty = ScopeFile::default();
            let table = table.unwrap_or(&empty);
            let items = m.items().with(table.items());
            let integration =
                cx.integration(items)
                    .map_err(|e| match (e, table_name(scope, table)) {
                        (Error::File { file, message }, Some(t)) => Error::File {
                            file,
                            message: format!("{message} (at {t} scope)"),
                        },
                        (e, _) => e,
                    })?;
            let record = table
                .record
                .clone()
                .unwrap_or_else(|| match (scope, &m.record) {
                    (Scope::Local, _) => format!(".{}/harness.local.toml", m.name),
                    (_, Some(r)) => r.clone(),
                    _ => format!(".{}/harness.toml", m.name),
                });
            let declined = table
                .declined
                .clone()
                .or_else(|| m.declined.clone())
                .unwrap_or_else(|| format!(".{}/config.toml", m.name));
            out.push(ScopeData {
                integration,
                record,
                declined,
            });
        }
        Ok(Manifest {
            name: m.name,
            harnesses,
            scopes: out,
        })
    }

    /// The tool's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The tool's harness ids, in preference order.
    pub fn harness_ids(&self) -> &[String] {
        &self.harnesses
    }

    /// The integration at `scope`.
    pub fn integration(&self, scope: Scope) -> &Integration {
        &self.scope(scope).integration
    }

    fn scope(&self, scope: Scope) -> &ScopeData {
        let n = SCOPES.iter().position(|s| *s == scope).expect("a scope");
        &self.scopes[n]
    }

    /// The JSON Schema of a manifest file.
    #[cfg(feature = "schemars")]
    pub fn schema() -> Value {
        serde_json::to_value(schemars::schema_for!(ManifestFile)).expect("a schema serialises")
    }
}

/// The scope table's name in an error, when it has items of its own.
fn table_name(scope: Scope, table: &ScopeFile) -> Option<&'static str> {
    let items = table.items();
    let own = items.instructions.is_some()
        || items.hook_match.is_some()
        || items.skills.is_some()
        || items.hooks.is_some()
        || items.mcp_servers.is_some()
        || items.allow_commands.is_some()
        || items.allow_mcp_tools.is_some()
        || items.agents.is_some()
        || items.commands.is_some()
        || items.parts.is_some();
    own.then(|| scope.as_str())
}

/// The manifest file: its version read first (`version`), so a manifest of
/// another version is refused for that, then the whole (`full`).
// @zen-impl: AHA-1_AC-2
// @zen-impl: AHA-1_AC-7
fn parse<E: std::fmt::Display>(
    display: &str,
    version: std::result::Result<VersionOnly, E>,
    full: impl FnOnce() -> std::result::Result<ManifestFile, E>,
) -> Result<ManifestFile> {
    let refuse = |v: u64| {
        Error::file(
            display,
            format!("version {v} is not one agent-harness-adapter reads ({VERSION})"),
        )
    };
    match version {
        Ok(v) if v.version != VERSION => return Err(refuse(v.version)),
        _ => {}
    }
    let m = full().map_err(|e| Error::file(display, e.to_string().trim_end().to_string()))?;
    if m.version != VERSION {
        return Err(refuse(m.version));
    }
    Ok(m)
}

/// A [`Manifest`] as a [`Tool`]: project and local scope under `project`,
/// user scope under `home`.
// @zen-impl: AHA-1_AC-9
#[derive(Debug, Clone)]
pub struct ManifestTool {
    manifest: Manifest,
    project: PathBuf,
    home: PathBuf,
}

impl ManifestTool {
    /// `manifest` for the project directory `project`, with the home
    /// directory `home`.
    pub fn new(manifest: Manifest, project: impl Into<PathBuf>, home: impl Into<PathBuf>) -> Self {
        ManifestTool {
            manifest,
            project: project.into(),
            home: home.into(),
        }
    }

    /// The manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
}

impl Tool for ManifestTool {
    fn name(&self) -> &str {
        &self.manifest.name
    }

    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        self.manifest
            .harnesses
            .iter()
            .filter_map(|id| harness::find(id))
            .collect()
    }

    fn integration(&self, scope: Scope) -> Integration {
        self.manifest.integration(scope).clone()
    }

    fn root(&self, scope: Scope) -> Result<PathBuf> {
        Ok(match scope {
            Scope::User => self.home.clone(),
            _ => self.project.clone(),
        })
    }

    fn record_path(&self, scope: Scope) -> Result<PathBuf> {
        Ok(self.root(scope)?.join(&self.manifest.scope(scope).record))
    }

    fn declined_store(&self, scope: Scope) -> Result<Box<dyn DeclinedStore + '_>> {
        let path = &self.manifest.scope(scope).declined;
        let display = match scope {
            Scope::User => format!("~/{path}"),
            _ => path.clone(),
        };
        Ok(Box::new(TomlDeclined::new(
            self.root(scope)?.join(path),
            display,
        )))
    }
}

#[cfg(test)]
mod tests;
