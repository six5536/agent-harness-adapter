//! Harness integration, the same for every harness: the [`Harness`]
//! contract, the parts a harness renders (`file`, `region`, `merge`,
//! `external`), the state of each part, the shared-location choice, the
//! record, the declined parts, and [`install`] / [`status`] /
//! [`uninstall`] over a set of harnesses. A harness's own formats live in its module (e.g.
//! [`claude`](crate::claude)).

mod adapter;
mod declined;
mod edits;
mod file;
mod install;
mod keep;
mod meaning;
mod merge;
mod part;
mod record;
mod region;
mod result;
mod shared;
mod state;
mod status;
mod tool;
mod uninstall;
mod write;

pub use adapter::{Context, Harness, Reads, builtin, find};
pub use declined::{DeclinedStore, TomlDeclined};
pub use edits::{Edit, edits};
pub use install::{Force, InstallOptions, expand, install, installed};
pub(crate) use keep::keep_beside;
pub use keep::{Kept, keep_instructions};
pub(crate) use meaning::{digits, plain, reads_as_text};
pub(crate) use merge::parse_toml;
pub use merge::{EntryMatch, MergeOp};
pub(crate) use part::Profile;
pub use part::{ExternalPart, Part};
pub(crate) use record::read_record;
pub use region::Markers;
pub use result::{Action, HarnessResult, InstallResult, MissingSource, PartResult};
pub use state::State;
pub use status::status;
pub(crate) use tool::is_tool_name;
pub use tool::{Scope, Tool};
pub use uninstall::{UninstallOptions, uninstall};
