//! Whole-file reads and atomic writes that respect what the user set up:
//! a symlink stays a symlink, a file keeps its permissions.
// @zen-component: KIT-Fs

use std::{
    fs::{self, File},
    io::Write as _,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{Error, Result};

/// Symlinks followed before giving up (the kernel's own limit is 40).
const MAX_LINKS: usize = 40;

static NEXT_TMP: AtomicU64 = AtomicU64::new(0);

/// The text of `path`; `None` when absent.
// @zen-impl: KIT-15_AC-1
pub fn read_text(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

/// The user's home directory: `HOME`, else `USERPROFILE` (Windows); `None`
/// when neither is set.
// @zen-impl: KIT-15_AC-3
pub fn home_dir() -> Option<PathBuf> {
    home_from(|k| std::env::var_os(k))
}

/// Whether `a` and `b` name the same directory: equal, or equal once each is
/// resolved, so a path through a symlink (macOS's `/var` is `/private/var`)
/// or a Windows short name matches the path it stands for.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    a == b
        || matches!(
            (fs::canonicalize(a), fs::canonicalize(b)),
            (Ok(x), Ok(y)) if x == y
        )
}

/// `path` relative to `root`, `/`-separated on every system, when it lies
/// under `root` (and is not `root` itself). Built from the path's
/// components, so a path that mixes `\` and `/`, as `root.join("a/b")` does on
/// Windows, still gives `a/b`, and a `\` in a Unix file name stays. When the
/// paths differ only through a symlink or a Windows short name, the resolved
/// paths decide.
///
/// Every path shown to a user or stored in a file, relative to a root, comes
/// from here or from [`shown`], never from `Path::display`.
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let abs = |p: &Path| std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf());
    let (path, root) = (abs(path), abs(root));
    let rel = path
        .strip_prefix(&root)
        .ok()
        .map(Path::to_path_buf)
        .or_else(|| {
            let (p, r) = (fs::canonicalize(&path).ok()?, fs::canonicalize(&root).ok()?);
            p.strip_prefix(r).ok().map(Path::to_path_buf)
        })?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// `path` as a user reads it: [`relative`] to `root` when it lies under it,
/// else the whole path in the system's form.
pub fn shown(root: &Path, path: &Path) -> String {
    relative(root, path).unwrap_or_else(|| path.display().to_string())
}

fn home_from(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(var)
        .find(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Write `contents` to `path` through a temp file and a rename, so a reader
/// never sees half a file, creating the parent directories. When `path` is a
/// symlink the file it points to is replaced, not the link; an existing
/// file's permissions carry over. The temp name is unique per write, so
/// concurrent writers never share one.
// @zen-impl: KIT-15_AC-2
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    let target = resolve(path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let n = NEXT_TMP.fetch_add(1, Ordering::Relaxed);
    let tmp = target.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()));
    let written = fill(&tmp, &target, contents.as_ref())
        .and_then(|()| fs::rename(&tmp, &target).map_err(|e| Error::io(&target, e)));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// Create `tmp` with `target`'s permissions (before any byte lands in it),
/// then write `contents`.
fn fill(tmp: &Path, target: &Path, contents: &[u8]) -> Result<()> {
    let mut f = File::create(tmp).map_err(|e| Error::io(tmp, e))?;
    if let Ok(meta) = fs::metadata(target) {
        f.set_permissions(meta.permissions())
            .map_err(|e| Error::io(tmp, e))?;
    }
    f.write_all(contents).map_err(|e| Error::io(tmp, e))
}

/// `path` with its symlinks followed, dangling ones included (the write then
/// creates the file the link names).
fn resolve(path: &Path) -> PathBuf {
    let mut p = path.to_path_buf();
    for _ in 0..MAX_LINKS {
        let Ok(link) = fs::read_link(&p) else { break };
        // A relative link is relative to the link's directory; `join` keeps
        // an absolute one as is.
        p = match p.parent() {
            Some(dir) => dir.join(link),
            None => link,
        };
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    // @zen-test: KIT-15_AC-3
    #[test]
    fn home_is_home_else_userprofile() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| std::ffi::OsString::from(v))
            }
        };
        assert_eq!(
            home_from(env(&[("HOME", "/h"), ("USERPROFILE", "C:\\u")])),
            Some(PathBuf::from("/h"))
        );
        assert_eq!(
            home_from(env(&[("HOME", ""), ("USERPROFILE", "C:\\u")])),
            Some(PathBuf::from("C:\\u"))
        );
        assert_eq!(home_from(env(&[])), None);
        let _ = home_dir();
    }
    use crate::test_support::temp_dir;

    // @zen-test: KIT-15_AC-1
    #[test]
    fn reads_absent_as_none() {
        let dir = temp_dir("fs-read");
        assert_eq!(read_text(&dir.join("nope")).unwrap(), None);
        write_atomic(&dir.join("a/b.txt"), "x").unwrap();
        assert_eq!(
            read_text(&dir.join("a/b.txt")).unwrap().as_deref(),
            Some("x")
        );
        assert!(matches!(read_text(&dir), Err(Error::Io { .. })));
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: KIT-15_AC-2
    #[test]
    fn leaves_no_temp_files() {
        let dir = temp_dir("fs-tmp");
        write_atomic(&dir.join("f"), "1").unwrap();
        write_atomic(&dir.join("f"), b"2").unwrap();
        assert_eq!(fs::read(dir.join("f")).unwrap(), b"2");
        let names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names, ["f"]);
        // A failed rename (onto a directory) removes its temp file.
        fs::create_dir(dir.join("d")).unwrap();
        fs::write(dir.join("d/x"), "").unwrap();
        assert!(write_atomic(&dir.join("d"), "3").is_err());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: KIT-15_AC-2
    #[cfg(unix)]
    #[test]
    fn writes_through_symlinks_and_keeps_the_mode() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let dir = temp_dir("fs-link");
        fs::write(dir.join("AGENTS.md"), "a\n").unwrap();
        symlink("AGENTS.md", dir.join("CLAUDE.md")).unwrap();
        write_atomic(&dir.join("CLAUDE.md"), "b\n").unwrap();
        assert!(
            fs::symlink_metadata(dir.join("CLAUDE.md"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(dir.join("AGENTS.md")).unwrap(), "b\n");
        // A dangling link gets its file created.
        symlink(dir.join("sub/new.toml"), dir.join("cfg.toml")).unwrap();
        write_atomic(&dir.join("cfg.toml"), "c\n").unwrap();
        assert!(
            fs::symlink_metadata(dir.join("cfg.toml"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(dir.join("sub/new.toml")).unwrap(), "c\n");
        // A private file stays private.
        let secret = dir.join("settings.json");
        fs::write(&secret, "{}").unwrap();
        fs::set_permissions(&secret, fs::Permissions::from_mode(0o600)).unwrap();
        write_atomic(&secret, "{\"a\":1}").unwrap();
        let mode = fs::metadata(&secret).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_path_is_shown_relative_with_slashes() {
        let root = Path::new("/r");
        assert_eq!(
            relative(root, &root.join(".tool/h.toml")).as_deref(),
            Some(".tool/h.toml")
        );
        // Joined a component at a time, as the system separates them.
        let nested = root.join(".claude").join("skills").join("review");
        assert_eq!(
            relative(root, &nested).as_deref(),
            Some(".claude/skills/review")
        );
        assert_eq!(relative(root, root), None);
        assert_eq!(relative(root, Path::new("/x/h.toml")), None);
        assert_eq!(shown(root, &root.join(".tool/h.toml")), ".tool/h.toml");
        assert_eq!(
            shown(root, Path::new("/x/h.toml")),
            Path::new("/x/h.toml").display().to_string()
        );
    }

    // On Unix `\` is a character of a file name, not a separator.
    #[cfg(unix)]
    #[test]
    fn a_backslash_in_a_unix_name_stays() {
        assert_eq!(
            relative(Path::new("/r"), Path::new("/r/a\\b")).as_deref(),
            Some("a\\b")
        );
    }

    #[test]
    fn a_path_through_a_link_is_still_under_its_root() {
        let dir = temp_dir("fs-same");
        fs::create_dir_all(dir.join("a/b")).unwrap();
        assert!(same_dir(&dir.join("a"), &dir.join("a/b/..")));
        assert!(!same_dir(&dir.join("a"), &dir.join("a/b")));
        // An absent path matches only itself.
        assert!(same_dir(&dir.join("none"), &dir.join("none")));
        assert!(!same_dir(&dir.join("none"), &dir.join("a")));
        fs::remove_dir_all(&dir).unwrap();
    }
}
