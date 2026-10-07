//! `ahk`'s errors: the library's, IO on its own streams, and usage
//! mistakes clap cannot see.

use std::{fmt, io};

/// What can make an `ahk` command fail.
#[derive(Debug)]
pub enum Error {
    /// From the library: a refusal, a manifest mistake, file IO.
    Kit(agent_harness_kit_core::Error),
    /// Reading stdin or writing stdout.
    Io(io::Error),
    /// A mistake in the arguments, e.g. no home directory for user scope.
    Usage(String),
}

/// `ahk`'s result.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Kit(e) => e.fmt(f),
            Error::Io(e) => e.fmt(f),
            Error::Usage(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Kit(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Usage(_) => None,
        }
    }
}

impl From<agent_harness_kit_core::Error> for Error {
    fn from(e: agent_harness_kit_core::Error) -> Self {
        Error::Kit(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
