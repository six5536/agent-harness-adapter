//! Migration from Claude Code (AHA-9): when the first install writes the
//! CONFIG, Claude Code's setup moves into the standard places, so every
//! agent gets it; the install that follows points or copies it back.
// @zen-component: AHA-Migrate

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use agent_harness_adapter_core::{Scope, fs as kit_fs, hook::wire};
use regex::Regex;
use serde_json::{Map, Value};

use crate::error::{Error, Result};

/// One move: what, from where, to where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub what: String,
    pub from: PathBuf,
    pub to: PathBuf,
}

/// The migration, planned: the moves, what is left with a reason, and every
/// file write (`None`: remove) and directory removal, in order.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub moves: Vec<Move>,
    pub left: Vec<String>,
    writes: Vec<(PathBuf, Option<String>)>,
    dirs: Vec<PathBuf>,
}

impl Plan {
    /// Whether there is anything to move.
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// The moves and what is left, as lines for the user.
    pub fn describe(&self, base: &Path) -> String {
        let rel = |p: &Path| p.strip_prefix(base).unwrap_or(p).display().to_string();
        let mut out = String::from("Claude Code's setup to move into the standard places:\n");
        for m in &self.moves {
            out.push_str(&format!(
                "  {}: {} -> {}\n",
                m.what,
                rel(&m.from),
                rel(&m.to)
            ));
        }
        for l in &self.left {
            out.push_str(&format!("  not migrated: {l}\n"));
        }
        out
    }

    /// Every file the migration changes or removes.
    fn touched(&self) -> Vec<&Path> {
        let mut out: Vec<&Path> = self.writes.iter().map(|(p, _)| p.as_path()).collect();
        out.sort();
        out.dedup();
        out
    }
}

/// Where Claude Code's setup and the standard places are at `scope`.
struct Places {
    claude_md: PathBuf,
    settings: PathBuf,
    claude_dir: PathBuf,
    instructions: PathBuf,
    agents_dir: PathBuf,
}

fn places(scope: Scope, root: &Path) -> Places {
    let user = scope == Scope::User;
    Places {
        claude_md: root.join(if user {
            ".claude/CLAUDE.md"
        } else {
            "CLAUDE.md"
        }),
        settings: root.join(".claude/settings.json"),
        claude_dir: root.join(".claude"),
        instructions: root.join(if user {
            ".agents/AGENTS.md"
        } else {
            "AGENTS.md"
        }),
        agents_dir: root.join(".agents"),
    }
}

fn read(path: &Path) -> Result<Option<String>> {
    Ok(kit_fs::read_text(path)?)
}

/// The migration of Claude Code's setup at `scope` under `root` (the
/// project root, or the home directory), checked for conflicts.
// @zen-impl: AHA-9_AC-2
// @zen-impl: AHA-9_AC-3
// @zen-impl: AHA-9_AC-4
pub fn plan(scope: Scope, root: &Path) -> Result<Plan> {
    let p = places(scope, root);
    let mut plan = Plan::default();
    instructions(&p, &mut plan)?;
    folders(&p, "skill", "skills", true, &mut plan)?;
    folders(&p, "agent", "agents", false, &mut plan)?;
    hooks(&p, &mut plan)?;
    for (path, what) in [
        (
            p.claude_dir.join("commands"),
            ".claude/commands (slash commands)",
        ),
        (
            p.claude_dir.join("settings.local.json"),
            ".claude/settings.local.json (personal)",
        ),
        (
            root.join(".mcp.json"),
            ".mcp.json (already a standard place)",
        ),
    ] {
        if path.exists() {
            plan.left.push(what.into());
        }
    }
    Ok(plan)
}

/// `CLAUDE.md`'s own text (outside every tool's markers) into the
/// instructions source, under a heading when that has text already.
fn instructions(p: &Places, plan: &mut Plan) -> Result<()> {
    let Some(text) = read(&p.claude_md)? else {
        return Ok(());
    };
    let (own, kept) = split_regions(&text);
    let own: String = own
        .lines()
        .filter(|l| {
            !l.trim().starts_with("@AGENTS.md") && !l.trim().starts_with("@~/.agents/AGENTS.md")
        })
        .map(|l| format!("{l}\n"))
        .collect();
    let own = own.trim();
    if own.is_empty() {
        return Ok(());
    }
    let target = read(&p.instructions)?.unwrap_or_default();
    let merged = if target.trim().is_empty() {
        format!("{own}\n")
    } else if target.contains(own) {
        target
    } else {
        format!("{}\n\n## From CLAUDE.md\n\n{own}\n", target.trim_end())
    };
    plan.moves.push(Move {
        what: "instructions".into(),
        from: p.claude_md.clone(),
        to: p.instructions.clone(),
    });
    plan.writes.push((p.instructions.clone(), Some(merged)));
    plan.writes.push((p.claude_md.clone(), Some(kept)));
    Ok(())
}

/// `text` split into its own lines and the tool regions (`<!-- x:harness
/// -->` … `<!-- /x:harness -->`), which stay where they are.
fn split_regions(text: &str) -> (String, String) {
    let open = Regex::new(r"^<!-- ([^/ ][^ ]*):harness -->$").expect("a regex");
    let (mut own, mut kept) = (String::new(), String::new());
    let mut inside: Option<String> = None;
    for line in text.split_inclusive('\n') {
        let t = line.trim();
        match &inside {
            Some(name) => {
                kept.push_str(line);
                if t == format!("<!-- /{name}:harness -->") {
                    inside = None;
                }
            }
            None => match open.captures(t) {
                Some(c) => {
                    inside = Some(c[1].to_string());
                    kept.push_str(line);
                }
                None => own.push_str(line),
            },
        }
    }
    (own, kept)
}

/// Each skill directory (`dirs`) or `<name>.md` agent under
/// `.claude/<kind>` into `.agents/<kind>`; one already there as it is stays
/// where it is (a tool's), one there with other content is refused.
fn folders(p: &Places, what: &str, kind: &str, dirs: bool, plan: &mut Plan) -> Result<()> {
    let from_dir = p.claude_dir.join(kind);
    let Ok(entries) = fs::read_dir(&from_dir) else {
        return Ok(());
    };
    let mut entries: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for from in entries {
        let name = from
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let wanted = if dirs {
            from.join("SKILL.md").is_file()
        } else {
            from.is_file() && name.ends_with(".md")
        };
        if name.starts_with('.') || !wanted {
            continue;
        }
        let to = p.agents_dir.join(kind).join(&name);
        let files = tree(&from)?;
        if to.exists() {
            if tree(&to)? == files {
                plan.left
                    .push(format!(".claude/{kind}/{name} (already in .agents/{kind})"));
                continue;
            }
            return Err(Error::Usage(format!(
                "cannot migrate: {} and {} differ; merge them by hand, then run `agent-harness-adapter install --migrate`",
                from.display(),
                to.display()
            )));
        }
        for (rel, text) in &files {
            plan.writes.push((join(&to, rel), Some(text.clone())));
            plan.writes.push((join(&from, rel), None));
        }
        if dirs {
            plan.dirs.push(from.clone());
        }
        plan.moves.push(Move {
            what: format!("{what} {}", name.trim_end_matches(".md")),
            from,
            to,
        });
    }
    Ok(())
}

fn join(base: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() {
        base.to_path_buf()
    } else {
        base.join(rel)
    }
}

/// The files of `path` (a file: itself, at `""`), relative and in order.
fn tree(path: &Path) -> Result<Vec<(String, String)>> {
    if path.is_file() {
        return Ok(vec![(String::new(), read(path)?.unwrap_or_default())]);
    }
    let mut out = Vec::new();
    let mut stack = vec![(path.to_path_buf(), String::new())];
    while let Some((dir, rel)) = stack.pop() {
        for e in fs::read_dir(&dir).map_err(|e| kit_fs_error(&dir, e))? {
            let e = e.map_err(|e| kit_fs_error(&dir, e))?;
            let at = format!("{rel}{}", e.file_name().to_string_lossy());
            if e.path().is_dir() {
                stack.push((e.path(), format!("{at}/")));
            } else {
                out.push((at, read(&e.path())?.unwrap_or_default()));
            }
        }
    }
    out.sort();
    Ok(out)
}

fn kit_fs_error(path: &Path, e: std::io::Error) -> Error {
    agent_harness_adapter_core::Error::io(path, e).into()
}

/// The groups of `hooks` in `.claude/settings.json` under the seven events,
/// made only of command entries, into the hooks file; the rest stay.
fn hooks(p: &Places, plan: &mut Plan) -> Result<()> {
    let Some(text) = read(&p.settings)? else {
        return Ok(());
    };
    let display = p.settings.display().to_string();
    let mut settings: Value = serde_json::from_str(&text)
        .map_err(|e| agent_harness_adapter_core::Error::file(&display, e.to_string()))?;
    if settings.get("permissions").is_some() {
        plan.left.push(".claude/settings.json permissions".into());
    }
    let Some(Value::Object(events)) = settings.get_mut("hooks") else {
        return Ok(());
    };
    let file = p.agents_dir.join("hooks.json");
    let mut target: Value = match read(&file)? {
        Some(t) => serde_json::from_str(&t).map_err(|e| {
            agent_harness_adapter_core::Error::file(file.display().to_string(), e.to_string())
        })?,
        None => serde_json::json!({ "hooks": {} }),
    };
    let mut moved = 0;
    for (event, groups) in events.iter_mut() {
        let Value::Array(list) = groups else { continue };
        if wire::event_from_name(event).is_none() {
            plan.left.push(format!(
                ".claude/settings.json {event} hooks (not run outside Claude Code)"
            ));
            continue;
        }
        let (go, stay): (Vec<Value>, Vec<Value>) = list.drain(..).partition(movable);
        for g in &stay {
            let why = if runs_adapter(g) {
                "installed by agent-harness-adapter or a tool"
            } else {
                "of another type than `command`"
            };
            let line = format!(".claude/settings.json {event} hooks {why}");
            if !plan.left.contains(&line) {
                plan.left.push(line);
            }
        }
        *list = stay;
        let dest = target["hooks"]
            .as_object_mut()
            .ok_or_else(|| {
                agent_harness_adapter_core::Error::file(
                    file.display().to_string(),
                    "`hooks` is not an object",
                )
            })?
            .entry(event.clone())
            .or_insert_with(|| Value::Array(Vec::new()));
        let Value::Array(dest) = dest else {
            return Err(agent_harness_adapter_core::Error::file(
                file.display().to_string(),
                format!("`hooks.{event}` is not an array"),
            )
            .into());
        };
        for g in go {
            moved += 1;
            if !dest.contains(&g) {
                dest.push(g);
            }
        }
    }
    events.retain(|_, v| !matches!(v, Value::Array(a) if a.is_empty()));
    if moved == 0 {
        return Ok(());
    }
    if events.is_empty() {
        settings.as_object_mut().map(|o| o.remove("hooks"));
    }
    plan.moves.push(Move {
        what: format!("{moved} hook group{}", if moved == 1 { "" } else { "s" }),
        from: p.settings.clone(),
        to: file.clone(),
    });
    plan.writes.push((file, Some(pretty(&target))));
    let empty = settings.as_object().is_none_or(Map::is_empty);
    plan.writes
        .push((p.settings.clone(), (!empty).then(|| pretty(&settings))));
    Ok(())
}

/// A group that is only command entries, none run by agent-harness-adapter.
fn movable(group: &Value) -> bool {
    !runs_adapter(group)
        && group["hooks"].as_array().is_some_and(|h| {
            h.iter()
                .all(|e| e.get("type").and_then(Value::as_str).unwrap_or("command") == "command")
        })
}

/// Whether an entry of `group` runs `agent-harness-adapter hook`: its own
/// dispatch entries (in the hooks file they would run themselves) and tools'
/// bridged hooks stay where they are.
fn runs_adapter(group: &Value) -> bool {
    group["hooks"].as_array().is_some_and(|h| {
        h.iter().any(|e| {
            e.get("command").and_then(Value::as_str).is_some_and(|c| {
                c.contains("agent-harness-adapter hook")
                    || c.contains(" hook --hooks ")
                    || c.contains(" hook --tool ")
            })
        })
    })
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("JSON serialises");
    s.push('\n');
    s
}

/// Refuse when, at project scope, a file the migration changes has changes
/// git has not committed (no git, or not a repository: no check).
// @zen-impl: AHA-9_AC-5
pub fn check_git(plan: &Plan, root: &Path) -> Result<()> {
    let paths: Vec<&Path> = plan.touched().into_iter().filter(|p| p.exists()).collect();
    if paths.is_empty() {
        return Ok(());
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain", "--"])
        .args(&paths)
        .output();
    match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => Err(Error::Usage(format!(
            "cannot migrate: these files have changes git has not committed (commit them, or run with --force):\n{}",
            String::from_utf8_lossy(&o.stdout).trim_end()
        ))),
        _ => Ok(()),
    }
}

/// Write the plan: at user scope, first a copy of every file it changes or
/// removes under `.agents/backup-<UTC time>/` of the home directory (its
/// path returned); then the new files, then the removals.
// @zen-impl: AHA-9_AC-6
// @zen-impl: AHA-9_AC-7
pub fn apply(plan: &Plan, scope: Scope, root: &Path) -> Result<Option<PathBuf>> {
    let backup = if scope == Scope::User {
        let dir = root.join(".agents").join(format!("backup-{}", utc_now()));
        for path in plan.touched() {
            if let (Some(text), Ok(rel)) = (read(path)?, path.strip_prefix(root)) {
                kit_fs::write_atomic(&dir.join(rel), &text)?;
            }
        }
        Some(dir)
    } else {
        None
    };
    for (path, text) in &plan.writes {
        if let Some(text) = text {
            kit_fs::write_atomic(path, text)?;
        }
    }
    for (path, text) in &plan.writes {
        if text.is_none() && path.exists() {
            fs::remove_file(path).map_err(|e| kit_fs_error(path, e))?;
        }
    }
    for dir in &plan.dirs {
        // Only directories the moves emptied.
        let _ = remove_empty(dir);
    }
    Ok(backup)
}

fn remove_empty(dir: &Path) -> std::io::Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            remove_empty(&p)?;
        }
    }
    fs::remove_dir(dir)
}

/// The time now in UTC as `YYYYMMDDTHHMMSSZ`.
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (days, rest) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests;
