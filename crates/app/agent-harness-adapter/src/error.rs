//! `agent-harness-adapter`'s errors: the library's, IO on its own streams, and usage
//! mistakes clap cannot see.

use std::{fmt, io};

/// What can make an `agent-harness-adapter` command fail.
#[derive(Debug)]
pub enum Error {
    /// From the library: a refusal, a manifest mistake, file IO.
    Core(agent_harness_adapter_core::Error),
    /// Reading stdin or writing stdout.
    Io(io::Error),
    /// A mistake in the arguments, e.g. no home directory for user scope.
    Usage(String),
}

/// `agent-harness-adapter`'s result.
pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Core(e) => e.fmt(f),
            Error::Io(e) => e.fmt(f),
            Error::Usage(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Core(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Usage(_) => None,
        }
    }
}

impl From<agent_harness_adapter_core::Error> for Error {
    fn from(e: agent_harness_adapter_core::Error) -> Self {
        Error::Core(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[test]
    fn each_kind_displays_and_chains() {
        let core = Error::from(agent_harness_adapter_core::Error::Refused("no".into()));
        assert_eq!(core.to_string(), "no");
        assert!(core.source().is_some());
        let io = Error::from(io::Error::new(io::ErrorKind::BrokenPipe, "pipe"));
        assert_eq!(io.to_string(), "pipe");
        assert!(agent_harness_adapter_core::cli::is_broken_pipe(&io));
        let usage = Error::Usage("set HOME".into());
        assert_eq!(usage.to_string(), "set HOME");
        assert!(usage.source().is_none());
    }
}
