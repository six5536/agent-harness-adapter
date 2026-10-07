//! Apply a plan: the external parts, then the part files, then the record.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{Result, fs::write_atomic, harness::ExternalPart};

/// Everything `install` will write, computed before any write.
#[derive(Debug, Clone, Default)]
pub(crate) struct Plan {
    /// Each file's full new text, in profile order.
    pub(crate) files: Vec<(PathBuf, String)>,
    /// The external parts to write, in profile order.
    pub(crate) externals: Vec<Arc<dyn ExternalPart>>,
    /// The record file and its new text, when it changes.
    pub(crate) record: Option<(PathBuf, String)>,
}

impl Plan {
    /// The planned text of `path`, when a part planned one already.
    pub(crate) fn pending(&self, path: &Path) -> Option<&str> {
        self.files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, t)| t.as_str())
    }

    /// Plan a write, replacing an earlier plan for the same path.
    pub(crate) fn set(&mut self, path: PathBuf, text: String) {
        match self.files.iter_mut().find(|(p, _)| *p == path) {
            Some(entry) => entry.1 = text,
            None => self.files.push((path, text)),
        }
    }
}

/// Write `text` to `path` unless it is already there ([`write_atomic`]).
// @zen-impl: KIT-7_AC-1
pub(crate) fn write_if_changed(path: &Path, text: &str) -> Result<()> {
    if fs::read_to_string(path).ok().as_deref() == Some(text) {
        return Ok(());
    }
    write_atomic(path, text)
}

/// Write the plan: the external parts first (they run other programs, the
/// likeliest to fail, so a failure leaves every file as found), then the
/// files in order, then the record. A file whose text is already on disk is
/// left untouched.
// @zen-impl: KIT-6_AC-2
pub(crate) fn apply_plan(plan: &Plan) -> Result<()> {
    for ext in &plan.externals {
        ext.write()?;
    }
    for (path, text) in &plan.files {
        write_if_changed(path, text)?;
    }
    if let Some((path, text)) = &plan.record {
        write_if_changed(path, text)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, test_support::temp_dir};

    #[test]
    fn a_plan_replaces_an_earlier_write_to_the_same_path() {
        let mut plan = Plan::default();
        plan.set("/a".into(), "1".into());
        plan.set("/b".into(), "2".into());
        plan.set("/a".into(), "3".into());
        assert_eq!(plan.pending(Path::new("/a")), Some("3"));
        assert_eq!(plan.pending(Path::new("/c")), None);
        assert_eq!(plan.files.len(), 2);
    }

    // @zen-test: KIT-7_AC-1
    #[test]
    fn writes_files_and_record_creating_directories() {
        let dir = temp_dir("write");
        let plan = Plan {
            files: vec![(dir.join(".claude/skills/t/SKILL.md"), "s\n".into())],
            externals: vec![],
            record: Some((dir.join(".tool/harness.toml"), "r\n".into())),
        };
        apply_plan(&plan).unwrap();
        assert_eq!(
            fs::read_to_string(dir.join(".claude/skills/t/SKILL.md")).unwrap(),
            "s\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join(".tool/harness.toml")).unwrap(),
            "r\n"
        );
        let before = fs::metadata(dir.join(".tool/harness.toml"))
            .unwrap()
            .modified()
            .unwrap();
        apply_plan(&plan).unwrap();
        assert_eq!(
            fs::metadata(dir.join(".tool/harness.toml"))
                .unwrap()
                .modified()
                .unwrap(),
            before
        );
        // A parent that is a file cannot be created.
        let e = write_if_changed(&dir.join(".tool/harness.toml/x"), "x").unwrap_err();
        assert!(matches!(e, Error::Io { .. }), "{e}");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[derive(Debug)]
    struct Failing;

    impl ExternalPart for Failing {
        fn location(&self) -> String {
            "failing".into()
        }
        fn expected(&self) -> String {
            String::new()
        }
        fn observe(&self) -> Result<Option<String>> {
            Ok(None)
        }
        fn write(&self) -> Result<()> {
            Err(Error::Refused("could not run it".into()))
        }
    }

    // @zen-test: KIT-6_AC-2
    #[test]
    fn a_failing_external_part_leaves_every_file_as_found() {
        let dir = temp_dir("write-external");
        let plan = Plan {
            files: vec![(dir.join("a.md"), "a\n".into())],
            externals: vec![Arc::new(Failing)],
            record: Some((dir.join("r.toml"), "r\n".into())),
        };
        let e = apply_plan(&plan).unwrap_err();
        assert_eq!(e.to_string(), "could not run it");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }
}
