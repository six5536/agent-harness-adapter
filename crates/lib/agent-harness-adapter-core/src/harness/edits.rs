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
        meaning,
        state::{Found, Observed, expected, observe},
    },
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
/// per file that differs from the tool's by its meaning, in the order
/// `status` reports the parts. Writes nothing.
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
        for text in texts(&e.path, expected(e.part), found) {
            if !text.same {
                out.push(Edit {
                    harness: m.harness.id().to_string(),
                    part: e.part.name().to_string(),
                    path: text.path,
                    expected: text.expected,
                    found: text.found,
                });
            }
        }
    }
    Ok(out)
}

/// One file's path, expected text and found text, and whether the two
/// mean the same.
#[derive(Debug, PartialEq, Eq)]
struct Text {
    path: String,
    expected: String,
    found: String,
    same: bool,
}

/// Each file's texts for a part at `path`, compared as the states compare
/// them.
// sokf-implements: KIT-25_AC-2
fn texts(path: &str, want: Found, have: Option<Found>) -> Vec<Text> {
    match (want, have) {
        (Found::Files(want), have) => {
            let have = match have {
                Some(Found::Files(f)) => f,
                _ => Vec::new(),
            };
            want.into_iter()
                .map(|(rel, expected)| {
                    let found = have.iter().find(|(r, _)| *r == rel).map(|(_, t)| t);
                    let same = found.is_some_and(|found| {
                        meaning::same(&meaning::file(&rel, &expected), &meaning::file(&rel, found))
                    });
                    Text {
                        path: format!("{path}/{rel}"),
                        expected,
                        found: found.cloned().unwrap_or_default(),
                        same,
                    }
                })
                .collect()
        }
        (Found::Block { file, text }, have) => {
            let found = match have {
                Some(Found::Block { text, .. }) => Some(text),
                _ => None,
            };
            let same = found.as_ref().is_some_and(|found| {
                meaning::same(&meaning::block(&file, &text), &meaning::block(&file, found))
            });
            vec![Text {
                path: path.to_string(),
                expected: text,
                found: found.unwrap_or_default(),
                same,
            }]
        }
        (Found::Text(want), have) => {
            let found = match have {
                Some(Found::Text(t)) => Some(t),
                _ => None,
            };
            let same = found
                .as_ref()
                .is_some_and(|found| meaning::same(&meaning::text(&want), &meaning::text(found)));
            vec![Text {
                path: path.to_string(),
                expected: want,
                found: found.unwrap_or_default(),
                same,
            }]
        }
        (Found::Entries(want), have) => {
            let have = match have {
                Some(Found::Entries(e)) => e,
                _ => Vec::new(),
            };
            let same = want == have;
            vec![Text {
                path: path.to_string(),
                expected: pretty(want),
                found: pretty(have),
                same,
            }]
        }
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

    fn text(path: &str, expected: &str, found: &str, same: bool) -> Text {
        Text {
            path: path.into(),
            expected: expected.into(),
            found: found.into(),
            same,
        }
    }

    // sokf-tests: KIT-25_AC-2
    #[test]
    fn each_kind_gives_its_texts() {
        let files = texts(
            ".claude/skills",
            Found::Files(vec![
                ("t/SKILL.md".into(), "new\n".into()),
                ("t/a.md".into(), "*a*\n".into()),
            ]),
            Some(Found::Files(vec![
                ("t/SKILL.md".into(), "mine\n".into()),
                ("t/a.md".into(), "_a_\n".into()),
            ])),
        );
        assert_eq!(
            files,
            [
                text(".claude/skills/t/SKILL.md", "new\n", "mine\n", false),
                text(".claude/skills/t/a.md", "*a*\n", "_a_\n", true),
            ]
        );
        let missing = texts(
            "d",
            Found::Files(vec![("a".into(), "a\n".into())]),
            Some(Found::Files(vec![])),
        );
        assert_eq!(missing, [text("d/a", "a\n", "", false)]);
        let block = |t: &str| Found::Block {
            file: "AGENTS.md".into(),
            text: t.into(),
        };
        assert_eq!(
            texts("AGENTS.md", block("ours"), Some(block("mine"))),
            [text("AGENTS.md", "ours", "mine", false)]
        );
        assert!(texts("AGENTS.md", block("*ours*"), Some(block("_ours_")))[0].same);
        let merge = texts(
            ".claude/settings.json",
            Found::Entries(vec![json!("Bash(t:*)")]),
            Some(Found::Entries(vec![])),
        );
        assert_eq!(merge[0].expected, "[\n  \"Bash(t:*)\"\n]\n");
        assert_eq!(merge[0].found, "[]\n");
        assert!(!merge[0].same);
        let ext = texts("ext:x", Found::Text("a".into()), None);
        assert_eq!(ext, [text("ext:x", "a", "", false)]);
        assert!(
            texts(
                "ext:x",
                Found::Text("a\n".into()),
                Some(Found::Text("a\r\n".into()))
            )[0]
            .same
        );
    }
}
