//! `ahk hook`: a harness's hook, bridged to a command that speaks the hook
//! contract (AHK-2) on stdin and stdout.
// @zen-component: AHK-Bridge

use std::{
    ffi::{OsStr, OsString},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
};

use agent_harness_kit_core::{
    Harness,
    cli::EXIT_ERRORS,
    harness,
    hook::{Answer, Event, HookInput, emit, wire},
};
use clap::Args;
use serde_json::Value;

/// `ahk hook`.
#[derive(Debug, Args)]
pub struct HookArgs {
    /// The tool's name in messages (default: the command's name).
    #[arg(long, value_name = "NAME")]
    pub tool: Option<String>,
    /// The harness that runs the hook, e.g. claude.
    pub harness: String,
    /// The event, e.g. pre-tool.
    pub event: String,
    /// The command and its arguments, after `--`.
    #[arg(last = true, required = true, value_name = "COMMAND")]
    pub command: Vec<OsString>,
}

/// Bridge one hook run: the process's stdin to the command and its answer
/// back through the harness. Returns the exit code.
pub fn run(args: &HookArgs) -> io::Result<u8> {
    let mut input = String::new();
    // A harness that sends nothing, or something unreadable, gets the
    // input with no fields from it (AHK-4_AC-3).
    let _ = io::stdin().read_to_string(&mut input);
    bridge(
        args,
        &input,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

// @zen-impl: AHK-4_AC-1
// @zen-impl: AHK-4_AC-4
fn bridge(
    args: &HookArgs,
    text: &str,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> io::Result<u8> {
    let Some(h) = harness::find(&args.harness) else {
        writeln!(stderr, "error: no harness named `{}`", args.harness)?;
        return Ok(EXIT_ERRORS);
    };
    let event: Event = match args.event.parse() {
        Ok(e) => e,
        Err(e) => {
            writeln!(stderr, "error: {e}")?;
            return Ok(EXIT_ERRORS);
        }
    };
    let input = parse(h.as_ref(), event, text);
    let tool = args.tool.clone().unwrap_or_else(|| name(&args.command[0]));
    let answer = ask(&args.command, &input).unwrap_or_else(|reason| Answer::Allow {
        stderr: Some(format!("{tool}: {reason}\n")),
    });
    emit(
        h.as_ref(),
        event,
        Ok::<_, std::convert::Infallible>(answer),
        stdout,
        stderr,
    )
}

/// The harness's input; with only `harness` and `event` when it is not JSON.
// @zen-impl: AHK-4_AC-3
fn parse(h: &dyn Harness, event: Event, text: &str) -> HookInput {
    HookInput::parse(h, event, text).unwrap_or_else(|_| HookInput::new(h.id(), event, Value::Null))
}

/// The command's answer to `input`, or why there is none.
// @zen-impl: AHK-4_AC-2
fn ask(command: &[OsString], input: &HookInput) -> Result<Answer, String> {
    let program = &command[0];
    let mut child = Command::new(resolve(program))
        .args(&command[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("could not run `{}`: {e}", program.to_string_lossy()))?;
    let mut json = wire::input_json(input).to_string();
    json.push('\n');
    let mut stdin = child.stdin.take().expect("piped");
    // Written beside the read, so a command that answers before it reads
    // all its input cannot dead-lock on a full pipe.
    let writer = thread::spawn(move || {
        let _ = stdin.write_all(json.as_bytes());
    });
    let out = child
        .wait_with_output()
        .map_err(|e| format!("could not run `{}`: {e}", program.to_string_lossy()))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(match out.status.code() {
            Some(code) => format!("exited with {code}"),
            None => "was killed".into(),
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    wire::parse_answer(&text).map_err(|e| e.to_string())
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
        let dir = std::env::temp_dir().join(format!("ahk-resolve-{}", std::process::id()));
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
