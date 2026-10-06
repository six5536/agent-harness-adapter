//! The path of each part under a root directory.

use std::path::Path;

use crate::{
    Result,
    harness::part::{Kind, Part, Target},
};

/// The path `part` is written to, relative to `root` and `/`-separated; for
/// an external part, its location.
pub(crate) fn target_path(root: &Path, part: &Part) -> Result<String> {
    Ok(match &part.kind {
        Kind::Files { dir, .. } => dir.clone(),
        Kind::Region { target, .. } => match target {
            Target::Fixed(p) => p.clone(),
            Target::Chosen(choose) => choose(root)?,
        },
        Kind::Merge { file, .. } => file.clone(),
        Kind::External(ext) => ext.location(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    fn chosen(root: &Path) -> Result<String> {
        Ok(format!("{}.md", root.display()))
    }

    fn refuses(_: &Path) -> Result<String> {
        Err(Error::Refused("no".into()))
    }

    #[test]
    fn each_kind_has_its_path() {
        let root = Path::new("r");
        let t = |p: &Part| target_path(root, p);
        assert_eq!(t(&Part::files("f", "d/x", vec![])).unwrap(), "d/x");
        assert_eq!(t(&Part::region("r", "N.md", "b")).unwrap(), "N.md");
        assert_eq!(t(&Part::region_chosen("r", chosen, "b")).unwrap(), "r.md");
        assert!(t(&Part::region_chosen("r", refuses, "b")).is_err());
        assert_eq!(
            t(&Part::merge("m", ".claude/settings.json", vec![])).unwrap(),
            ".claude/settings.json"
        );
    }
}
