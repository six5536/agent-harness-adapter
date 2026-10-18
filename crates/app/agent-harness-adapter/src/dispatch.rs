//! `agent-harness-adapter hook --hooks <path> <harness> <event>`: the hooks
//! file's matching entries (Claude Code's format) run at hook time, each
//! through the shell with the hook contract's input, their answers combined
//! and given back in the harness's form (AHA-4.1).
// @zen-component: AHA-Dispatch

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use agent_harness_adapter_core::{
    Harness, fs,
    hook::{
        Answer, Event, HooksFile,
        file::{DEFAULT_TIMEOUT, Entry},
        wire,
    },
    manifest::project_dirs,
};
use regex::Regex;

use crate::bridge::{self, Ran};

/// Who notes are from on stderr.
const ME: &str = "agent-harness-adapter";

/// Set for the entries a dispatch runs: a dispatch inside one (a hooks file
/// whose entry dispatches again) would never end, so it allows instead.
const DISPATCHING: &str = "AGENT_HARNESS_ADAPTER_DISPATCHING";

/// Dispatch one hook: `hooks` is the `--hooks` path, `text` the harness's
/// input.
// @zen-impl: AHA-4.1_AC-1
// @zen-impl: AHA-4.1_AC-6
pub fn dispatch(
    hooks: &str,
    h: &dyn Harness,
    event: Event,
    text: &str,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u8> {
    let input = bridge::parse(h, event, text);
    let cwd = input
        .cwd
        .as_deref()
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let allow = |why: String| Answer::Allow {
        stderr: Some(format!("{ME}: {why}\n")),
    };
    // @zen-impl: AHA-4.1_AC-7
    if std::env::var_os(DISPATCHING).is_some() {
        let a = allow(format!("{hooks} is not dispatched from inside a dispatch"));
        return bridge::answer(h, event, ME, a, stdout, stderr);
    }
    let Some(file) = locate(hooks, &cwd, fs::home_dir().as_deref()) else {
        let a = allow(format!(
            "no hooks file {hooks} at or above {}",
            cwd.display()
        ));
        return bridge::answer(h, event, ME, a, stdout, stderr);
    };
    let parsed = match HooksFile::read(&file) {
        Ok(Some(f)) => f,
        Ok(None) => HooksFile::default(),
        Err(e) => return bridge::answer(h, event, ME, allow(e.to_string()), stdout, stderr),
    };
    // @zen-impl: AHA-4.1_AC-8
    // Named by the harness that parsed the input (the Cursor CLI runs
    // Claude Code's hooks with its own).
    let parser = agent_harness_adapter_core::harness::find(&input.harness);
    let named_by = parser.as_deref().unwrap_or(h);
    let tool = input.tool.as_ref().map(|t| named_by.claude_tool(t).name);
    let mut entries = Vec::new();
    for group in parsed.groups(event) {
        match matches(group.matcher.as_deref(), tool.as_deref()) {
            Ok(true) => entries.extend(group.hooks.iter().filter(|e| e.dispatched())),
            Ok(false) => {}
            Err(e) => writeln!(
                stderr,
                "{ME}: matcher `{}` is not a regular expression: {e}",
                group.matcher.as_deref().unwrap_or_default()
            )?,
        }
    }
    let project = project_dir(hooks, &file, &cwd);
    let json = bridge::stdin_text(h, &input);
    let run = |e: &Entry| run_entry(e, &json, &cwd, &project);
    if h.id() == "claude" && entries.len() == 1 {
        // @zen-impl: AHA-4.1_AC-4
        return match run(entries[0]) {
            Ok(ran) => bridge::pass_through(&ran, stdout, stderr),
            Err(why) => {
                writeln!(stderr, "{ME}: `{}`: {why}", command(entries[0]))?;
                Ok(0)
            }
        };
    }
    let answer = combine(event, h.id(), &entries, run, stderr)?;
    if h.id() == "claude" {
        if let Answer::Allow { stderr: Some(text) } = &answer {
            stderr.write_all(text.as_bytes())?;
        }
        let json = wire::answer_json(event, &answer).unwrap_or_else(|_| serde_json::json!({}));
        writeln!(stdout, "{json}")?;
        stdout.flush()?;
        return Ok(0);
    }
    bridge::answer(h, event, ME, answer, stdout, stderr)
}

/// The answer of `entries` in order: the first deny or continue ends it;
/// else their contexts joined by a blank line; else allow, with every
/// entry's stderr.
// @zen-impl: AHA-4.1_AC-3
fn combine(
    event: Event,
    harness: &str,
    entries: &[&Entry],
    mut run: impl FnMut(&Entry) -> Result<Ran, String>,
    stderr: &mut impl Write,
) -> io::Result<Answer> {
    let mut contexts = Vec::new();
    let mut notes = String::new();
    for e in entries {
        let ran = match run(e) {
            Ok(ran) => ran,
            Err(why) => {
                notes.push_str(&format!("{ME}: `{}`: {why}\n", command(e)));
                continue;
            }
        };
        let reply = bridge::read(event, command(e), &ran);
        for d in &reply.dropped {
            writeln!(stderr, "{}: {d} is not supported by {harness}", command(e))?;
        }
        match reply.answer {
            a @ (Answer::Deny { .. } | Answer::Continue { .. }) => {
                stderr.write_all(notes.as_bytes())?;
                return Ok(a);
            }
            Answer::Context { text } => contexts.push(text),
            Answer::Allow { stderr: Some(text) } => notes.push_str(&text),
            _ => {}
        }
    }
    Ok(if contexts.is_empty() {
        Answer::Allow {
            stderr: (!notes.is_empty()).then_some(notes),
        }
    } else {
        stderr.write_all(notes.as_bytes())?;
        Answer::Context {
            text: contexts.join("\n\n"),
        }
    })
}

/// Whether a group's `matcher` matches the Claude Code tool name: absent,
/// empty or `*` match every call, as does any matcher for an event without
/// a tool call; else a regular expression matching the whole name.
fn matches(matcher: Option<&str>, tool: Option<&str>) -> Result<bool, regex::Error> {
    let (Some(m), Some(tool)) = (matcher.filter(|m| !m.is_empty() && *m != "*"), tool) else {
        return Ok(true);
    };
    Ok(Regex::new(&format!("^(?:{m})$"))?.is_match(tool))
}

/// The hooks file `hooks` names: `~/…` under the home directory, an
/// absolute path as it is, a relative one in the project: from `cwd` up to
/// the repository's root, or (outside git) to the first directory that has
/// the path's first part (e.g. `.agents`); never at or above the home
/// directory, so a file outside the project is never run.
// @zen-impl: AHA-4.1_AC-5
fn locate(hooks: &str, cwd: &Path, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(rest) = hooks.strip_prefix("~/") {
        return Some(home?.join(rest)).filter(|p| p.is_file());
    }
    let path = Path::new(hooks);
    if path.is_absolute() {
        return Some(path.to_path_buf()).filter(|p| p.is_file());
    }
    let first = path
        .components()
        .next()?
        .as_os_str()
        .to_string_lossy()
        .into_owned();
    project_dirs(cwd, home, &first)
        .into_iter()
        .map(|d| d.join(path))
        .find(|p| p.is_file())
}

/// The project directory hooks see as `CLAUDE_PROJECT_DIR`: the directory
/// holding the hooks file's `.agents` (where a relative `hooks` was found);
/// the hook's working directory for a user's hooks file.
fn project_dir(hooks: &str, file: &Path, cwd: &Path) -> PathBuf {
    if hooks.starts_with("~/") || Path::new(hooks).is_absolute() {
        return cwd.to_path_buf();
    }
    let depth = Path::new(hooks).components().count();
    file.ancestors()
        .nth(depth)
        .map_or_else(|| cwd.to_path_buf(), Path::to_path_buf)
}

/// Run one entry through the shell, `input` on its stdin, in `cwd`, with
/// `CLAUDE_PROJECT_DIR`, stopped after its timeout.
// @zen-impl: AHA-4.1_AC-2
fn run_entry(e: &Entry, input: &str, cwd: &Path, project: &Path) -> Result<Ran, String> {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c");
        c
    };
    cmd.arg(command(e))
        .current_dir(cwd)
        .env("CLAUDE_PROJECT_DIR", project)
        .env(DISPATCHING, "1");
    let timeout = Duration::from_secs(e.timeout.unwrap_or(DEFAULT_TIMEOUT));
    bridge::run_child(&mut cmd, input, Some(timeout))
}

fn command(e: &Entry) -> &str {
    e.command.as_deref().unwrap_or_default()
}

#[cfg(test)]
mod tests;
