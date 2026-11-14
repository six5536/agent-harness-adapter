//! `edits`: how each edited part differs from the tool's content, as the
//! text `install` would write and the text found, for a consumer to show as
//! a patch.

use serde::Serialize;
use serde_json::Value;

use crate::{
    Result,
    harness::{
        Scope, State, Tool,
        install::{Force, run},
        state::{Found, Observed, expected, observe},
    },
    hash::normalise,
};

// sokf:begin edit
/// One file of an edited part: the text `install` would write, and the text
/// found.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Edit {
    /// The harness.
    pub harness: String,
    /// The part's name.
    pub part: String,
    /// The file's path relative to the root, `/`-separated; an external
    /// part's location.
    pub path: String,
    /// The text `install` would write.
    pub expected: String,
    /// The text found; empty for a file of the part that is missing.
    pub found: String,
}
// sokf:end edit

/// The edits of the parts of `harnesses` at `scope` whose state is edited,
/// per file, in the order `status` reports the parts. Writes nothing.
// sokf-implements: KIT-25_AC-1
pub fn edits<T: Tool + ?Sized, I: IntoIterator<Item = S>, S: AsRef<str>>(
    tool: &T,
    harnesses: I,
    scope: Scope,
) -> Result<Vec<Edit>> {
    let named: Vec<String> = harnesses
        .into_iter()
        .map(|s| s.as_ref().to_string())
        .collect();
    let mut run = run(tool, &named, scope, &Force::None)?;
    let shared = run.share()?;
    let mut out = Vec::new();
    for e in run.examine(&shared)? {
        let m = &run.members[e.member];
        if !m.named || e.state != State::Edited {
            continue;
        }
        let found = match observe(&run.cx.root, e.part, &e.path, &run.markers)? {
            Observed::Present(f) => Some(f),
            Observed::Absent => None,
        };
        let edit = |path: String, expected: String, found: String| Edit {
            harness: m.harness.id().to_string(),
            part: e.part.name().to_string(),
            path,
            expected,
            found,
        };
        for (path, want, have) in texts(&e.path, expected(e.part), found) {
            if normalise(&want) != normalise(&have) {
                out.push(edit(path, want, have));
            }
        }
    }
    Ok(out)
}

/// Each file's path, expected text and found text for a part at `path`.
// sokf-implements: KIT-25_AC-2
fn texts(path: &str, want: Found, have: Option<Found>) -> Vec<(String, String, String)> {
    match (want, have) {
        (Found::Files(want), have) => {
            let have = match have {
                Some(Found::Files(f)) => f,
                _ => Vec::new(),
            };
            let text = |rel: &str| {
                have.iter()
                    .find(|(r, _)| r == rel)
                    .map(|(_, t)| t.clone())
                    .unwrap_or_default()
            };
            want.into_iter()
                .map(|(rel, t)| (format!("{path}/{rel}"), t, text(&rel)))
                .collect()
        }
        (Found::Block(want) | Found::Text(want), have) => {
            vec![(path.to_string(), want, block(have))]
        }
        (Found::Entries(want), have) => {
            let have = match have {
                Some(Found::Entries(e)) => e,
                _ => Vec::new(),
            };
            vec![(path.to_string(), pretty(want), pretty(have))]
        }
    }
}

fn block(have: Option<Found>) -> String {
    match have {
        Some(Found::Block(t) | Found::Text(t)) => t,
        _ => String::new(),
    }
}

/// `entries` as a pretty JSON array, with a final newline.
fn pretty(entries: Vec<Value>) -> String {
    let mut text = serde_json::to_string_pretty(&Value::Array(entries)).unwrap_or_default();
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // sokf-tests: KIT-25_AC-2
    #[test]
    fn each_kind_gives_its_texts() {
        let files = texts(
            ".claude/skills",
            Found::Files(vec![("t/SKILL.md".into(), "new\n".into())]),
            Some(Found::Files(vec![("t/SKILL.md".into(), "mine\n".into())])),
        );
        assert_eq!(
            files,
            [(
                ".claude/skills/t/SKILL.md".to_string(),
                "new\n".to_string(),
                "mine\n".to_string()
            )]
        );
        let missing = texts(
            "d",
            Found::Files(vec![("a".into(), "a\n".into())]),
            Some(Found::Files(vec![])),
        );
        assert_eq!(missing[0].2, "");
        let block = texts(
            "AGENTS.md",
            Found::Block("ours".into()),
            Some(Found::Block("mine".into())),
        );
        assert_eq!(block[0], ("AGENTS.md".into(), "ours".into(), "mine".into()));
        let merge = texts(
            ".claude/settings.json",
            Found::Entries(vec![json!("Bash(t:*)")]),
            Some(Found::Entries(vec![])),
        );
        assert_eq!(merge[0].1, "[\n  \"Bash(t:*)\"\n]\n");
        assert_eq!(merge[0].2, "[]\n");
        let ext = texts("ext:x", Found::Text("a".into()), None);
        assert_eq!(ext[0].2, "");
    }
}
