//! Claude Code: [`Claude`], the harness, and [`instructions_file`], its
//! rule for the instructions file it reads.

mod adapter;

pub(crate) use adapter::tool_kind;
pub use adapter::{Claude, instructions_file};
