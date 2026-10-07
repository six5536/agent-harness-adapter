//! Hooks, the same for every harness: events, the input a harness sends,
//! the answer a hook gives, [`emit`], which writes it in the harness's form,
//! and [`LoopGuard`].
// @zen-component: KIT-Hook

mod answer;
mod event;
mod guard;
mod input;
pub mod wire;

pub use answer::{Answer, Output, emit};
pub use event::{Event, ToolCall, ToolKind};
pub use guard::LoopGuard;
pub use input::HookInput;
