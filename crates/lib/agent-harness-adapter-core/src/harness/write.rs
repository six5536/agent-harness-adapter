//! Apply a plan: the external parts, then the part files (writes, then
//! deletions and the directories they leave empty), then the record.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{Error, Result, fs::write_atomic, harness::ExternalPart};

/// Everything `install` or `uninstall` will write, computed before any
/// write.
#[derive(Debug, Clone, Default)]
pub(crate) struct Plan {
    /// Each file's full new text, in profile order.
    pub(crate) files: Vec<(PathBuf, String)>,
    /// The files to delete.
    pub(crate) deletes: Vec<PathBuf>,
    /// After the deletions, remove each directory they left empty, up to
    /// (not including) this root.
    pub(crate) prune_root: Option<PathBuf>,
    /// The external parts to write, in profile order.
    pub(crate) externals: Vec<Arc<dyn ExternalPart>>,
    /// The external parts to remove, in profile order.
    pub(crate) removals: Vec<Arc<dyn ExternalPart>>,
    /// The record file and its new text (`None`: delete it), when it
    /// changes.
    pub(crate) record: Option<(PathBuf, Option<String>)>,
}

impl Plan {
    /// The planned content of `path`, when a part planned it already:
    /// `Some(None)` for a planned deletion.
    pub(crate) fn pending(&self, path: &Path) -> Option<Option<&str>> {
        if self.deletes.iter().any(|p| p == path) {
            return Some(None);
        }
        self.files
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, t)| Some(t.as_str()))
    }

    /// Plan a write, replacing an earlier plan for the same path.
    pub(crate) fn set(&mut self, path: PathBuf, text: String) {
        self.deletes.retain(|p| *p != path);
        match self.files.iter_mut().find(|(p, _)| *p == path) {
            Some(entry) => entry.1 = text,
            None => self.files.push((path, text)),
        }
    }

    /// Plan a deletion, replacing an earlier plan for the same path.
    pub(crate) fn delete(&mut self, path: PathBuf) {
        self.files.retain(|(p, _)| *p != path);
        if !self.deletes.contains(&path) {
            self.deletes.push(path);
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
// @zen-impl: KIT-22_AC-8
pub(crate) fn apply_plan(plan: &Plan) -> Result<()> {
    for ext in &plan.externals {
        ext.write()?;
    }
    for ext in &plan.removals {
        ext.remove()?;
    }
    for (path, text) in &plan.files {
        write_if_changed(path, text)?;
    }
    for path in &plan.deletes {
        delete(path)?;
    }
    if let Some(root) = &plan.prune_root {
        for path in &plan.deletes {
            prune(root, path);
        }
    }
    match &plan.record {
        Some((path, Some(text))) => write_if_changed(path, text)?,
        Some((path, None)) => {
            delete(path)?;
            if let Some(root) = &plan.prune_root {
                prune(root, path);
            }
        }
        None => {}
    }
    Ok(())
}

/// Delete `path`; one already gone is fine.
fn delete(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// Remove each directory above `path` that is empty, stopping at the first
/// that is not, or at `root`.
// @zen-impl: KIT-22_AC-1
fn prune(root: &Path, path: &Path) {
    let mut dir = path.parent();
    while let Some(d) = dir {
        if d == root || !d.starts_with(root) || fs::remove_dir(d).is_err() {
            return;
        }
        dir = d.parent();
    }
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
        assert_eq!(plan.pending(Path::new("/a")), Some(Some("3")));
        assert_eq!(plan.pending(Path::new("/c")), None);
        assert_eq!(plan.files.len(), 2);
        // A deletion replaces a write, and a write a deletion.
        plan.delete("/a".into());
        plan.delete("/a".into());
        assert_eq!(plan.pending(Path::new("/a")), Some(None));
        assert_eq!((plan.files.len(), plan.deletes.len()), (1, 1));
        plan.set("/a".into(), "4".into());
        assert_eq!(plan.pending(Path::new("/a")), Some(Some("4")));
        assert!(plan.deletes.is_empty());
    }

    // @zen-test: KIT-22_AC-1
    #[test]
    fn deletions_take_the_directories_they_empty() {
        let dir = temp_dir("write-delete");
        let skill = dir.join(".claude/skills/t/SKILL.md");
        let other = dir.join(".claude/settings.json");
        let record = dir.join(".tool/harness.toml");
        for f in [&skill, &other, &record] {
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, "x").unwrap();
        }
        let plan = Plan {
            deletes: vec![skill.clone(), dir.join("gone/already.md")],
            prune_root: Some(dir.clone()),
            record: Some((record.clone(), None)),
            ..Plan::default()
        };
        apply_plan(&plan).unwrap();
        assert!(!dir.join(".claude/skills").exists());
        assert!(other.exists(), "a directory still holding a file stays");
        assert!(!dir.join(".tool").exists());
        assert!(dir.exists(), "the root stays");
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: KIT-7_AC-1
    #[test]
    fn writes_files_and_record_creating_directories() {
        let dir = temp_dir("write");
        let plan = Plan {
            files: vec![(dir.join(".claude/skills/t/SKILL.md"), "s\n".into())],
            record: Some((dir.join(".tool/harness.toml"), Some("r\n".into()))),
            ..Plan::default()
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
            record: Some((dir.join("r.toml"), Some("r\n".into()))),
            ..Plan::default()
        };
        let e = apply_plan(&plan).unwrap_err();
        assert_eq!(e.to_string(), "could not run it");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }
}
