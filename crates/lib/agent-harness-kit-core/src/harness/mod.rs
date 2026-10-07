//! Harness integration, the same for every harness: the [`Harness`]
//! contract, the parts a harness renders (`file`, `region`, `merge`,
//! `external`), the state of each part, the shared-location choice, the
//! record, the declined parts, and [`install`] / [`status`] over a set of
//! harnesses. A harness's own formats live in its module (e.g.
//! [`claude`](crate::claude)).

mod adapter;
mod declined;
mod file;
mod install;
mod merge;
mod part;
mod record;
mod region;
mod result;
mod shared;
mod state;
mod tool;
mod write;

pub use adapter::{Context, Harness, Reads, builtin, find};
pub use declined::{DeclinedStore, TomlDeclined};
pub use install::{InstallOptions, expand, install, installed, status};
pub(crate) use merge::parse_toml;
pub use merge::{EntryMatch, MergeOp};
pub(crate) use part::Profile;
pub use part::{ExternalPart, Part};
pub use region::Markers;
pub use result::{Action, HarnessResult, InstallResult, PartResult};
pub use state::State;
pub use tool::{Scope, Tool};
