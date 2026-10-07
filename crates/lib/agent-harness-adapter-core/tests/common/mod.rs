//! A test tool over a temporary tree.

#![allow(dead_code)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use agent_harness_adapter_core::{
    DeclinedStore, Error, ExternalPart, Harness, InstallResult, Integration, Part, PartResult,
    Result, Scope, TomlDeclined, Tool,
    claude::Claude,
    hook::Event,
    integration::{Hook, McpServer, Skill},
};
use serde_json::json;

pub const INSTRUCTIONS: &str = "This project uses tool.\nRead the tool skill first.\n";
pub const SKILL_BODY: &str = "# Tool\n";
pub const SKILL: &str = "---\nname: tool\ndescription: Use the tool.\n---\n\n# Tool\n";
pub const COMMAND: &str = "tool harness hook {harness} {event}";

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A temporary directory: the project at its root, the user's home under
/// `home/`.
pub struct TempTree {
    dir: PathBuf,
}

impl TempTree {
    pub fn empty(name: &str) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("aha-it-{name}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        TempTree { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn write(&self, rel: &str, text: &str) {
        let p = self.dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.dir.join(rel)).unwrap()
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.dir.join(rel).exists()
    }

    /// Every file under the tree with its bytes.
    pub fn files(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        walk(&self.dir, &self.dir, &mut out);
        out
    }

    pub fn tool(&self) -> TestTool {
        TestTool::new(self, false)
    }

    pub fn old_tool(&self) -> TestTool {
        TestTool::new(self, true)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(root, &path, out);
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, fs::read(&path).unwrap());
        }
    }
}

/// A user-scope part the tool writes itself: stands in for `claude mcp
/// add-json --scope user` by keeping the server's JSON in a file.
#[derive(Debug)]
pub struct FakeMcp {
    pub file: PathBuf,
}

impl ExternalPart for FakeMcp {
    fn location(&self) -> String {
        "claude mcp (user)".into()
    }

    fn expected(&self) -> String {
        json!({"command": "tool", "args": ["mcp"]}).to_string()
    }

    fn observe(&self) -> Result<Option<String>> {
        match fs::read_to_string(&self.file) {
            Ok(t) => Ok(Some(t)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::io(&self.file, e)),
        }
    }

    fn write(&self) -> Result<()> {
        // External parts are written before the part files, so nothing else
        // has created the directory yet.
        if let Some(dir) = self.file.parent() {
            fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        fs::write(&self.file, self.expected()).map_err(|e| Error::io(&self.file, e))
    }
}

/// The tool under test: named `tool`. `old` is an older version of it:
/// another instructions block and only the stop hook. `harnesses` are the
/// ones it supports (Claude Code by default).
pub struct TestTool {
    pub dir: PathBuf,
    pub old: bool,
    pub harnesses: Vec<Arc<dyn Harness>>,
}

impl TestTool {
    fn new(tree: &TempTree, old: bool) -> Self {
        TestTool {
            dir: tree.dir.clone(),
            old,
            harnesses: vec![Arc::new(Claude)],
        }
    }

    pub fn with(mut self, harnesses: Vec<Arc<dyn Harness>>) -> Self {
        self.harnesses = harnesses;
        self
    }

    fn home(&self) -> PathBuf {
        self.dir.join("home")
    }
}

impl Tool for TestTool {
    fn name(&self) -> &str {
        "tool"
    }

    fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
        self.harnesses.clone()
    }

    fn integration(&self, scope: Scope) -> Integration {
        let events: &[Event] = if self.old {
            &[Event::Stop]
        } else {
            &[Event::SessionStart, Event::PromptSubmit, Event::Stop]
        };
        let mut i = Integration::new()
            .instructions(if self.old { "Old  text." } else { INSTRUCTIONS })
            .skill(Skill::new("tool", "Use the tool.", SKILL_BODY))
            .mcp_server(McpServer::stdio("tool", "tool", ["mcp"]))
            .allow_command("tool");
        for e in events {
            i = i.hook(Hook::new(*e, COMMAND));
        }
        if scope == Scope::User {
            i = i.part(
                "claude",
                Part::external(
                    "claude-mcp",
                    Arc::new(FakeMcp {
                        file: self.home().join("claude-mcp-user.json"),
                    }),
                ),
            );
        }
        i
    }

    fn root(&self, scope: Scope) -> Result<PathBuf> {
        Ok(match scope {
            Scope::User => self.home(),
            _ => self.dir.clone(),
        })
    }

    fn record_path(&self, scope: Scope) -> Result<PathBuf> {
        Ok(match scope {
            Scope::User => self.home().join(".config/tool/harness.toml"),
            Scope::Local => self.dir.join(".tool/harness.local.toml"),
            _ => self.dir.join(".tool/harness.toml"),
        })
    }

    fn declined_store(&self, scope: Scope) -> Result<Box<dyn DeclinedStore + '_>> {
        Ok(Box::new(match scope {
            Scope::User => TomlDeclined::new(
                self.home().join(".config/tool/config.toml"),
                "~/.config/tool/config.toml",
            ),
            _ => TomlDeclined::new(self.dir.join(".tool/config.toml"), ".tool/config.toml"),
        }))
    }
}

/// The parts of `harness` in `result`.
pub fn parts<'a>(result: &'a InstallResult, harness: &str) -> &'a [PartResult] {
    &result
        .harness(harness)
        .unwrap_or_else(|| panic!("no {harness} in {result:?}"))
        .parts
}

/// The report word of each part of `harness`.
pub fn verbs(result: &InstallResult, harness: &str) -> Vec<String> {
    parts(result, harness)
        .iter()
        .map(|p| p.verb().to_string())
        .collect()
}
