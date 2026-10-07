//! The crate's error type.
// @zen-component: KIT-Error

use std::{fmt, io, path::PathBuf};

use crate::harness::Scope;

/// Everything that can go wrong in the library. Every variant but [`Error::Io`]
/// and [`Error::Internal`] is a refusal: `install` returns it before any
/// write.
// @zen-impl: KIT-16_AC-1
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A scope name that is not `project` or `user`.
    UnknownScope {
        /// The name given.
        name: String,
    },
    /// A hook event name that is not one of [`Event`](crate::hook::Event)'s.
    UnknownEvent {
        /// The name given.
        name: String,
    },
    /// A harness id the tool does not support.
    UnknownHarness {
        /// The harness id given.
        harness: String,
    },
    /// A harness that has no files at the scope.
    UnsupportedScope {
        /// The harness id.
        harness: String,
        /// The scope given.
        scope: Scope,
    },
    /// A part name (e.g. in `--without`) no named harness's profile has.
    UnknownPart {
        /// The harness, or the harnesses named, comma-separated.
        harness: String,
        /// The part name given.
        part: String,
    },
    /// Something a harness cannot do, e.g. an answer it cannot express for
    /// an event.
    Unsupported {
        /// The harness id.
        harness: String,
        /// What it cannot do, e.g. `answer deny at stop`.
        what: String,
    },
    /// A file the library must read does not parse or has the wrong shape.
    File {
        /// The file, as the tool shows it.
        file: String,
        /// What is wrong with it.
        message: String,
    },
    /// A refusal from the tool's own [`ExternalPart`] or [`DeclinedStore`].
    ///
    /// [`ExternalPart`]: crate::harness::ExternalPart
    /// [`DeclinedStore`]: crate::harness::DeclinedStore
    Refused(String),
    /// An I/O failure on `path`.
    Io {
        /// The path being operated on.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },
    /// A bug, or an inconsistent integration or harness.
    Internal(String),
}

impl Error {
    /// An [`Error::Io`] for `path` from an [`io::Error`].
    pub fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// An [`Error::File`]: `file` (as the tool shows it) and what is wrong.
    pub fn file(file: impl Into<String>, message: impl Into<String>) -> Self {
        Error::File {
            file: file.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnknownScope { name } => write!(f, "no scope named `{name}`"),
            Error::UnknownEvent { name } => write!(f, "no hook event named `{name}`"),
            Error::UnknownHarness { harness } => write!(f, "no harness named `{harness}`"),
            Error::UnsupportedScope { harness, scope } => {
                write!(f, "harness `{harness}` has no {scope} scope")
            }
            Error::UnknownPart { harness, part } => {
                write!(f, "harness `{harness}` has no part named `{part}`")
            }
            Error::Unsupported { harness, what } => {
                write!(f, "harness `{harness}` cannot {what}")
            }
            Error::File { file, message } => write!(f, "{file}: {message}"),
            Error::Refused(message) => f.write_str(message),
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Internal(message) => write!(f, "internal: {message}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The crate's result type.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: KIT-16_AC-1
    #[test]
    fn each_kind_displays_one_line() {
        let e = Error::io(
            "/tmp/x",
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        let msg = e.to_string();
        assert!(msg.contains("/tmp/x") && msg.contains("denied"), "{msg}");
        assert!(std::error::Error::source(&e).is_some());
        let cases = [
            (
                Error::UnknownScope { name: "g".into() },
                "no scope named `g`",
            ),
            (
                Error::UnknownEvent { name: "e".into() },
                "no hook event named `e`",
            ),
            (
                Error::UnknownHarness {
                    harness: "x".into(),
                },
                "no harness named `x`",
            ),
            (
                Error::UnsupportedScope {
                    harness: "cursor".into(),
                    scope: Scope::Local,
                },
                "harness `cursor` has no local scope",
            ),
            (
                Error::UnknownPart {
                    harness: "claude".into(),
                    part: "p".into(),
                },
                "harness `claude` has no part named `p`",
            ),
            (
                Error::Unsupported {
                    harness: "copilot".into(),
                    what: "answer context at stop".into(),
                },
                "harness `copilot` cannot answer context at stop",
            ),
            (
                Error::file("a.json", "is not a JSON object"),
                "a.json: is not a JSON object",
            ),
            (Error::Refused("could not run x".into()), "could not run x"),
            (Error::Internal("bug".into()), "internal: bug"),
        ];
        for (e, text) in cases {
            assert_eq!(e.to_string(), text);
            assert!(std::error::Error::source(&e).is_none());
        }
    }
}
