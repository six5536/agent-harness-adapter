//! The crate's error type.
// @zen-component: KIT-Error

use std::{fmt, io, path::PathBuf};

/// Everything that can go wrong in the kit. Every variant but [`Error::Io`]
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
    /// The tool has no profile for the harness at the scope.
    UnknownProfile {
        /// The harness name given.
        harness: String,
    },
    /// A part name (e.g. in `--without`) the profile does not have.
    UnknownPart {
        /// The profile's harness.
        harness: String,
        /// The part name given.
        part: String,
    },
    /// A file the kit must read does not parse or has the wrong shape.
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
    /// A bug or an inconsistent profile.
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
            Error::UnknownProfile { harness } => write!(f, "no profile named `{harness}`"),
            Error::UnknownPart { harness, part } => {
                write!(f, "profile `{harness}` has no part named `{part}`")
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
                Error::UnknownProfile {
                    harness: "x".into(),
                },
                "no profile named `x`",
            ),
            (
                Error::UnknownPart {
                    harness: "claude".into(),
                    part: "p".into(),
                },
                "profile `claude` has no part named `p`",
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
