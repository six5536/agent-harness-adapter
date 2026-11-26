//! The state of a part, from its content compared by meaning with the
//! tool's, and the value that the record keeps of a content.

use std::{collections::BTreeMap, path::Path};

use serde::Serialize;
use serde_json::Value;

use crate::{
    Result,
    fs::read_text,
    harness::{
        Markers, meaning,
        meaning::{Unusable, Vouch},
        merge::observe_entries,
        part::{Kind, Part},
        record::Recorded,
    },
};

// sokf:begin state
/// The state of a part.
// sokf-implements: KIT-3_AC-1
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum State {
    /// Declined by the user.
    Skipped,
    /// Another harness's part puts the same content in a location this
    /// harness loads; nothing is written for it.
    Shared,
    /// The file, the block or the entries are not there.
    Absent,
    /// The same as the tool's content, by its meaning.
    Current,
    /// Differs from the tool's content and is what the record holds: the
    /// tool wrote it and has changed since.
    Stale,
    /// Differs from both, or present with no record that can vouch for it.
    Edited,
}
// sokf:end state

impl State {
    /// The state's name in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            State::Skipped => "skipped",
            State::Shared => "shared",
            State::Absent => "absent",
            State::Current => "current",
            State::Stale => "stale",
            State::Edited => "edited",
        }
    }
}

// sokf:begin found
/// A part's content in a comparable form: what the tool wants, or what the
/// tree holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found {
    /// A `file` part's files: (relative path, text).
    Files(Vec<(String, String)>),
    /// A `region` part's block, compared as Markdown named as its file.
    Block {
        /// The file that holds the block, relative to the root.
        file: String,
        /// The block, LF.
        text: String,
    },
    /// A `merge` part's entries that are present, in operation order.
    Entries(Vec<Value>),
    /// An `external` part's text.
    Text(String),
}
// sokf:end found

/// What the tree holds for a part.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Observed {
    /// Nothing the tool could own.
    Absent,
    /// The part's content as found.
    Present(Found),
}

/// The content the tool wants for `part`.
pub(crate) fn expected(part: &Part) -> Found {
    match &part.kind {
        Kind::Files { files, .. } => Found::Files(files.clone()),
        // The block as the region holds it, so that a file of no format,
        // compared by its bytes, finds the block that it wrote.
        Kind::Region { file, block } => Found::Block {
            file: file.clone(),
            text: Markers::body(block),
        },
        // An op that owns entries but has none (e.g. a hook event the tool no
        // longer uses) expects nothing: its old entries make the part stale.
        Kind::Merge { ops, .. } => Found::Entries(
            ops.iter()
                .filter(|op| !op.expects_nothing())
                .map(|op| op.value().clone())
                .collect(),
        ),
        Kind::External(ext) => Found::Text(ext.expected()),
    }
}

/// Read a part's content at `path` (relative to `root`), through its kind.
/// A `merge` file that does not parse is a refusal.
pub(crate) fn observe(root: &Path, part: &Part, path: &str, markers: &Markers) -> Result<Observed> {
    let fs_path = root.join(path);
    Ok(match &part.kind {
        // The files that exist: a part with some of them missing differs
        // from the tool's content, so an edited file among the rest is never
        // rewritten without `--force`; absent only when none exists.
        Kind::Files { files, .. } => {
            let mut found = Vec::new();
            for (rel, _) in files {
                if let Some(text) = read_text(&fs_path.join(rel))? {
                    found.push((rel.clone(), text));
                }
            }
            if found.is_empty() {
                Observed::Absent
            } else {
                Observed::Present(Found::Files(found))
            }
        }
        Kind::Region { .. } => match read_text(&fs_path)?.and_then(|t| markers.find(&t)) {
            Some(text) => Observed::Present(Found::Block {
                file: path.to_string(),
                text,
            }),
            None => Observed::Absent,
        },
        Kind::Merge { ops, .. } => match read_text(&fs_path)? {
            Some(text) => {
                let entries = observe_entries(path, &text, ops)?;
                if entries.is_empty() {
                    Observed::Absent
                } else {
                    Observed::Present(Found::Entries(entries))
                }
            }
            None => Observed::Absent,
        },
        Kind::External(ext) => match ext.observe()? {
            Some(text) => Observed::Present(Found::Text(text)),
            None => Observed::Absent,
        },
    })
}

/// A content's fingerprints: one, or one for each file of a file part.
enum Prints {
    One(same_same::Fingerprint),
    Files(Vec<(String, same_same::Fingerprint)>),
}

/// A merge part's entries as compact JSON with the keys of each object
/// sorted, whose plain hash the record keeps.
fn entries_text(entries: &[Value]) -> String {
    let mut value = Value::Array(entries.to_vec());
    value.sort_all_objects();
    value.to_string()
}

/// The fingerprints of a content: a file's by its name, a block's as
/// Markdown of its own, an external text's by its bytes, and the plain hash
/// of a merge part's entries.
fn prints(content: &Found) -> Prints {
    match content {
        Found::Files(files) => Prints::Files(
            files
                .iter()
                .map(|(path, text)| (path.clone(), meaning::file(path, text)))
                .collect(),
        ),
        Found::Block { file, text } => Prints::One(meaning::block(file, text)),
        Found::Entries(entries) => Prints::One(meaning::plain(entries_text(entries).as_bytes())),
        Found::Text(text) => Prints::One(meaning::text(text)),
    }
}

/// Whether two contents mean the same: each file of a file part by its
/// path and meaning, a block and an external text by their meaning, and
/// entries by their value.
// sokf-implements: KIT-3_AC-2
fn equal(a: &Found, prints_a: &Prints, b: &Found) -> bool {
    match (a, prints_a, b) {
        (Found::Entries(x), _, Found::Entries(y)) => x == y,
        (Found::Files(_), Prints::Files(x), Found::Files(_)) => {
            let Prints::Files(y) = prints(b) else {
                return false;
            };
            x.len() == y.len()
                && x.iter()
                    .zip(&y)
                    .all(|((p, s), (q, t))| p == q && meaning::same(s, t))
        }
        (Found::Block { .. }, Prints::One(x), Found::Block { .. })
        | (Found::Text(_), Prints::One(x), Found::Text(_)) => {
            let Prints::One(y) = prints(b) else {
                return false;
            };
            meaning::same(x, &y)
        }
        _ => false,
    }
}

/// What the record says of a content found, by its fingerprints: each file
/// of a file part that the record holds and the tool still wants is found,
/// each file found is recorded and is what its recorded value holds, and a
/// recorded file that the tool no longer wants is left out; any other
/// part's content is what its one value holds.
fn vouch(recorded: &Recorded, found: &Prints, expected: &Found) -> Vouch {
    match (recorded, found, expected) {
        (Recorded::One(value), Prints::One(print), _) => {
            let wanted = match prints(expected) {
                Prints::One(w) => Some(w),
                Prints::Files(_) => None,
            };
            meaning::vouch(value, print, wanted.as_ref())
        }
        (Recorded::Files(values), Prints::Files(prints), Found::Files(wanted)) => {
            let missing = wanted.iter().any(|(path, _)| {
                values.contains_key(path) && !prints.iter().any(|(p, _)| p == path)
            });
            if missing || prints.iter().any(|(p, _)| !values.contains_key(p)) {
                return Vouch::Differs;
            }
            let vouches: Vec<Vouch> = prints
                .iter()
                .map(|(path, print)| {
                    let wanted = wanted
                        .iter()
                        .find(|(p, _)| p == path)
                        .map(|(p, t)| meaning::file(p, t));
                    meaning::vouch(&values[path], print, wanted.as_ref())
                })
                .collect();
            if vouches.contains(&Vouch::Differs) {
                Vouch::Differs
            } else {
                vouches
                    .into_iter()
                    .find(|v| matches!(v, Vouch::Unusable(_)))
                    .unwrap_or(Vouch::Holds)
            }
        }
        // A string of 0.4.0's form for a file part is a hash of 0.4.0; any
        // other value of the wrong shape is one of another form.
        (Recorded::One(value), Prints::Files(_), _)
            if value.parse::<same_same::Fingerprint>().is_err() =>
        {
            Vouch::Unusable(Unusable::Old)
        }
        _ => Vouch::Unusable(Unusable::Shape),
    }
}

/// The state of a part from what the tree holds, the tool's content and the
/// recorded value, with why the record could not tell an edit from the
/// tool's earlier content where that made the part edited.
// sokf-implements: KIT-3_AC-10
pub(crate) fn state(
    observed: &Observed,
    expected: &Found,
    recorded: Option<&Recorded>,
    declined: bool,
) -> (State, Option<Unusable>) {
    if declined {
        return (State::Skipped, None);
    }
    let Observed::Present(found) = observed else {
        return (State::Absent, None);
    };
    let found_prints = prints(found);
    if equal(found, &found_prints, expected) {
        return (State::Current, None);
    }
    match recorded.map(|r| vouch(r, &found_prints, expected)) {
        Some(Vouch::Holds) => (State::Stale, None),
        Some(Vouch::Unusable(why)) => (State::Edited, Some(why)),
        Some(Vouch::Differs) | None => (State::Edited, None),
    }
}

/// The value that the record keeps of a content: the fingerprint of a
/// region's block or an external text, the plain hash of a merge part's
/// entries, or a table of each file's fingerprint by its path in the part,
/// each in `same-same`'s text form.
// sokf-implements: KIT-3_AC-9
pub(crate) fn hash(content: &Found) -> Recorded {
    match prints(content) {
        Prints::One(print) => Recorded::One(print.to_string()),
        Prints::Files(files) => Recorded::Files(
            files
                .into_iter()
                .map(|(path, print)| (path, print.to_string()))
                .collect::<BTreeMap<_, _>>(),
        ),
    }
}

#[cfg(test)]
mod tests;
