//! agent-harness-kit: shared plumbing for CLIs that plug into LLM agent
//! harnesses (Claude Code, Codex, Gemini CLI, GitHub Copilot, Cursor,
//! Factory Droid, Pi, and any agent that reads `AGENTS.md`).
//!
//! - [`integration`]: what a tool installs, declared once without naming a
//!   harness.
//! - [`harness`]: the [`Harness`] contract, the parts a harness renders,
//!   their states, and [`install`] / [`status`] over a set of harnesses,
//!   with shared content written once.
//! - [`hook`]: hook events, the input a harness sends, the answer a hook
//!   gives, [`emit`](hook::emit), and the [`LoopGuard`](hook::LoopGuard).
//! - One module per harness: [`claude`].
//! - [`report`]: findings (error / warning / info) and their text and JSON
//!   forms.
//! - [`cli`]: exit codes, stdout, broken pipes and the `error:` runner.
//! - [`fs`]: whole-file reads and atomic writes that keep symlinks and modes.
#![warn(missing_docs)]

#[doc = include_str!("../README.md")]
#[cfg(doctest)]
pub struct ReadmeDoctests;

mod common;
mod error;
mod hash;
#[cfg(test)]
mod test_support;

pub mod claude;
pub mod cli;
pub mod fs;
pub mod harness;
pub mod hook;
pub mod integration;
pub mod report;

pub use error::{Error, Result};
pub use harness::{
    Action, DeclinedStore, EntryMatch, ExternalPart, Harness, HarnessResult, InstallOptions,
    InstallResult, MergeOp, Part, PartResult, Profile, Scope, State, TomlDeclined, Tool, install,
    status,
};
pub use integration::{Integration, Item};
pub use report::{Finding, Report, Severity};
