//! Profiles and parts: data the tool supplies, one profile per harness and
//! scope. The kit embeds no content.

use std::{fmt, path::Path, sync::Arc};

use crate::{Result, harness::MergeOp};

/// A part the kit does not write as a file: the tool reads and writes it
/// itself, e.g. by running `claude mcp add-json --scope user`.
// @zen-impl: KIT-2_AC-4
pub trait ExternalPart: fmt::Debug + Send + Sync {
    /// Where the part lives, as the report shows it, e.g. `claude mcp (user)`.
    fn location(&self) -> String;
    /// The content the tool wants, compared with what [`observe`] returns
    /// (line endings aside) and hashed into the record.
    ///
    /// [`observe`]: ExternalPart::observe
    fn expected(&self) -> String;
    /// What is there now; `None` when the part is absent.
    fn observe(&self) -> Result<Option<String>>;
    /// Make the part hold [`expected`](ExternalPart::expected).
    fn write(&self) -> Result<()>;
}

/// A rule that picks a region part's file under the root at install time,
/// as a path relative to the root, `/`-separated. See
/// [`Part::region_chosen`].
pub type ChooseFile = fn(&Path) -> Result<String>;

/// Where a region lives.
#[derive(Debug, Clone)]
pub(crate) enum Target {
    /// A path the profile fixes, relative to the root, `/`-separated.
    Fixed(String),
    /// A path a rule chooses under the root.
    Chosen(ChooseFile),
}

/// A part's kind and the content the tool wants there.
#[derive(Debug, Clone)]
pub(crate) enum Kind {
    /// `file`: the tool owns these files (relative path, text) of `dir`.
    Files {
        dir: String,
        files: Vec<(String, String)>,
    },
    /// `region`: the tool owns one block between its markers.
    Region { target: Target, block: String },
    /// `merge`: the tool owns entries in the JSON object of `file`.
    Merge { file: String, ops: Vec<MergeOp> },
    /// `external`: the tool reads and writes the part itself.
    External(Arc<dyn ExternalPart>),
}

/// One part of a profile, built by [`Part::files`], [`Part::region`],
/// [`Part::region_chosen`], [`Part::merge`] or [`Part::external`].
#[derive(Debug, Clone)]
pub struct Part {
    name: String,
    pub(crate) kind: Kind,
}

impl Part {
    fn new(name: impl Into<String>, kind: Kind) -> Self {
        Part {
            name: name.into(),
            kind,
        }
    }

    /// A `file` part: `files` (relative path, text) under the directory
    /// `dir`, written with LF line endings.
    // @zen-impl: KIT-2_AC-1
    pub fn files(
        name: impl Into<String>,
        dir: impl Into<String>,
        files: Vec<(String, String)>,
    ) -> Self {
        Self::new(
            name,
            Kind::Files {
                dir: dir.into(),
                files,
            },
        )
    }

    /// A `region` part in a fixed file.
    // @zen-impl: KIT-2_AC-2
    pub fn region(
        name: impl Into<String>,
        file: impl Into<String>,
        block: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            Kind::Region {
                target: Target::Fixed(file.into()),
                block: block.into(),
            },
        )
    }

    /// A `region` part in a file `choose` picks under the root when the part
    /// is examined, e.g. [`claude::instructions_file`](crate::claude::instructions_file).
    pub fn region_chosen(
        name: impl Into<String>,
        choose: ChooseFile,
        block: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            Kind::Region {
                target: Target::Chosen(choose),
                block: block.into(),
            },
        )
    }

    /// A `merge` part: `ops` applied to the JSON object in `file`.
    pub fn merge(name: impl Into<String>, file: impl Into<String>, ops: Vec<MergeOp>) -> Self {
        Self::new(
            name,
            Kind::Merge {
                file: file.into(),
                ops,
            },
        )
    }

    /// An `external` part.
    pub fn external(name: impl Into<String>, part: Arc<dyn ExternalPart>) -> Self {
        Self::new(name, Kind::External(part))
    }

    /// The part's name, as `--without` and the report use it.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A harness profile at one scope: its parts in profile order.
// @zen-impl: KIT-1_AC-1
#[derive(Debug, Clone)]
pub struct Profile {
    harness: String,
    parts: Vec<Part>,
}

impl Profile {
    /// The profile of `harness` (as `<NAME>` on the command line, e.g.
    /// `claude`) with `parts` in profile order.
    pub fn new(harness: impl Into<String>, parts: Vec<Part>) -> Self {
        Profile {
            harness: harness.into(),
            parts,
        }
    }

    /// The harness name.
    pub fn harness(&self) -> &str {
        &self.harness
    }

    /// The parts, in profile order.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }

    /// The part named `name`.
    pub fn part(&self, name: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct Ext;

    impl ExternalPart for Ext {
        fn location(&self) -> String {
            "ext (user)".into()
        }
        fn expected(&self) -> String {
            "x".into()
        }
        fn observe(&self) -> Result<Option<String>> {
            Ok(None)
        }
        fn write(&self) -> Result<()> {
            Ok(())
        }
    }

    fn agents(_: &Path) -> Result<String> {
        Ok("AGENTS.md".into())
    }

    // @zen-test: KIT-2_AC-1
    // @zen-test: KIT-2_AC-2
    // @zen-test: KIT-2_AC-4
    #[test]
    fn constructors_set_the_kind() {
        let p = Profile::new(
            "claude",
            vec![
                Part::files(
                    "skills",
                    ".claude/skills/t",
                    vec![("SKILL.md".into(), "s".into())],
                ),
                Part::region_chosen("instructions", agents, "b"),
                Part::region("notes", "NOTES.md", "n"),
                Part::merge("hooks", ".claude/settings.json", vec![]),
                Part::external("mcp", Arc::new(Ext)),
            ],
        );
        assert_eq!(p.harness(), "claude");
        assert_eq!(p.parts().len(), 5);
        assert!(
            matches!(&p.part("skills").unwrap().kind, Kind::Files { dir, .. } if dir == ".claude/skills/t")
        );
        assert!(matches!(
            &p.part("instructions").unwrap().kind,
            Kind::Region {
                target: Target::Chosen(_),
                ..
            }
        ));
        assert!(matches!(
            &p.part("notes").unwrap().kind,
            Kind::Region { target: Target::Fixed(f), .. } if f == "NOTES.md"
        ));
        assert!(matches!(&p.part("hooks").unwrap().kind, Kind::Merge { .. }));
        assert!(matches!(&p.part("mcp").unwrap().kind, Kind::External(_)));
        assert_eq!(p.part("mcp").unwrap().name(), "mcp");
        assert!(p.part("nope").is_none());
    }
}
