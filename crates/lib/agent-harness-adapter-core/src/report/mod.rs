//! Findings and the report: severities error / warning / info, the text
//! form `<path>:<line>: <level>: <message> (<authority>)` with the counts
//! line, and the JSON form.

mod collect;
mod finding;

pub use collect::Report;
pub use finding::{Finding, Severity};
