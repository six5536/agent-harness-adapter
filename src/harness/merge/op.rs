//! The merge operations: which entries of a settings file the tool owns.

use serde_json::Value;

/// Entries the tool owns in a JSON or TOML file, built by
/// [`MergeOp::array_entry`], [`MergeOp::object_member`],
/// [`MergeOp::owned_entries`] or [`MergeOp::group_entries`].
// @zen-impl: KIT-2_AC-3
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeOp(pub(crate) Op);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Op {
    ArrayEntry {
        path: Vec<String>,
        value: Value,
    },
    ObjectMember {
        path: Vec<String>,
        key: String,
        value: Value,
    },
    OwnedEntries {
        path: Vec<String>,
        field: String,
        owned: EntryMatch,
        entries: Value,
    },
    GroupEntries {
        path: Vec<String>,
        entries: String,
        field: String,
        owned: EntryMatch,
        groups: Value,
    },
}

/// Which entries are the tool's: those whose owning field matches. See
/// [`MergeOp::owned_entries`] and [`MergeOp::group_entries`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntryMatch {
    /// The field starts with the text, e.g. `mytool hook `.
    Prefix(String),
    /// The field contains the text, e.g. ` hook `, for a tool whose program
    /// users may rename or call by path.
    Contains(String),
    /// Any of these matches.
    Any(Vec<EntryMatch>),
}

impl EntryMatch {
    /// Whether `text` matches.
    pub fn matches(&self, text: &str) -> bool {
        match self {
            EntryMatch::Prefix(p) => text.starts_with(p.as_str()),
            EntryMatch::Contains(c) => text.contains(c.as_str()),
            EntryMatch::Any(all) => all.iter().any(|m| m.matches(text)),
        }
    }

    /// One match for all of `matches`: the match itself when there is one,
    /// else [`EntryMatch::Any`] of the distinct ones.
    pub fn any_of<I: IntoIterator<Item = EntryMatch>>(matches: I) -> Self {
        let mut all: Vec<EntryMatch> = Vec::new();
        for m in matches {
            if !all.contains(&m) {
                all.push(m);
            }
        }
        if all.len() == 1 {
            all.pop().expect("one")
        } else {
            EntryMatch::Any(all)
        }
    }
}

fn segments<P: IntoIterator<Item = S>, S: Into<String>>(path: P) -> Vec<String> {
    path.into_iter().map(Into::into).collect()
}

impl MergeOp {
    /// `value` is present once in the array at `path` (keys from the
    /// top-level object, e.g. `["permissions", "allow"]`). Found by
    /// equality.
    pub fn array_entry<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        value: impl Into<Value>,
    ) -> Self {
        MergeOp(Op::ArrayEntry {
            path: segments(path),
            value: value.into(),
        })
    }

    /// The member `key` of the object (a TOML table) at `path` (empty for the
    /// top level) is `value`, e.g. `tool` under `["mcpServers"]`. Found by
    /// its key. The only operation a TOML file takes.
    pub fn object_member<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        key: impl Into<String>,
        value: impl Into<Value>,
    ) -> Self {
        MergeOp(Op::ObjectMember {
            path: segments(path),
            key: key.into(),
            value: value.into(),
        })
    }

    /// The tool's entries of the array at `path`: the objects whose member
    /// `field` is a string `owned` matches. They become `entries` (an array
    /// of objects), in place of the tool's first entry, else appended; the
    /// user's entries stay.
    pub fn owned_entries<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        field: impl Into<String>,
        owned: EntryMatch,
        entries: Vec<Value>,
    ) -> Self {
        MergeOp(Op::OwnedEntries {
            path: segments(path),
            field: field.into(),
            owned,
            entries: Value::Array(entries),
        })
    }

    /// The tool's entries in the groups of the array at `path`. Each group is
    /// an object holding its entries as an array under `entries`, beside
    /// other keys (e.g. a `matcher`); an entry is the tool's when its member
    /// `field` is a string `owned` matches. The tool owns those entries, not
    /// the groups: each of `groups` (objects with the tool's entries under
    /// `entries`) takes the place of the tool's entries in the first group
    /// with the same other keys that held one, else is appended. The user's
    /// entries and keys stay; a group that held only the tool's entries and
    /// is left empty goes.
    pub fn group_entries<P: IntoIterator<Item = S>, S: Into<String>>(
        path: P,
        entries: impl Into<String>,
        field: impl Into<String>,
        owned: EntryMatch,
        groups: Vec<Value>,
    ) -> Self {
        MergeOp(Op::GroupEntries {
            path: segments(path),
            entries: entries.into(),
            field: field.into(),
            owned,
            groups: Value::Array(groups),
        })
    }

    /// The value the operation writes: the member, the entry, or the array
    /// of the tool's entries or groups.
    pub fn value(&self) -> &Value {
        match &self.0 {
            Op::ArrayEntry { value, .. } | Op::ObjectMember { value, .. } => value,
            Op::OwnedEntries { entries, .. } => entries,
            Op::GroupEntries { groups, .. } => groups,
        }
    }

    /// The path of the op's container.
    pub(crate) fn path(&self) -> &[String] {
        match &self.0 {
            Op::ArrayEntry { path, .. }
            | Op::ObjectMember { path, .. }
            | Op::OwnedEntries { path, .. }
            | Op::GroupEntries { path, .. } => path,
        }
    }

    /// Whether the op owns entries but has none to write: it only removes
    /// the tool's old ones.
    pub(crate) fn expects_nothing(&self) -> bool {
        match &self.0 {
            Op::OwnedEntries { entries: v, .. } | Op::GroupEntries { groups: v, .. } => {
                v.as_array().is_some_and(Vec::is_empty)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_matches() {
        assert!(EntryMatch::Prefix("a".into()).matches("ab"));
        assert!(!EntryMatch::Prefix("b".into()).matches("ab"));
        assert!(EntryMatch::Contains("b".into()).matches("abc"));
        let any = EntryMatch::any_of([
            EntryMatch::Prefix("x".into()),
            EntryMatch::Prefix("y".into()),
            EntryMatch::Prefix("x".into()),
        ]);
        assert_eq!(
            any,
            EntryMatch::Any(vec![
                EntryMatch::Prefix("x".into()),
                EntryMatch::Prefix("y".into())
            ])
        );
        assert!(any.matches("y1") && !any.matches("z"));
        assert_eq!(
            EntryMatch::any_of([EntryMatch::Prefix("x".into())]),
            EntryMatch::Prefix("x".into())
        );
    }
}
