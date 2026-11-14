//! Whether a harness is in use (HAR-16), from its own files.

use std::path::Path;

use crate::harness::{Context, Markers, Scope};

/// Whether one of `project` (at project and local scope) or `user` (at
/// user scope) exists under the context's root: a directory, or a file that
/// holds text of the project's own beside the tools' blocks, since a file
/// the adapter wrote for a block alone shows no use.
// sokf-implements: HAR-16_AC-2
pub(crate) fn used(cx: &Context, project: &[&str], user: &[&str]) -> bool {
    let paths = if cx.scope == Scope::User {
        user
    } else {
        project
    };
    paths.iter().any(|p| {
        let path = cx.root.join(p);
        if path.is_dir() {
            dir_used(&path)
        } else {
            file_used(&path)
        }
    })
}

/// Whether a file shows use: it holds text of its own, or it cannot be
/// read as text (the user's, then).
fn file_used(path: &Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(t) => Markers::own_text(&t),
        Err(_) => path.exists(),
    }
}

/// Whether a directory shows use: it holds a directory, or a file that
/// shows use; one the adapter made for a block's file alone shows none.
fn dir_used(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return true;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        p.is_dir() || file_used(&p)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    // sokf-tests: HAR-16_AC-2
    #[test]
    fn a_path_of_the_scope_shows_it() {
        let dir = temp_dir("used");
        let cx = |scope| Context::new("t", scope, dir.clone(), None);
        assert!(!used(&cx(Scope::Project), &["A.md"], &[".a"]));
        // A file of tools' blocks alone shows no use; text of its own does.
        std::fs::write(
            dir.join("A.md"),
            "<!-- t:harness -->\nx\n<!-- /t:harness -->\n",
        )
        .unwrap();
        assert!(!used(&cx(Scope::Project), &["A.md"], &[".a"]));
        std::fs::write(dir.join("A.md"), "# Mine\n").unwrap();
        assert!(used(&cx(Scope::Project), &["A.md"], &[".a"]));
        assert!(used(&cx(Scope::Local), &["A.md"], &[".a"]));
        assert!(!used(&cx(Scope::User), &["A.md"], &[".a"]));
        // A directory of a block's file alone shows none; with more, it does.
        std::fs::create_dir(dir.join(".a")).unwrap();
        std::fs::write(
            dir.join(".a/AGENTS.md"),
            "<!-- t:harness -->\nx\n<!-- /t:harness -->\n",
        )
        .unwrap();
        assert!(!used(&cx(Scope::User), &["A.md"], &[".a"]));
        std::fs::write(dir.join(".a/settings.json"), "{}\n").unwrap();
        assert!(used(&cx(Scope::User), &["A.md"], &[".a"]));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
