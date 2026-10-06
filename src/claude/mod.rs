//! Claude Code: its hook input and answers, the instructions file it reads,
//! and the hook groups of its `settings.json`. Parts and operations built
//! here are ordinary [`harness`](crate::harness) values.
// @zen-component: KIT-Claude

mod hook;
mod parts;

pub use hook::{Answer, HookInput, emit};
pub use parts::{hook_command, instructions, instructions_file};
