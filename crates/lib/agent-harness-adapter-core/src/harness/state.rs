//! The state of a part and the hash the record keeps of its content.

use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::{
    Result,
    fs::read_text,
    harness::{
        Markers,
        merge::observe_entries,
        part::{Kind, Part},
    },
    hash::{Fnv, normalise},
};

/// The state of a part.
// @zen-impl: KIT-3_AC-1
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
    /// Equal to the tool's content.
    Current,
    /// Differs from the tool's content and equals the recorded hash: the
    /// tool wrote it and has changed since.
    Stale,
    /// Differs from both, or present with no recorded hash.
    Edited,
}

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

/// A part's content in a comparable form: what the tool wants, or what the
/// tree holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found {
    /// A `file` part's files: (relative path, text).
    Files(Vec<(String, String)>),
    /// A `region` part's block, compared by words.
    Block(String),
    /// A `merge` part's entries that are present, in operation order.
    Entries(Vec<Value>),
    /// An `external` part's text.
    Text(String),
}

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
        Kind::Region { block, .. } => Found::Block(block.clone()),
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
            Some(block) => Observed::Present(Found::Block(block)),
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

/// The words of a text: whitespace-insensitive comparison.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split_whitespace()
}

/// Whether two contents are equal: files and text line endings aside, a
/// block by its words.
// @zen-impl: KIT-3_AC-2
fn equal(a: &Found, b: &Found) -> bool {
    match (a, b) {
        (Found::Files(x), Found::Files(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|((p, s), (q, t))| p == q && normalise(s) == normalise(t))
        }
        (Found::Block(x), Found::Block(y)) => words(x).eq(words(y)),
        (Found::Entries(x), Found::Entries(y)) => x == y,
        (Found::Text(x), Found::Text(y)) => normalise(x) == normalise(y),
        _ => false,
    }
}

/// The state of a part from what the tree holds, the tool's content and the
/// recorded hash.
pub(crate) fn state(
    observed: &Observed,
    expected: &Found,
    recorded: Option<&str>,
    declined: bool,
) -> State {
    if declined {
        return State::Skipped;
    }
    match observed {
        Observed::Absent => State::Absent,
        Observed::Present(found) => {
            if equal(found, expected) {
                State::Current
            } else if recorded.is_some_and(|h| h == hash(found)) {
                State::Stale
            } else {
                State::Edited
            }
        }
    }
}

/// The FNV-1a hash of a content, as `fnv1a64:` and sixteen hex digits:
/// files with LF line endings and their paths; a block's words joined by
/// single spaces; entries as compact JSON; text with LF line endings.
pub(crate) fn hash(content: &Found) -> String {
    let mut h = Fnv::new();
    match content {
        Found::Files(files) => {
            for (path, text) in files {
                h.update(path.as_bytes());
                h.update(&[0]);
                h.update(normalise(text).as_bytes());
                h.update(&[0]);
            }
        }
        Found::Block(text) => h.update(words(text).collect::<Vec<_>>().join(" ").as_bytes()),
        Found::Entries(v) => h.update(Value::Array(v.clone()).to_string().as_bytes()),
        Found::Text(text) => h.update(normalise(text).as_bytes()),
    }
    h.render()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::hash::hash_text;

    fn block(s: &str) -> Found {
        Found::Block(s.into())
    }

    #[test]
    fn the_hash_is_fnv1a() {
        assert_eq!(hash(&block("")), "fnv1a64:cbf29ce484222325");
        assert_eq!(hash(&block("a")), "fnv1a64:af63dc4c8601ec8c");
        // A block hashes its words.
        assert_eq!(hash(&block("x\r\n  y\n")), hash(&block("x y")));
        // Files hash their paths too, so a rename changes the hash.
        let a = Found::Files(vec![("A.md".into(), "t\n".into())]);
        let b = Found::Files(vec![("B.md".into(), "t\r\n".into())]);
        assert_ne!(hash(&a), hash(&b));
        let e = Found::Entries(vec![serde_json::json!({"a": [1, 2]})]);
        assert_eq!(hash(&e), hash_text("[{\"a\":[1,2]}]"));
        assert_eq!(hash(&Found::Text("t\r\n".into())), hash_text("t\n"));
    }

    // @zen-test: KIT-3_AC-2
    #[test]
    fn a_block_is_compared_by_words() {
        let expected = block("Use the tool.\nAlways.\n");
        let reflowed = Observed::Present(block("Use  the\ntool.   Always.\r\n\n"));
        assert_eq!(state(&reflowed, &expected, None, false), State::Current);
        let other = Observed::Present(block("Use a tool. Always."));
        assert_eq!(state(&other, &expected, None, false), State::Edited);
    }

    // @zen-test: KIT-3_AC-1
    #[test]
    fn each_row_of_the_state_table() {
        let expected = block("new\n");
        let old = block("old\n");
        let recorded = hash(&old);
        assert_eq!(
            state(&Observed::Present(expected.clone()), &expected, None, true),
            State::Skipped
        );
        assert_eq!(
            state(&Observed::Absent, &expected, None, false),
            State::Absent
        );
        assert_eq!(
            state(&Observed::Present(block("new\r\n")), &expected, None, false),
            State::Current
        );
        assert_eq!(
            state(
                &Observed::Present(old.clone()),
                &expected,
                Some(&recorded),
                false
            ),
            State::Stale
        );
        assert_eq!(
            state(&Observed::Present(old.clone()), &expected, None, false),
            State::Edited
        );
        assert_eq!(
            state(
                &Observed::Present(block("other\n")),
                &expected,
                Some(&recorded),
                false
            ),
            State::Edited
        );
        assert_eq!(
            state(
                &Observed::Present(Found::Files(vec![])),
                &expected,
                None,
                false
            ),
            State::Edited
        );
        let files = Found::Files(vec![("a".into(), "x\n".into())]);
        assert_eq!(
            state(
                &Observed::Present(Found::Files(vec![("a".into(), "x\r\n".into())])),
                &files,
                None,
                false
            ),
            State::Current
        );
        assert_eq!(
            state(
                &Observed::Present(Found::Text("a\r\n".into())),
                &Found::Text("a\n".into()),
                None,
                false
            ),
            State::Current
        );
        assert_eq!(State::Stale.as_str(), "stale");
        assert_eq!(serde_json::to_string(&State::Edited).unwrap(), "\"edited\"");
    }

    fn arb_block() -> impl Strategy<Value = String> {
        prop::collection::vec("[a-zA-Z.][a-zA-Z .]{0,19}", 1..4)
            .prop_map(|lines| format!("{}\n", lines.join("\n")))
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        // @zen-test: KIT_P-3
        #[test]
        fn states_partition(found in prop::option::of(arb_block()), recorded_of_found in any::<bool>(), declined in any::<bool>()) {
            let expected = Found::Block("embedded\n".into());
            let observed = match &found {
                Some(b) => Observed::Present(Found::Block(b.clone())),
                None => Observed::Absent,
            };
            let recorded = match &found {
                Some(b) if recorded_of_found => Some(hash(&Found::Block(b.clone()))),
                _ => Some("fnv1a64:0000000000000000".to_string()),
            };
            let got = state(&observed, &expected, recorded.as_deref(), declined);
            let want = if declined {
                State::Skipped
            } else if found.is_none() {
                State::Absent
            } else if found.as_deref().unwrap().split_whitespace().eq(["embedded"]) {
                State::Current
            } else if recorded_of_found {
                State::Stale
            } else {
                State::Edited
            };
            prop_assert_eq!(got, want);
            let h = hash(&expected);
            prop_assert_eq!(state(&Observed::Present(expected.clone()), &expected, Some(&h), false), State::Current);
        }

        // @zen-test: KIT_P-4
        #[test]
        fn a_reflowed_block_is_current(block in arb_block(), seps in prop::collection::vec(prop::sample::select(vec![" ", "  ", "\n", "\r\n", "\t", " \n\n"]), 1..40)) {
            let words: Vec<&str> = block.split_whitespace().collect();
            let mut reflowed = String::new();
            for (i, w) in words.iter().enumerate() {
                reflowed.push_str(w);
                reflowed.push_str(seps[i % seps.len()]);
            }
            let expected = Found::Block(block.clone());
            let found = Found::Block(reflowed);
            prop_assert_eq!(state(&Observed::Present(found.clone()), &expected, None, false), State::Current);
            prop_assert_eq!(hash(&found), hash(&expected));
        }
    }
}
