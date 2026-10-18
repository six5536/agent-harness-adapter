//! The CONFIG (AHA-7): a setup's manifest at its default place,
//! `.agents/agent-harness-adapter.toml` (or `.json`) under the project root,
//! or under the home directory at user scope. Found here, so the CLI and the
//! bindings find it the same way.
// @zen-component: AHA-Config

use std::path::{Path, PathBuf};

use crate::{Error, Item, Result, harness::Scope};

/// The directory the CONFIG is in, under its root.
pub const CONFIG_DIR: &str = ".agents";

/// The CONFIG's file name without its extension.
pub const CONFIG_NAME: &str = "agent-harness-adapter";

/// A CONFIG found: its path, and the root its paths are read from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Found {
    /// The CONFIG file.
    pub path: PathBuf,
    /// The directory holding its `.agents`.
    pub root: PathBuf,
}

/// The CONFIG for `scope`: under `home` at user scope; else in `root` when
/// given; else in [`project_dirs`] from `cwd`. `None` when there is none.
///
/// # Errors
///
/// [`Error::File`] when one `.agents` holds both a `.toml` and a `.json`
/// CONFIG.
// @zen-impl: AHA-7_AC-1
// @zen-impl: AHA-7_AC-2
// @zen-impl: AHA-7_AC-3
pub fn find_config(
    scope: Scope,
    root: Option<&Path>,
    cwd: &Path,
    home: &Path,
) -> Result<Option<Found>> {
    if scope == Scope::User {
        return at(home);
    }
    if let Some(root) = root {
        return at(root);
    }
    for d in project_dirs(cwd, Some(home), CONFIG_DIR) {
        if let Some(found) = at(&d)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// The directories a project's file (under `marker`, e.g. `.agents`) is
/// looked for in, from `start` upwards, never at or above `home`: in a git
/// repository, up to its root (the directory holding `.git`); outside one,
/// up to the first directory that has `marker`. So a file above the
/// project, another project's or one planted there, is never used.
pub fn project_dirs(start: &Path, home: Option<&Path>, marker: &str) -> Vec<PathBuf> {
    let chain: Vec<&Path> = start.ancestors().take_while(|d| Some(*d) != home).collect();
    let end = match chain.iter().position(|d| d.join(".git").exists()) {
        Some(git) => git,
        None => chain
            .iter()
            .position(|d| d.join(marker).exists())
            .unwrap_or(chain.len().saturating_sub(1)),
    };
    chain
        .iter()
        .take(end + 1)
        .map(|d| d.to_path_buf())
        .collect()
}

/// The CONFIG in `dir`'s `.agents`, when there is one.
fn at(dir: &Path) -> Result<Option<Found>> {
    let base = dir.join(CONFIG_DIR);
    let toml = base.join(format!("{CONFIG_NAME}.toml"));
    let json = base.join(format!("{CONFIG_NAME}.json"));
    let found = |path: PathBuf| {
        Some(Found {
            path,
            root: dir.to_path_buf(),
        })
    };
    match (toml.is_file(), json.is_file()) {
        (true, true) => Err(Error::file(
            toml.display().to_string(),
            format!("two configs: use this one or {}, not both", json.display()),
        )),
        (true, false) => Ok(found(toml)),
        (false, true) => Ok(found(json)),
        (false, false) => Ok(None),
    }
}

/// The STANDARD SOURCES at `scope`, each item's path relative to the
/// config's root (the project, or the home directory at user scope): what a
/// new config reads (AHA-1_AC-11).
pub fn standard_sources(scope: Scope) -> [(Item, &'static str); 5] {
    let (instructions, mcp) = match scope {
        Scope::User => (".agents/AGENTS.md", ".agents/mcp.json"),
        _ => ("AGENTS.md", ".mcp.json"),
    };
    [
        (Item::Instructions, instructions),
        (Item::Skills, ".agents/skills"),
        (Item::Hooks, ".agents/hooks.json"),
        (Item::Mcp, mcp),
        (Item::Agents, ".agents/agents"),
    ]
}

/// The text of a new CONFIG named `name` for `scope`: every item from its
/// standard SOURCE (AHA-8_AC-1).
// @zen-impl: AHA-8_AC-1
pub fn config_text(scope: Scope, name: &str) -> String {
    let [i, s, h, m, a] = standard_sources(scope).map(|(_, path)| path);
    format!(
        "# agent-harness-adapter: one agent setup for every coding agent.\n\
         # Edit the files below; `agent-harness-adapter install` brings them to\n\
         # each agent. https://six5536.github.io/agent-harness-adapter/\n\
         version = 1\n\
         name = \"{name}\"\n\
         \n\
         instructions = {{ from = \"{i}\" }}\n\
         skills       = {{ from = \"{s}\" }}\n\
         hooks        = {{ from = \"{h}\" }}\n\
         mcp_servers  = {{ from = \"{m}\" }}\n\
         agents       = {{ from = \"{a}\" }}\n"
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aha-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(dir: &Path, ext: &str) -> PathBuf {
        let p = dir.join(CONFIG_DIR).join(format!("{CONFIG_NAME}.{ext}"));
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "").unwrap();
        p
    }

    // @zen-test: AHA-7_AC-1
    #[test]
    fn found_upwards_until_the_repository() {
        let home = tmp("find");
        let repo = home.join("repo");
        let deep = repo.join("src/a");
        fs::create_dir_all(&deep).unwrap();
        fs::create_dir_all(repo.join(".git")).unwrap();
        assert_eq!(
            find_config(Scope::Project, None, &deep, &home).unwrap(),
            None
        );
        let p = put(&repo, "toml");
        let f = find_config(Scope::Project, None, &deep, &home)
            .unwrap()
            .unwrap();
        assert_eq!((f.path, f.root), (p.clone(), repo.clone()));
        // A package's own `.agents` does not hide the repository's config.
        let sub = repo.join("src");
        fs::create_dir_all(sub.join(".agents")).unwrap();
        assert_eq!(
            find_config(Scope::Project, None, &deep, &home)
                .unwrap()
                .unwrap()
                .path,
            p
        );
        fs::remove_dir_all(sub.join(".agents")).unwrap();
        // Not past the repository's root, nor into the home directory.
        put(&home, "toml");
        let elsewhere = home.join("elsewhere");
        fs::create_dir_all(&elsewhere).unwrap();
        assert_eq!(
            find_config(Scope::Project, None, &elsewhere, &home).unwrap(),
            None
        );
        let outer = tmp("outer");
        put(&outer, "toml");
        let inner = outer.join("x");
        fs::create_dir_all(inner.join(".git")).unwrap();
        assert_eq!(
            find_config(Scope::Project, None, &inner, &home).unwrap(),
            None
        );
        // `--root` looks there only.
        assert_eq!(
            find_config(Scope::Local, Some(&deep), &deep, &home).unwrap(),
            None
        );
        assert_eq!(
            find_config(Scope::Local, Some(&repo), &home, &home)
                .unwrap()
                .unwrap()
                .path,
            p
        );
    }

    // @zen-test: AHA-7_AC-2
    // @zen-test: AHA-7_AC-3
    #[test]
    fn user_scope_and_two_configs() {
        let home = tmp("user");
        assert_eq!(find_config(Scope::User, None, &home, &home).unwrap(), None);
        let j = put(&home, "json");
        assert_eq!(
            find_config(Scope::User, None, Path::new("/"), &home)
                .unwrap()
                .unwrap()
                .path,
            j
        );
        put(&home, "toml");
        let e = find_config(Scope::User, None, &home, &home)
            .unwrap_err()
            .to_string();
        assert!(e.contains("two configs"), "{e}");
    }

    // @zen-test: AHA-8_AC-1
    #[test]
    fn the_new_config_names_the_standard_sources() {
        let t = config_text(Scope::Project, "agents");
        assert!(t.contains("instructions = { from = \"AGENTS.md\" }"));
        assert!(t.contains("mcp_servers  = { from = \".mcp.json\" }"));
        let u = config_text(Scope::User, "me");
        assert!(
            u.contains("name = \"me\"")
                && u.contains("from = \".agents/AGENTS.md\"")
                && u.contains(".agents/mcp.json")
        );
    }

    #[test]
    fn outside_git_the_first_agents_decides() {
        let home = tmp("nogit");
        let outer = home.join("tmp");
        put(&outer, "toml");
        let project = outer.join("x");
        fs::create_dir_all(project.join(".agents")).unwrap();
        let dirs = project_dirs(&project.join("src"), Some(&home), CONFIG_DIR);
        assert_eq!(dirs, vec![project.join("src"), project.clone()]);
        assert_eq!(
            find_config(Scope::Project, None, &project, &home).unwrap(),
            None
        );
    }
}
