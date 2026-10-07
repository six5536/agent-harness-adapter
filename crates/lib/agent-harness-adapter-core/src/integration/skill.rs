//! Agent skills: declared, or read from a skill directory.

use std::{fs, path::Path};

use super::items::yaml_scalar;
use crate::{Error, Result};

/// An agent skill: `<name>/SKILL.md` with frontmatter `name` and
/// `description`, plus extra files beside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skill {
    name: String,
    description: String,
    body: String,
    files: Vec<(String, String)>,
    /// `SKILL.md` as read from a skill directory, written unchanged.
    skill_md: Option<String>,
}

impl Skill {
    /// A skill named `name` (lowercase, digits and `-`), described by
    /// `description`, with `body` after the frontmatter.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Skill {
            name: name.into(),
            description: description.into(),
            body: body.into(),
            files: Vec::new(),
            skill_md: None,
        }
    }

    /// The skill in directory `dir`: its `SKILL.md`, written unchanged, with
    /// the name and description of its frontmatter, and every other file
    /// under `dir` (hidden ones aside) as an extra file.
    ///
    /// # Errors
    ///
    /// [`Error::File`] when `SKILL.md` is missing, its frontmatter has no
    /// `name` of lowercase letters, digits and `-`, or a file is not UTF-8
    /// text; [`Error::Io`] when a file cannot be read.
    // @zen-impl: KIT-17_AC-5
    // @zen-impl: KIT-17_AC-6
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let path = dir.join("SKILL.md");
        let display = path.display().to_string();
        if !path.is_file() {
            return Err(Error::file(display, "no such file"));
        }
        let skill_md = read(&path)?;
        let (front, body) = split_frontmatter(&skill_md).ok_or_else(|| {
            Error::file(&display, "no frontmatter between `---` lines at the top")
        })?;
        let name = field(front, "name").filter(|n| is_name(n)).ok_or_else(|| {
            Error::file(
                &display,
                "the frontmatter has no `name` of lowercase letters, digits and `-`",
            )
        })?;
        let description = field(front, "description").unwrap_or_default();
        let body = body.strip_prefix('\n').unwrap_or(body).to_string();
        let mut files = Vec::new();
        collect(dir, "", &mut files)?;
        Ok(Skill {
            name,
            description,
            body,
            files,
            skill_md: Some(skill_md),
        })
    }

    /// With an extra file at `path` (relative to the skill's directory).
    #[must_use]
    pub fn file(mut self, path: impl Into<String>, text: impl Into<String>) -> Self {
        self.files.push((path.into(), text.into()));
        self
    }

    /// The skill's name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The skill's description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// The skill's body.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// The extra files: (path relative to the skill's directory, text).
    pub fn files(&self) -> &[(String, String)] {
        &self.files
    }

    /// The skill's files under a skills directory: `<name>/SKILL.md` (as
    /// read from a skill directory, else rendered), then `<name>/<path>` for
    /// each extra file.
    pub fn dir_files(&self) -> Vec<(String, String)> {
        let skill_md = self.skill_md.clone().unwrap_or_else(|| {
            format!(
                "---\nname: {}\ndescription: {}\n---\n\n{}",
                self.name,
                yaml_scalar(&self.description),
                self.body
            )
        });
        let mut out = vec![(format!("{}/SKILL.md", self.name), skill_md)];
        out.extend(
            self.files
                .iter()
                .map(|(p, t)| (format!("{}/{p}", self.name), t.clone())),
        );
        out
    }
}

/// The text of `path`, refused when it is not UTF-8.
fn read(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|e| Error::io(path, e))?;
    String::from_utf8(bytes).map_err(|_| Error::file(path.display().to_string(), "not UTF-8 text"))
}

/// Every file under `dir` but its top `SKILL.md` and hidden entries, in
/// path order, as (`rel`-prefixed path with `/`, text).
fn collect(dir: &Path, rel: &str, out: &mut Vec<(String, String)>) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| Error::io(dir, e))?
        .collect::<std::io::Result<_>>()
        .map_err(|e| Error::io(dir, e))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let path = e.path();
        let at = format!("{rel}{name}");
        if name.starts_with('.') || at == "SKILL.md" {
            continue;
        }
        if path.is_dir() {
            collect(&path, &format!("{at}/"), out)?;
        } else {
            out.push((at, read(&path)?));
        }
    }
    Ok(())
}

/// The frontmatter of `text` (between a first line `---` and the next
/// `---` line) and the text after it.
fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))?;
    let mut at = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..at], &rest[at + line.len()..]));
        }
        at += line.len();
    }
    None
}

/// The value of the top-level frontmatter key `key`: a plain or quoted
/// scalar, or a `|` / `>` block (its lines joined by a line break or a
/// space).
fn field(front: &str, key: &str) -> Option<String> {
    let mut lines = front.lines();
    while let Some(line) = lines.next() {
        let Some(value) = line.strip_prefix(key).and_then(|r| r.strip_prefix(':')) else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || value.starts_with(['|', '>']) {
            let sep = if value.starts_with('|') { "\n" } else { " " };
            let block: Vec<&str> = lines
                .take_while(|l| l.trim().is_empty() || l.starts_with([' ', '\t']))
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .collect();
            return Some(block.join(sep));
        }
        return Some(unquote(value));
    }
    None
}

fn unquote(value: &str) -> String {
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        serde_json::from_str(value).unwrap_or_else(|_| value[1..value.len() - 1].to_string())
    } else if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        value[1..value.len() - 1].replace("''", "'")
    } else {
        value.to_string()
    }
}

/// A skill name: lowercase letters, digits and `-`.
fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    #[test]
    fn a_skill_renders_its_directory() {
        let s = Skill::new("tool", "Use the tool: always.", "# Tool\n").file("ref.md", "r\n");
        assert_eq!(
            s.dir_files(),
            vec![
                (
                    "tool/SKILL.md".to_string(),
                    "---\nname: tool\ndescription: \"Use the tool: always.\"\n---\n\n# Tool\n"
                        .to_string()
                ),
                ("tool/ref.md".to_string(), "r\n".to_string()),
            ]
        );
        assert_eq!(
            (s.name(), s.description(), s.body(), s.files().len()),
            ("tool", "Use the tool: always.", "# Tool\n", 1)
        );
    }

    // @zen-test: KIT-17_AC-5
    #[test]
    fn a_skill_directory_is_read_as_it_is() {
        let dir = temp_dir("skill-dir");
        let skill_md = "---\nname: review\ndescription: >\n  Reviews code\n  carefully.\nlicense: MIT\nallowed-tools: Read\n---\n\n# Review\n";
        fs::write(dir.join("SKILL.md"), skill_md).unwrap();
        fs::create_dir_all(dir.join("scripts/sub")).unwrap();
        fs::write(dir.join("scripts/sub/b.sh"), "b\n").unwrap();
        fs::write(dir.join("a.md"), "a\n").unwrap();
        fs::write(dir.join(".DS_Store"), [0xff, 0xfe]).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        fs::write(dir.join(".git/x"), "x").unwrap();
        let s = Skill::from_dir(&dir).unwrap();
        assert_eq!(
            (s.name(), s.description(), s.body()),
            ("review", "Reviews code carefully.", "# Review\n")
        );
        assert_eq!(
            s.dir_files(),
            vec![
                ("review/SKILL.md".to_string(), skill_md.to_string()),
                ("review/a.md".to_string(), "a\n".to_string()),
                ("review/scripts/sub/b.sh".to_string(), "b\n".to_string()),
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: KIT-17_AC-6
    #[test]
    fn a_bad_skill_directory_is_refused_naming_the_file() {
        let dir = temp_dir("skill-bad");
        let refused = |dir: &Path| Skill::from_dir(dir).unwrap_err().to_string();
        assert!(
            refused(&dir).contains("SKILL.md: no such file"),
            "{}",
            refused(&dir)
        );
        fs::write(dir.join("SKILL.md"), "# no frontmatter\n").unwrap();
        assert!(refused(&dir).contains("no frontmatter"));
        fs::write(dir.join("SKILL.md"), "---\nname: Bad Name\n---\n").unwrap();
        assert!(refused(&dir).contains("no `name`"));
        fs::write(
            dir.join("SKILL.md"),
            "---\nname: \"ok\"\ndescription: 'it''s'\n---\nx",
        )
        .unwrap();
        fs::write(dir.join("logo.png"), [0x89, 0x50, 0xff]).unwrap();
        let e = refused(&dir);
        assert!(
            e.contains("logo.png") && e.contains("not UTF-8 text"),
            "{e}"
        );
        fs::remove_file(dir.join("logo.png")).unwrap();
        let s = Skill::from_dir(&dir).unwrap();
        assert_eq!((s.name(), s.description(), s.body()), ("ok", "it's", "x"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn frontmatter_fields() {
        let front = "name: x\ndescription: |\n  one\n  two\nother: \"a\\tb\"\n";
        assert_eq!(field(front, "description").unwrap(), "one\ntwo");
        assert_eq!(field(front, "other").unwrap(), "a\tb");
        assert_eq!(field(front, "missing"), None);
        assert_eq!(
            split_frontmatter("---\r\na: 1\r\n---\r\nrest"),
            Some(("a: 1\r\n", "rest"))
        );
        assert_eq!(split_frontmatter("---\nnever closed\n"), None);
        assert!(is_name("a-1") && !is_name("") && !is_name("../x"));
    }
}
