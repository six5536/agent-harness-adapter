//! Running `agent-harness-adapter` over a temporary directory.

#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use assert_cmd::Command;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// The `agent-harness-adapter` binary.
pub fn adapter() -> Command {
    Command::cargo_bin("agent-harness-adapter").unwrap()
}

/// Run `cmd`: its exit code, stdout and stderr.
pub fn output(cmd: &mut Command) -> (i32, String, String) {
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

/// A temporary directory, removed on drop.
pub struct Tree(PathBuf);

impl Tree {
    pub fn new() -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("aha-cli-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Tree(dir)
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }

    pub fn dir(&self) -> &Path {
        &self.0
    }

    pub fn mkdir(&self, rel: &str) {
        fs::create_dir_all(self.0.join(rel)).unwrap();
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.0.join(rel)).unwrap()
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.0.join(rel).exists()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
