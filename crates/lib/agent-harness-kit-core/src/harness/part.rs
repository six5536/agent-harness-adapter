//! Profiles and parts: what a harness renders from a tool's integration,
//! one profile per harness and scope. The kit embeds no content.

use std::{fmt, sync::Arc};

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

/// A part's kind and the content the tool wants there.
#[derive(Debug, Clone)]
pub(crate) enum Kind {
    /// `file`: the tool owns these files (relative path, text) of `dir`.
    Files {
        dir: String,
        files: Vec<(String, String)>,
    },
    /// `region`: the tool owns one block between its markers.
    Region { file: String, block: String },
    /// `merge`: the tool owns entries in the JSON or TOML file `file`.
    Merge { file: String, ops: Vec<MergeOp> },
    /// `external`: the tool reads and writes the part itself.
    External(Arc<dyn ExternalPart>),
}

/// One part of a profile, built by [`Part::files`], [`Part::region`],
/// [`Part::merge`] or [`Part::external`].
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

    /// A `region` part in `file`. A harness picks the file when it renders,
    /// from the tree under the root.
    // @zen-impl: KIT-2_AC-2
    pub fn region(
        name: impl Into<String>,
        file: impl Into<String>,
        block: impl Into<String>,
    ) -> Self {
        Self::new(
            name,
            Kind::Region {
                file: file.into(),
                block: block.into(),
            },
        )
    }

    /// A `merge` part: `ops` applied to the JSON object in `file`, or to the
    /// TOML document when `file` ends in `.toml`.
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

    /// Where the part is written, relative to the root and `/`-separated:
    /// the directory of a `file` part, the file of a region or merge, an
    /// external part's location.
    pub fn location(&self) -> String {
        match &self.kind {
            Kind::Files { dir, .. } => dir.clone(),
            Kind::Region { file, .. } | Kind::Merge { file, .. } => file.clone(),
            Kind::External(ext) => ext.location(),
        }
    }

    /// A `merge` part's operations.
    #[cfg(test)]
    pub(crate) fn ops(&self) -> &[MergeOp] {
        match &self.kind {
            Kind::Merge { ops, .. } => ops,
            _ => &[],
        }
    }

    /// Whether `self` and `other` write the same content to the same
    /// location (names aside); external parts by location and expected
    /// content.
    pub(crate) fn same_content(&self, other: &Part) -> bool {
        match (&self.kind, &other.kind) {
            (Kind::Files { dir: a, files: x }, Kind::Files { dir: b, files: y }) => {
                a == b && x == y
            }
            (Kind::Region { file: a, block: x }, Kind::Region { file: b, block: y }) => {
                a == b && x == y
            }
            (Kind::Merge { file: a, ops: x }, Kind::Merge { file: b, ops: y }) => a == b && x == y,
            (Kind::External(x), Kind::External(y)) => {
                x.location() == y.location() && x.expected() == y.expected()
            }
            _ => false,
        }
    }
}

/// A harness profile at one scope: its parts in profile order, as the
/// harness rendered them from the integration, then the raw parts.
#[derive(Debug, Clone)]
pub(crate) struct Profile {
    parts: Vec<Part>,
}

impl Profile {
    /// A profile of `parts` in profile order.
    pub(crate) fn new(parts: Vec<Part>) -> Self {
        Profile { parts }
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

    // @zen-test: KIT-2_AC-1
    // @zen-test: KIT-2_AC-2
    // @zen-test: KIT-2_AC-4
    #[test]
    fn constructors_set_the_kind() {
        let p = Profile::new(vec![
            Part::files(
                "skills",
                ".claude/skills/t",
                vec![("SKILL.md".into(), "s".into())],
            ),
            Part::region("notes", "NOTES.md", "n"),
            Part::merge("hooks", ".claude/settings.json", vec![]),
            Part::external("mcp", Arc::new(Ext)),
        ]);
        assert_eq!(p.parts().len(), 4);
        assert!(
            matches!(&p.part("skills").unwrap().kind, Kind::Files { dir, .. } if dir == ".claude/skills/t")
        );
        assert!(matches!(
            &p.part("notes").unwrap().kind,
            Kind::Region { file, .. } if file == "NOTES.md"
        ));
        let locations: Vec<_> = p.parts().iter().map(Part::location).collect();
        assert_eq!(
            locations,
            [
                ".claude/skills/t",
                "NOTES.md",
                ".claude/settings.json",
                "ext (user)"
            ]
        );
        let parts = p.parts();
        assert!(parts[0].same_content(&parts[0].clone()));
        assert!(!parts[0].same_content(&parts[1]));
        assert!(parts[1].same_content(&Part::region("other name", "NOTES.md", "n")));
        assert!(!parts[1].same_content(&Part::region("notes", "NOTES.md", "m")));
        assert!(parts[2].same_content(&Part::merge("x", ".claude/settings.json", vec![])));
        assert!(parts[3].same_content(&Part::external("y", Arc::new(Ext))));
        assert!(matches!(&p.part("hooks").unwrap().kind, Kind::Merge { .. }));
        assert!(matches!(&p.part("mcp").unwrap().kind, Kind::External(_)));
        assert_eq!(p.part("mcp").unwrap().name(), "mcp");
        assert!(p.part("nope").is_none());
    }
}
