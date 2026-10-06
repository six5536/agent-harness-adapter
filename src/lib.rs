//! agent-harness-kit: shared plumbing for CLIs that plug into LLM agent
//! harnesses (Claude Code first).
//!
//! - [`harness`]: the parts a tool installs (`file`, `region`, `merge`,
//!   `external`), their states, and [`install`] / [`status`] generic over a
//!   [`Tool`]. The same for every harness.
//! - [`claude`]: Claude Code's hook input and answers, the instructions file
//!   it reads, and the hook groups of its `settings.json`.
//! - [`LoopGuard`]: lets a stop hook block only once on the same text.
//! - [`report`]: findings (error / warning / info) and their text and JSON
//!   forms.
//! - [`cli`]: exit codes, stdout, broken pipes and the `error:` runner.
//! - [`fs`]: whole-file reads and atomic writes that keep symlinks and modes.
#![warn(missing_docs)]

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

mod error;
mod guard;
mod hash;
#[cfg(test)]
mod test_support;

pub mod claude;
pub mod cli;
pub mod fs;
pub mod harness;
pub mod report;

pub use error::{Error, Result};
pub use guard::LoopGuard;
pub use harness::{
    Action, DeclinedStore, ExternalPart, HarnessResult, InstallOptions, MergeOp, Part, PartResult,
    Profile, Scope, State, TomlDeclined, Tool, install, status,
};
pub use report::{Finding, Report, Severity};
