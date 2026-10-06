//! `Finding`: one line of a report.

use serde::{Deserialize, Serialize};

/// How a finding is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Severity {
    /// A failure. Exit 1.
    Error,
    /// What the tool cannot settle alone.
    Warning,
    /// Worth knowing; never a failure.
    Info,
}

impl Severity {
    /// The level as the text report shows it.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// One line of the report. Built with [`Finding::error`], [`Finding::warning`]
/// or [`Finding::info`], then [`Finding::line`] and [`Finding::authority`].
// @zen-impl: KIT-13_AC-1
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Finding {
    /// The file the finding is on, as the tool shows paths.
    pub path: String,
    /// The line, when one applies.
    pub line: Option<usize>,
    /// Error, warning or info.
    pub severity: Severity,
    /// The message.
    pub message: String,
    /// What the finding breaks, e.g. `CFG-3` or `spec §8`; shown in
    /// parentheses after the message.
    pub authority: Option<String>,
}

impl Finding {
    /// A finding of `severity` on `path`, with no line and no authority.
    pub fn new(severity: Severity, path: impl Into<String>, message: impl Into<String>) -> Self {
        Finding {
            path: path.into(),
            line: None,
            severity,
            message: message.into(),
            authority: None,
        }
    }

    /// An error finding.
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, path, message)
    }

    /// A warning finding.
    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, path, message)
    }

    /// An info finding.
    pub fn info(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Info, path, message)
    }

    /// The finding on `line`.
    #[must_use]
    pub fn line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// The finding with what it breaks.
    #[must_use]
    pub fn authority(mut self, authority: impl Into<String>) -> Self {
        self.authority = Some(authority.into());
        self
    }

    /// The message with its authority: `<message> (<authority>)`.
    pub fn full_message(&self) -> String {
        match &self.authority {
            Some(a) => format!("{} ({a})", self.message),
            None => self.message.clone(),
        }
    }

    /// The text line: `<path>:<line>: <level>: <message> (<authority>)`,
    /// without `:<line>` when there is none.
    pub fn to_line(&self) -> String {
        let level = self.severity.as_str();
        match self.line {
            Some(line) => format!("{}:{line}: {level}: {}", self.path, self.full_message()),
            None => format!("{}: {level}: {}", self.path, self.full_message()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: KIT-13_AC-1
    // @zen-test: KIT-13_AC-3
    #[test]
    fn the_line_carries_the_level_and_authority() {
        let f = Finding::error("a.yaml", "bad").line(3).authority("CFG-1");
        assert_eq!(f.to_line(), "a.yaml:3: error: bad (CFG-1)");
        let f = Finding::warning("a.yaml", "odd");
        assert_eq!(f.to_line(), "a.yaml: warning: odd");
        let f = Finding::info("c.toml", "project wins")
            .line(1)
            .authority("CLI-3");
        assert_eq!(f.severity, Severity::Info);
        assert_eq!(f.to_line(), "c.toml:1: info: project wins (CLI-3)");
    }

    #[test]
    fn findings_round_trip_through_serde() {
        let f = Finding::error("a.md", "bad").line(3).authority("spec §8");
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"severity\":\"error\""), "{json}");
        let back: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(back, f);
    }
}
