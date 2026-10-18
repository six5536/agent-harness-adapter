//! `agent-harness-adapter hook`: a harness's hook, bridged to a command that
//! speaks the hook contract (AHA-2), Claude Code's protocol: Claude Code's
//! input on stdin, its answer as exit code, stdout and stderr. With
//! `--hooks`, the hooks file's entries are dispatched instead (dispatch.rs).
// @zen-component: AHA-Bridge

use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use agent_harness_adapter_core::{
    Harness,
    cli::EXIT_ERRORS,
    harness,
    hook::{Answer, Event, HookInput, ToolKind, emit, wire},
};
use clap::Args;
use serde_json::Value;

use crate::dispatch;

/// `agent-harness-adapter hook`.
#[derive(Debug, Args)]
pub struct HookArgs {
    /// The tool's name in messages (default: the command's name).
    #[arg(long, value_name = "NAME")]
    pub tool: Option<String>,
    /// Run the command only for tool calls of this kind (shell, read,
    /// write, mcp, other); allow the others without running it.
    #[arg(long, value_name = "KIND", value_parser = parse_kind)]
    pub tools: Option<ToolKind>,
    /// Run the matching entries of this hooks file (Claude Code's format)
    /// instead of a command: relative paths are looked for upwards from the
    /// hook's working directory, `~/` is the home directory.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["tool", "tools"])]
    pub hooks: Option<String>,
    /// The harness that runs the hook, e.g. claude.
    pub harness: String,
    /// The event, e.g. pre-tool.
    pub event: String,
    /// The command and its arguments, after `--`.
    #[arg(last = true, required_unless_present = "hooks", value_name = "COMMAND")]
    pub command: Vec<OsString>,
}

/// Bridge or dispatch one hook run: the process's stdin to the command(s)
/// and the answer back through the harness. Returns the exit code.
pub fn run(args: &HookArgs) -> io::Result<u8> {
    let mut input = String::new();
    // A harness that sends nothing, or something unreadable, gets the
    // input with no fields from it (AHA-4_AC-3).
    let _ = io::stdin().read_to_string(&mut input);
    let (mut out, mut err) = (io::stdout().lock(), io::stderr().lock());
    let Some((h, event)) = target(args, &mut err)? else {
        return Ok(EXIT_ERRORS);
    };
    match &args.hooks {
        Some(hooks) => dispatch::dispatch(hooks, h.as_ref(), event, &input, &mut out, &mut err),
        None => bridge(args, h.as_ref(), event, &input, &mut out, &mut err),
    }
}

/// The harness and event named, or `error:` on stderr when either is
/// unknown.
// @zen-impl: AHA-4_AC-4
fn target(
    args: &HookArgs,
    stderr: &mut impl Write,
) -> io::Result<Option<(std::sync::Arc<dyn Harness>, Event)>> {
    let Some(h) = harness::find(&args.harness) else {
        writeln!(stderr, "error: no harness named `{}`", args.harness)?;
        return Ok(None);
    };
    match args.event.parse() {
        Ok(e) => Ok(Some((h, e))),
        Err(e) => {
            writeln!(stderr, "error: {e}")?;
            Ok(None)
        }
    }
}

// @zen-impl: AHA-4_AC-1
fn bridge(
    args: &HookArgs,
    h: &dyn Harness,
    event: Event,
    text: &str,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u8> {
    let input = parse(h, event, text);
    let tool = args.tool.clone().unwrap_or_else(|| name(&args.command[0]));
    if other_kind(args.tools, &input) {
        return answer(
            h,
            event,
            &tool,
            Answer::Allow { stderr: None },
            stdout,
            stderr,
        );
    }
    let program = &args.command[0];
    let mut cmd = Command::new(resolve(program));
    cmd.args(&args.command[1..]);
    match run_child(&mut cmd, &stdin_text(h, &input), None) {
        Err(reason) => {
            let reason = format!(
                "{tool}: could not run `{}`: {reason}\n",
                program.to_string_lossy()
            );
            answer(
                h,
                event,
                &tool,
                Answer::Allow {
                    stderr: Some(reason),
                },
                stdout,
                stderr,
            )
        }
        Ok(ran) if h.id() == "claude" => pass_through(&ran, stdout, stderr),
        Ok(ran) => {
            let reply = read(event, &tool, &ran);
            for d in &reply.dropped {
                writeln!(stderr, "{tool}: {d} is not supported by {}", h.id())?;
            }
            answer(h, event, &tool, reply.answer, stdout, stderr)
        }
    }
}

/// The contract's input for `input`, as one JSON line.
pub(crate) fn stdin_text(h: &dyn Harness, input: &HookInput) -> String {
    let mut json = wire::input_json(h, input).to_string();
    json.push('\n');
    json
}

/// A command's answer as the contract reads it; a failing command with
/// nothing on stderr is named, so the user sees why it allowed.
pub(crate) fn read(event: Event, tool: &str, ran: &Ran) -> wire::Reply {
    let mut reply = wire::parse_answer(event, ran.exit, &ran.stdout, &ran.stderr);
    if !matches!(ran.exit, 0 | 2) && ran.stderr.trim().is_empty() {
        if let Answer::Allow { stderr } = &mut reply.answer {
            *stderr = Some(format!("{tool}: exited with {}\n", ran.exit));
        }
    }
    reply
}

/// The command's output as it wrote it, for Claude Code (AHA-2_AC-5).
// @zen-impl: AHA-2_AC-5
pub(crate) fn pass_through(
    ran: &Ran,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u8> {
    stderr.write_all(ran.stderr.as_bytes())?;
    stdout.write_all(ran.stdout.as_bytes())?;
    stdout.flush()?;
    Ok(u8::try_from(ran.exit).unwrap_or(1))
}

/// `answer` emitted through the harness; one it cannot express for the
/// event is left out, with a note, and allows (AHA-2_AC-4).
// @zen-impl: AHA-2_AC-4
pub(crate) fn answer(
    h: &dyn Harness,
    event: Event,
    tool: &str,
    answer: Answer,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u8> {
    let answer = if h.answer(event, &answer).is_err() {
        writeln!(
            stderr,
            "{tool}: {} at {event} is not supported by {}",
            answer.as_str(),
            h.id()
        )?;
        Answer::Allow { stderr: None }
    } else {
        answer
    };
    emit(
        h,
        event,
        Ok::<_, std::convert::Infallible>(answer),
        stdout,
        stderr,
    )
}

/// Whether `input` is a tool call of another kind than `tools`.
// @zen-impl: AHA-4_AC-5
fn other_kind(tools: Option<ToolKind>, input: &HookInput) -> bool {
    matches!((tools, &input.tool), (Some(k), Some(t)) if t.kind != k)
}

/// `--tools`'s value: a tool kind's name.
fn parse_kind(s: &str) -> Result<ToolKind, String> {
    serde_json::from_value(Value::String(s.into()))
        .map_err(|_| format!("no tool kind `{s}`: shell, read, write, mcp or other"))
}

/// The harness's input; with only `harness` and `event` when it is not JSON.
// @zen-impl: AHA-4_AC-3
pub(crate) fn parse(h: &dyn Harness, event: Event, text: &str) -> HookInput {
    HookInput::parse(h, event, text).unwrap_or_else(|_| HookInput::new(h.id(), event, Value::Null))
}

/// What a command did: its exit code (1 when killed) and its output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ran {
    pub exit: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Run `cmd` with `input` on its stdin; past `timeout`, kill it.
// @zen-impl: AHA-4_AC-2
pub(crate) fn run_child(
    cmd: &mut Command,
    input: &str,
    timeout: Option<Duration>,
) -> Result<Ran, String> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().expect("piped");
    let input = input.to_string();
    // Written and read beside each other, so a command that answers before
    // it reads all its input cannot dead-lock on a full pipe.
    let writer = thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let drain = |mut r: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            String::from_utf8_lossy(&buf).into_owned()
        })
    };
    let out = drain(Box::new(child.stdout.take().expect("piped")));
    let err = drain(Box::new(child.stderr.take().expect("piped")));
    let deadline = timeout.map(|t| Instant::now() + t);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "timed out after {}s",
                timeout.unwrap_or_default().as_secs()
            ));
        }
        thread::sleep(Duration::from_millis(5));
    };
    let _ = writer.join();
    Ok(Ran {
        exit: status.code().unwrap_or(1),
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

/// The command's name, for messages: its file name without extension.
fn name(program: &OsString) -> String {
    Path::new(program)
        .file_stem()
        .map_or_else(|| program.to_string_lossy(), |s| s.to_string_lossy())
        .into_owned()
}

/// The program to start. On Windows a bare name is looked up on `PATH` with
/// each `PATHEXT` extension, so an npm `.cmd` shim runs (`Command` alone
/// finds only `.exe`); elsewhere it is used as given.
fn resolve(program: &OsString) -> PathBuf {
    let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    let path = std::env::var_os("PATH").unwrap_or_default();
    resolve_in(program, cfg!(windows), &path, &exts)
}

/// [`resolve`] with its environment given.
fn resolve_in(program: &OsString, windows: bool, path: &OsStr, exts: &str) -> PathBuf {
    let given = PathBuf::from(program);
    if !windows || given.extension().is_some() || given.components().count() > 1 {
        return given;
    }
    let base = given.display().to_string();
    std::env::split_paths(path)
        .flat_map(|dir| {
            exts.split(';')
                .filter(|e| !e.is_empty())
                .map(|e| dir.join(format!("{base}{e}")))
                .collect::<Vec<_>>()
        })
        .find(|p| p.is_file())
        .unwrap_or(given)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_programs() {
        assert_eq!(name(&"/usr/bin/mytool".into()), "mytool");
        assert_eq!(name(&"mytool.cmd".into()), "mytool");
        if !cfg!(windows) {
            assert_eq!(resolve(&"mytool".into()), PathBuf::from("mytool"));
        }
    }

    #[test]
    fn windows_finds_a_bare_name_with_its_extension_on_the_path() {
        let dir = std::env::temp_dir().join(format!("aha-resolve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mytool.CMD"), "").unwrap();
        let path = std::env::join_paths([dir.join("none"), dir.clone()]).unwrap();
        let exts = ".EXE;.CMD;";
        let found = |p: &str| resolve_in(&p.into(), true, &path, exts);
        assert_eq!(found("mytool"), dir.join("mytool.CMD"));
        // A name with an extension or a directory, or not found: as given.
        assert_eq!(found("mytool.exe"), PathBuf::from("mytool.exe"));
        assert_eq!(found("bin/mytool"), PathBuf::from("bin/mytool"));
        assert_eq!(found("other"), PathBuf::from("other"));
        assert_eq!(
            resolve_in(&"mytool".into(), false, &path, exts),
            PathBuf::from("mytool")
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
