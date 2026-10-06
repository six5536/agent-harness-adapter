//! Harness integration, the same for every harness: profiles of parts the
//! tool supplies, the state of each part, the write kinds (`file`,
//! `region`, `merge`, `external`), the record, the declined parts, and
//! [`install`] / [`status`]. A harness's own formats live in its module
//! (e.g. [`claude`](crate::claude)).

mod declined;
mod file;
mod install;
mod merge;
mod part;
mod record;
mod region;
mod state;
mod target;
mod tool;
mod write;

pub use declined::{DeclinedStore, TomlDeclined};
pub use install::{Action, HarnessResult, InstallOptions, PartResult, install, status};
pub use merge::MergeOp;
pub use part::{ChooseFile, ExternalPart, Part, Profile};
pub use region::Markers;
pub use state::State;
pub use tool::{Scope, Tool};
