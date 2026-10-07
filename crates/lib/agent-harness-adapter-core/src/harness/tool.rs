//! The tool a harness integration belongs to: its name, its integration,
//! the harnesses it supports and where it keeps things.

use std::{fmt, path::PathBuf, str::FromStr, sync::Arc};

use serde::Serialize;

use crate::{
    Error, Result,
    harness::{DeclinedStore, Harness},
    integration::Integration,
};

/// Where a harness integration is installed.
// @zen-impl: KIT-21_AC-1
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Scope {
    /// The project: files beside the project's own files.
    #[default]
    Project,
    /// The user: files under the home directory, in each harness's own
    /// configuration directory.
    User,
    /// The project's local, git-ignored files (e.g.
    /// `.claude/settings.local.json`), under the project root.
    Local,
}

impl Scope {
    /// The scope's name, as `--scope` takes it.
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::Project => "project",
            Scope::User => "user",
            Scope::Local => "local",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Scope {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "project" => Ok(Scope::Project),
            "user" => Ok(Scope::User),
            "local" => Ok(Scope::Local),
            other => Err(Error::UnknownScope {
                name: other.to_string(),
            }),
        }
    }
}

/// A CLI that plugs into agent harnesses. [`install`](crate::harness::install)
/// and [`status`](crate::harness::status) are generic over it.
// @zen-impl: KIT-1_AC-1
pub trait Tool {
    /// The tool's name, e.g. `mytool`: the region markers are
    /// `<!-- mytool:harness -->` and `<!-- /mytool:harness -->`.
    fn name(&self) -> &str;

    /// The harnesses the tool supports, in the order it prefers them: the
    /// earliest writes a location several share.
    /// [`harness::builtin()`](crate::harness::builtin) for all the library's.
    fn harnesses(&self) -> Vec<Arc<dyn Harness>>;

    /// What the tool installs at `scope`.
    fn integration(&self, scope: Scope) -> Integration;

    /// The directory the parts' relative paths resolve against: the project
    /// directory (project and local scope) or the home directory (user
    /// scope).
    fn root(&self, scope: Scope) -> Result<PathBuf>;

    /// The record file of `scope`.
    fn record_path(&self, scope: Scope) -> Result<PathBuf>;

    /// The store of the declined parts of `scope`.
    fn declined_store(&self, scope: Scope) -> Result<Box<dyn DeclinedStore + '_>>;

    /// The record file's first line.
    fn record_header(&self) -> String {
        format!("# Written by {} harness install. Do not edit.", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_parse_and_print() {
        for s in [Scope::Project, Scope::User, Scope::Local] {
            assert_eq!(s.as_str().parse::<Scope>().unwrap(), s);
            assert_eq!(s.to_string(), s.as_str());
        }
        assert_eq!(
            "global".parse::<Scope>().unwrap_err().to_string(),
            "no scope named `global`"
        );
        assert_eq!(serde_json::to_string(&Scope::User).unwrap(), "\"user\"");
        assert_eq!(Scope::default(), Scope::Project);
    }
}
