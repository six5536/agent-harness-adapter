//! A hook's answer, the same for every harness, and `emit`, which writes it
//! in the harness's form.

use std::{fmt::Display, io::Write};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    cli::{EXIT_ERRORS, EXIT_OK},
    harness::Harness,
    hook::Event,
};

/// What a hook decides. Each harness renders it in its own form
/// (`Harness::answer`).
///
/// As JSON (the hook contract's answer, AHA-2): `{"answer": "allow",
/// "stderr"?}`, `{"answer": "deny", "reason"}`, `{"answer": "continue",
/// "reason"}` or `{"answer": "context", "text"}`.
// @zen-impl: KIT-11_AC-4
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "lowercase")]
#[non_exhaustive]
pub enum Answer {
    /// Nothing to say; at stop, the agent may stop. `stderr` carries text
    /// for the user, e.g. a report the hook already blocked on.
    Allow {
        /// Text for stderr, when there is one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stderr: Option<String>,
    },
    /// Before a tool or a prompt: refuse it, with the reason.
    Deny {
        /// The reason, shown to the agent and the user.
        reason: String,
    },
    /// At stop: keep the agent going, with the reason as its input.
    Continue {
        /// The reason.
        reason: String,
    },
    /// Session start, prompt submit, after a tool: text added to the agent's
    /// context.
    Context {
        /// The text.
        text: String,
    },
}

impl Answer {
    /// The answer's name in an error, e.g. `deny`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Answer::Allow { .. } => "allow",
            Answer::Deny { .. } => "deny",
            Answer::Continue { .. } => "continue",
            Answer::Context { .. } => "context",
        }
    }

    /// The text for stderr, when there is one.
    pub fn stderr(&self) -> Option<&str> {
        match self {
            Answer::Allow { stderr } => stderr.as_deref(),
            _ => None,
        }
    }
}

/// What a hook command writes and its exit code, as a harness renders an
/// [`Answer`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Output {
    /// The text for stdout, without a final newline.
    pub stdout: String,
    /// The text for stderr, when there is one.
    pub stderr: Option<String>,
    /// The exit code.
    pub exit: u8,
}

impl Output {
    /// `value` as compact JSON on stdout, exit 0.
    pub fn json(value: Value) -> Self {
        Output {
            stdout: value.to_string(),
            stderr: None,
            exit: EXIT_OK,
        }
    }

    /// With `text` for stderr.
    #[must_use]
    pub fn stderr(mut self, text: Option<String>) -> Self {
        self.stderr = text;
        self
    }

    /// With exit code `exit`.
    #[must_use]
    pub fn exit(mut self, exit: u8) -> Self {
        self.exit = exit;
        self
    }
}

/// Write a hook's outcome through `harness` and return the exit code. An
/// answer: its stderr text, then the harness's stdout text and a newline,
/// and the harness's exit code. A failure, or an answer the harness cannot
/// express for `event`: `error: <message>` on stderr, nothing on stdout,
/// exit 1.
// @zen-impl: KIT-11_AC-5
pub fn emit<E: Display>(
    harness: &dyn Harness,
    event: Event,
    result: Result<Answer, E>,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> std::io::Result<u8> {
    let rendered = result
        .map_err(|e| e.to_string())
        .and_then(|a| harness.answer(event, &a).map_err(|e| e.to_string()));
    match rendered {
        Ok(out) => {
            if let Some(text) = &out.stderr {
                stderr.write_all(text.as_bytes())?;
            }
            stdout.write_all(format!("{}\n", out.stdout).as_bytes())?;
            stdout.flush()?;
            Ok(out.exit)
        }
        Err(e) => {
            writeln!(stderr, "error: {e}")?;
            Ok(EXIT_ERRORS)
        }
    }
}
