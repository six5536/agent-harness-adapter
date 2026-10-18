//! Which harnesses a command acts on (AHA-8): `--harness`, else the ones
//! installed, else a question, only when there is a terminal to ask in.
// @zen-component: AHA-Pick

use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
};

use agent_harness_adapter_core::{Scope, Tool, harness::expand, installed};

use crate::error::{Error, Result};

/// Where the harnesses came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// `--harness`.
    Given(Vec<String>),
    /// The ones installed at the scope.
    Installed(Vec<String>),
    /// The user's pick.
    Picked(Vec<String>),
}

impl Choice {
    /// The harness ids.
    pub fn ids(&self) -> &[String] {
        match self {
            Choice::Given(i) | Choice::Installed(i) | Choice::Picked(i) => i,
        }
    }
}

/// Whether the user can be asked: stdin and stderr are terminals.
pub fn interactive() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// The harnesses for `install`: `given` expanded; else the installed ones;
/// else, when `interactive`, the user's pick; else a refusal.
// @zen-impl: AHA-8_AC-2
// @zen-impl: AHA-8_AC-3
// @zen-impl: AHA-8_AC-5
pub fn choose<T: Tool + ?Sized>(
    given: &[String],
    tool: &T,
    scope: Scope,
    interactive: bool,
    ask: impl FnOnce(&[(String, bool)]) -> Result<Vec<String>>,
) -> Result<Choice> {
    if !given.is_empty() {
        return Ok(Choice::Given(expand(tool, given, scope)));
    }
    let now = installed(tool, scope)?;
    if !now.is_empty() {
        return Ok(Choice::Installed(now));
    }
    if !interactive {
        return Err(Error::Usage(
            "say which harnesses: --harness <ids|all>".into(),
        ));
    }
    let options: Vec<(String, bool)> = tool
        .harnesses()
        .iter()
        .filter(|h| h.scopes().contains(&scope))
        .map(|h| (h.id().to_string(), h.program().is_some_and(on_path)))
        .collect();
    let picked = ask(&options)?;
    if picked.is_empty() {
        return Err(Error::Usage("no harness picked".into()));
    }
    Ok(Choice::Picked(picked))
}

/// Ask on the terminal which of `options` (id, found) to install.
pub fn ask(options: &[(String, bool)]) -> Result<Vec<String>> {
    let labels: Vec<String> = options
        .iter()
        .map(|(id, found)| {
            if *found {
                format!("{id}  (found)")
            } else {
                id.clone()
            }
        })
        .collect();
    let defaults: Vec<bool> = options.iter().map(|(_, found)| *found).collect();
    let picked = dialoguer::MultiSelect::new()
        .with_prompt("Which agents do you use? (space to pick, enter to confirm)")
        .items(&labels)
        .defaults(&defaults)
        .interact_on_opt(&dialoguer::console::Term::stderr())
        .map_err(|e| Error::Usage(format!("could not ask: {e}")))?
        .ok_or_else(|| Error::Usage("no harness picked".into()))?;
    Ok(picked.into_iter().map(|i| options[i].0.clone()).collect())
}

/// Whether `program` is on `PATH`.
pub fn on_path(program: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    find_in(program, &path, cfg!(windows), &exts).is_some()
}

/// `program` in the directories of `path`: with each `exts` ending on
/// Windows; a file, executable on Unix.
fn find_in(program: &str, path: &std::ffi::OsStr, windows: bool, exts: &str) -> Option<PathBuf> {
    std::env::split_paths(path).find_map(|dir| {
        let names: Vec<String> = if windows {
            exts.split(';')
                .filter(|e| !e.is_empty())
                .map(|e| format!("{program}{e}"))
                .collect()
        } else {
            vec![program.to_string()]
        };
        names.into_iter().map(|n| dir.join(n)).find(|p| runnable(p))
    })
}

#[cfg(unix)]
fn runnable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    p.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn runnable(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn programs_are_found_when_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("aha-pick-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("claude");
        std::fs::write(&exe, "").unwrap();
        std::fs::write(dir.join("codex"), "").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = std::env::join_paths([dir.join("none"), dir.clone()]).unwrap();
        assert_eq!(find_in("claude", &path, false, ""), Some(exe));
        assert_eq!(find_in("codex", &path, false, ""), None);
        assert_eq!(find_in("gemini", &path, false, ""), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn windows_tries_each_ending() {
        let dir = std::env::temp_dir().join(format!("aha-pick-win-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let shim = dir.join("codex.CMD");
        std::fs::write(&shim, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::env::join_paths([dir.clone()]).unwrap();
        assert_eq!(find_in("codex", &path, true, ".EXE;.CMD"), Some(shim));
        assert_eq!(find_in("codex", &path, false, ".EXE;.CMD"), None);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn tool(dir: &Path) -> agent_harness_adapter_core::manifest::ManifestTool {
        use agent_harness_adapter_core::manifest::{Manifest, ManifestTool, config_text};
        let text = config_text(Scope::Project, "agents");
        let m = Manifest::from_text(&text, false, dir, "config").unwrap();
        ManifestTool::new(m, dir, dir.join("home"))
    }

    // @zen-test: AHA-8_AC-2
    // @zen-test: AHA-8_AC-3
    // @zen-test: AHA-8_AC-5
    #[test]
    fn harnesses_come_from_the_flag_the_record_or_a_question() {
        let dir = std::env::temp_dir().join(format!("aha-pick-choose-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("AGENTS.md"), "Be kind.\n").unwrap();
        let t = tool(&dir);
        let never = |_: &[(String, bool)]| -> Result<Vec<String>> { panic!("asked") };
        let given = choose(&["all".to_string()], &t, Scope::Local, false, never).unwrap();
        assert_eq!(
            given,
            Choice::Given(vec!["claude".into(), "copilot".into()])
        );
        let e = choose(&[], &t, Scope::Project, false, never)
            .unwrap_err()
            .to_string();
        assert_eq!(e, "say which harnesses: --harness <ids|all>");
        let mut offered = Vec::new();
        let picked = choose(&[], &t, Scope::Project, true, |o| {
            offered = o.to_vec();
            Ok(vec!["codex".into()])
        })
        .unwrap();
        assert_eq!(picked, Choice::Picked(vec!["codex".into()]));
        assert_eq!(offered.len(), 12);
        assert_eq!(offered.last().unwrap(), &("agents".to_string(), false));
        let e = choose(&[], &t, Scope::Project, true, |_| Ok(vec![]))
            .unwrap_err()
            .to_string();
        assert_eq!(e, "no harness picked");
        agent_harness_adapter_core::install(
            &t,
            &agent_harness_adapter_core::InstallOptions::new(["claude"], Scope::Project),
        )
        .unwrap();
        let again = choose(&[], &t, Scope::Project, true, never).unwrap();
        assert_eq!(again, Choice::Installed(vec!["claude".into()]));
        assert_eq!(again.ids(), ["claude"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
