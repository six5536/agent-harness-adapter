//! Items read from a SOURCE (KIT-17_AC-7): a file or directory the user
//! keeps, e.g. `AGENTS.md`, `.agents/skills`, `.mcp.json`. The item keeps
//! the SOURCE's path, so a harness that loads it where it is gets nothing
//! written (KIT-19_AC-8) and one that can point at it gets a REFERENCE
//! (KIT-23).
// @zen-component: KIT-Source

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

use super::{Agent, Item, McpServer, Skill, skill};
use crate::{Error, Result};

/// Where an item was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Source {
    /// The item.
    pub item: Item,
    /// The SOURCE's path, absolute.
    pub path: PathBuf,
}

impl Source {
    /// `item` read from `path` (made absolute against the working
    /// directory).
    pub fn new(item: Item, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        Source {
            item,
            path: std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
        }
    }

    /// Whether the SOURCE exists.
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

/// The skills in `dir`: each directory in it holding a `SKILL.md`, in name
/// order (hidden ones aside).
// @zen-impl: KIT-17_AC-7
pub(super) fn skills(dir: &Path) -> Result<Vec<Skill>> {
    entries(dir)?
        .into_iter()
        .filter(|p| p.join("SKILL.md").is_file())
        .map(Skill::from_dir)
        .collect()
}

/// The MCP servers of the MCP JSON file `file`: its `mcpServers`.
// @zen-impl: KIT-17_AC-9
pub(super) fn mcp_servers(file: &Path) -> Result<Vec<McpServer>> {
    let display = file.display().to_string();
    let text = skill::read(file)?;
    let value: Value =
        serde_json::from_str(&text).map_err(|e| Error::file(&display, e.to_string()))?;
    let Some(servers) = value.get("mcpServers") else {
        return Ok(Vec::new());
    };
    let servers = servers
        .as_object()
        .ok_or_else(|| Error::file(&display, "`mcpServers` is not an object"))?;
    let mut out = Vec::new();
    for (name, spec) in servers {
        let bad = |what: &str| Error::file(&display, format!("mcpServers.{name}: {what}"));
        let pairs = |key: &str| -> Result<Vec<(String, String)>> {
            match spec.get(key) {
                None => Ok(Vec::new()),
                Some(Value::Object(m)) => m
                    .iter()
                    .map(|(k, v)| {
                        v.as_str()
                            .map(|v| (k.clone(), v.to_string()))
                            .ok_or_else(|| bad(&format!("`{key}.{k}` is not a string")))
                    })
                    .collect(),
                Some(_) => Err(bad(&format!("`{key}` is not an object"))),
            }
        };
        let server = match (spec.get("command"), spec.get("url")) {
            (Some(Value::String(command)), None) => {
                let args: Vec<String> = match spec.get("args") {
                    None => Vec::new(),
                    Some(Value::Array(a)) => a
                        .iter()
                        .map(|v| {
                            v.as_str()
                                .map(str::to_string)
                                .ok_or_else(|| bad("an argument is not a string"))
                        })
                        .collect::<Result<_>>()?,
                    Some(_) => return Err(bad("`args` is not an array")),
                };
                pairs("env")?
                    .into_iter()
                    .fold(McpServer::stdio(name, command, args), |s, (k, v)| {
                        s.env(k, v)
                    })
            }
            (None, Some(Value::String(url))) => pairs("headers")?
                .into_iter()
                .fold(McpServer::http(name, url), |s, (k, v)| s.header(k, v)),
            _ => return Err(bad("needs a `command` or a `url` string, not both")),
        };
        out.push(server);
    }
    Ok(out)
}

/// The agents in `dir`: each `<name>.md`, a markdown agent, in name order.
// @zen-impl: KIT-17_AC-10
pub(super) fn agents(dir: &Path) -> Result<Vec<Agent>> {
    entries(dir)?
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "md"))
        .map(|p| Agent::from_markdown(&skill::read(&p)?, &p))
        .collect()
}

/// The entries of `dir` in name order, hidden ones aside.
fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| Error::io(dir, e))?
        .map(|e| e.map(|e| e.path()).map_err(|e| Error::io(dir, e)))
        .collect::<Result<_>>()?;
    out.retain(|p| {
        !p.file_name()
            .is_some_and(|n| n.to_string_lossy().starts_with('.'))
    });
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;

    use super::*;
    use crate::integration::Transport;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aha-source-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    // @zen-test: KIT-17_AC-7
    #[test]
    fn skills_are_the_directories_with_skill_md() {
        let d = tmp("skills");
        for (n, with) in [
            ("b-skill", true),
            ("a-skill", true),
            ("notes", false),
            (".hidden", true),
        ] {
            fs::create_dir_all(d.join(n)).unwrap();
            if with {
                fs::write(
                    d.join(n).join("SKILL.md"),
                    format!("---\nname: {n}\ndescription: d\n---\nbody\n"),
                )
                .unwrap();
            }
        }
        fs::write(d.join("README.md"), "x").unwrap();
        let names: Vec<String> = skills(&d)
            .unwrap()
            .iter()
            .map(|s| s.name().to_string())
            .collect();
        assert_eq!(names, ["a-skill", "b-skill"]);
    }

    // @zen-test: KIT-17_AC-9
    #[test]
    fn mcp_json_reads_both_transports() {
        let d = tmp("mcp");
        let f = d.join(".mcp.json");
        fs::write(
            &f,
            json!({"mcpServers": {
                "local": {"command": "srv", "args": ["--x"], "env": {"K": "v"}},
                "remote": {"type": "http", "url": "https://x", "headers": {"A": "b"}}
            }})
            .to_string(),
        )
        .unwrap();
        let s = mcp_servers(&f).unwrap();
        assert_eq!(
            s[0].to_json(),
            json!({"command": "srv", "args": ["--x"], "env": {"K": "v"}})
        );
        assert!(matches!(s[1].transport(), Transport::Http { .. }));
        for (bad, says) in [
            ("nope", "expected"),
            (r#"{"mcpServers": []}"#, "not an object"),
            (r#"{"mcpServers": {"x": {}}}"#, "mcpServers.x: needs"),
            (
                r#"{"mcpServers": {"x": {"command": "c", "args": "a"}}}"#,
                "`args` is not an array",
            ),
            (
                r#"{"mcpServers": {"x": {"command": "c", "env": {"K": 1}}}}"#,
                "`env.K` is not a string",
            ),
        ] {
            fs::write(&f, bad).unwrap();
            let e = mcp_servers(&f).unwrap_err().to_string();
            assert!(e.contains(says), "{bad}: {e}");
        }
        fs::write(&f, "{}").unwrap();
        assert!(mcp_servers(&f).unwrap().is_empty());
    }

    // @zen-test: KIT-17_AC-10
    #[test]
    fn agents_keep_their_file() {
        let d = tmp("agents");
        let text =
            "---\nname: reviewer\ndescription: Reviews.\ntools: Read, Grep\n---\n\nReview it.\n";
        fs::write(d.join("reviewer.md"), text).unwrap();
        fs::write(d.join("notes.txt"), "x").unwrap();
        let a = agents(&d).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].name(), a[0].description()), ("reviewer", "Reviews."));
        assert_eq!(a[0].prompt(), "Review it.\n");
        assert_eq!(a[0].to_markdown(&[]), text);
        assert_eq!(
            a[0].to_markdown(&[("model", "inherit"), ("tools", "x")]),
            "---\nname: reviewer\ndescription: Reviews.\ntools: Read, Grep\nmodel: inherit\n---\n\nReview it.\n"
        );
    }
}
