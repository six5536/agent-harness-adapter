//! A manifest's items → an [`Integration`], with TEXT files read.

use std::{fmt, path::Path, time::Duration};

use crate::{
    Error, Result,
    fs::read_text,
    harness::{EntryMatch, MergeOp, Part},
    integration::{Agent, Command, Hook, Integration, McpServer, Skill},
    manifest::file::{HookFile, Items, Match, McpFile, OpFile, PartFile, Text},
};

/// What items are read with: where TEXT files are, how errors name the
/// manifest, and what bridged hooks run.
pub(crate) struct Ctx<'a> {
    /// The manifest's directory.
    pub dir: &'a Path,
    /// The manifest, as errors name it.
    pub display: &'a str,
    /// The tool's name.
    pub name: &'a str,
    /// The command that runs `agent-harness-adapter`.
    pub adapter: &'a str,
    /// The harness ids parts and per-harness commands may name.
    pub harnesses: &'a [String],
}

impl Ctx<'_> {
    fn err(&self, key: &str, message: impl fmt::Display) -> Error {
        Error::file(self.display, format!("`{key}`: {message}"))
    }

    // @zen-impl: AHA-1_AC-5
    fn text(&self, key: &str, text: &Text) -> Result<String> {
        match text {
            Text::Inline(s) => Ok(s.clone()),
            Text::File(f) => {
                let path = self.dir.join(&f.file);
                match read_text(&path) {
                    Ok(Some(s)) => Ok(s),
                    Ok(None) => Err(self.err(key, format!("no file `{}`", f.file))),
                    Err(e) => Err(self.err(key, e)),
                }
            }
        }
    }

    fn harness(&self, key: &str, id: &str) -> Result<()> {
        if self.harnesses.iter().any(|h| h == id) {
            Ok(())
        } else {
            Err(self.err(
                key,
                format!("no harness `{id}` among the manifest's harnesses"),
            ))
        }
    }

    /// The integration of `items`.
    // @zen-impl: AHA-1_AC-4
    pub fn integration(&self, items: Items<'_>) -> Result<Integration> {
        let mut i = Integration::new();
        if let Some(t) = items.instructions {
            i = i.instructions(self.text("instructions", t)?);
        }
        if let Some(m) = items.hook_match {
            i = i.hook_match(entry_match(m));
        }
        for (n, s) in items.skills.into_iter().flatten().enumerate() {
            let key = format!("skills[{n}]");
            let mut skill = Skill::new(
                &s.name,
                &s.description,
                self.text(&format!("{key}.body"), &s.body)?,
            );
            for (path, t) in s.files.iter().flat_map(|f| &f.0) {
                skill = skill.file(path, self.text(&format!("{key}.files.{path}"), t)?);
            }
            i = i.skill(skill);
        }
        for (n, h) in items.hooks.into_iter().flatten().enumerate() {
            i = i.hook(self.hook(&format!("hooks[{n}]"), h)?);
        }
        for (n, m) in items.mcp_servers.into_iter().flatten().enumerate() {
            i = i.mcp_server(self.mcp(&format!("mcp_servers[{n}]"), m)?);
        }
        for c in items.allow_commands.into_iter().flatten() {
            i = i.allow_command(c);
        }
        for t in items.allow_mcp_tools.into_iter().flatten() {
            i = i.allow_mcp_tool(&t.server, &t.tool);
        }
        for (n, a) in items.agents.into_iter().flatten().enumerate() {
            let prompt = self.text(&format!("agents[{n}].prompt"), &a.prompt)?;
            i = i.agent(Agent::new(&a.name, &a.description, prompt));
        }
        for (n, c) in items.commands.into_iter().flatten().enumerate() {
            let prompt = self.text(&format!("commands[{n}].prompt"), &c.prompt)?;
            i = i.command(Command::new(&c.name, &c.description, prompt));
        }
        for (n, p) in items.parts.into_iter().flatten().enumerate() {
            let (harness, part) = self.part(&format!("parts[{n}]"), p)?;
            i = i.part(harness, part);
        }
        Ok(i)
    }

    // @zen-impl: AHA-1_AC-8
    fn hook(&self, key: &str, h: &HookFile) -> Result<Hook> {
        let template = match (&h.command, &h.run) {
            (Some(c), None) => c.clone(),
            (None, Some(run)) => format!(
                "{} hook --tool {}{} {{harness}} {{event}} -- {}",
                escape(self.adapter),
                escape(self.name),
                h.tools
                    .map(|k| format!(" --tools {}", k.as_str()))
                    .unwrap_or_default(),
                escape(run)
            ),
            _ => return Err(self.err(key, "give `command` or `run`, not both")),
        };
        let mut hook = Hook::new(h.event, template);
        if let Some(k) = h.tools {
            hook = hook.tools(k);
        }
        if let Some(s) = h.timeout {
            hook = hook.timeout(Duration::from_secs(s));
        }
        if let Some(m) = &h.owned {
            hook = hook.owned(entry_match(m));
        }
        for (harness, t) in h.commands.iter().flat_map(|c| &c.0) {
            self.harness(&format!("{key}.commands.{harness}"), harness)?;
            hook = hook.command_for(harness, t);
        }
        Ok(hook)
    }

    fn mcp(&self, key: &str, m: &McpFile) -> Result<McpServer> {
        let pairs = |p: &Option<crate::manifest::file::Ordered<String>>| {
            p.iter().flat_map(|p| p.0.clone()).collect::<Vec<_>>()
        };
        match (&m.command, &m.url) {
            (Some(command), None) if m.headers.is_none() => {
                let args = m.args.clone().unwrap_or_default();
                let mut s = McpServer::stdio(&m.name, command, args);
                for (k, v) in pairs(&m.env) {
                    s = s.env(k, v);
                }
                Ok(s)
            }
            (None, Some(url)) if m.args.is_none() && m.env.is_none() => {
                let mut s = McpServer::http(&m.name, url);
                for (k, v) in pairs(&m.headers) {
                    s = s.header(k, v);
                }
                Ok(s)
            }
            _ => Err(self.err(
                key,
                "give `command` (with `args`, `env`) or `url` (with `headers`)",
            )),
        }
    }

    fn part(&self, key: &str, p: &PartFile) -> Result<(String, Part)> {
        Ok(match p {
            PartFile::File {
                harness,
                name,
                dir,
                files,
            } => {
                self.harness(&format!("{key}.harness"), harness)?;
                let mut out = Vec::new();
                for (path, t) in &files.0 {
                    out.push((path.clone(), self.text(&format!("{key}.files.{path}"), t)?));
                }
                (harness.clone(), Part::files(name, dir, out))
            }
            PartFile::Region {
                harness,
                name,
                file,
                block,
            } => {
                self.harness(&format!("{key}.harness"), harness)?;
                let block = self.text(&format!("{key}.block"), block)?;
                (harness.clone(), Part::region(name, file, block))
            }
            PartFile::Merge {
                harness,
                name,
                file,
                ops,
            } => {
                self.harness(&format!("{key}.harness"), harness)?;
                (
                    harness.clone(),
                    Part::merge(name, file, ops.iter().map(merge_op).collect()),
                )
            }
        })
    }
}

fn entry_match(m: &Match) -> EntryMatch {
    match m {
        Match::Prefix(p) => EntryMatch::Prefix(p.clone()),
        Match::Contains(c) => EntryMatch::Contains(c.clone()),
        Match::Any(all) => EntryMatch::Any(all.iter().map(entry_match).collect()),
    }
}

fn merge_op(op: &OpFile) -> MergeOp {
    match op {
        OpFile::ArrayEntry { path, value } => MergeOp::array_entry(path, value.clone()),
        OpFile::ObjectMember { path, key, value } => {
            MergeOp::object_member(path, key, value.clone())
        }
        OpFile::OwnedEntries {
            path,
            field,
            owned,
            entries,
        } => MergeOp::owned_entries(path, field, entry_match(owned), entries.clone()),
        OpFile::GroupEntries {
            path,
            entries,
            field,
            owned,
            groups,
        } => MergeOp::group_entries(path, entries, field, entry_match(owned), groups.clone()),
    }
}

/// `text` with its braces doubled, so a hook template keeps it as written.
fn escape(text: &str) -> String {
    text.replace('{', "{{").replace('}', "}}")
}
