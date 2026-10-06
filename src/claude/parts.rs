//! Claude Code's parts: the instructions file it reads and the hook groups
//! of its `settings.json`.

use std::path::Path;

use serde_json::json;

use crate::{
    Error, Result,
    harness::{EntryMatch, MergeOp, Part},
};

/// The instructions file under `root`: `AGENTS.md` when it exists and
/// `CLAUDE.md` is absent or has a line `@AGENTS.md`; otherwise `CLAUDE.md`,
/// created when absent.
// @zen-impl: KIT-11_AC-1
pub fn instructions_file(root: &Path) -> Result<&'static str> {
    let agents = root.join("AGENTS.md");
    let claude = root.join("CLAUDE.md");
    if !agents.is_file() {
        return Ok("CLAUDE.md");
    }
    if !claude.is_file() {
        return Ok("AGENTS.md");
    }
    let text = std::fs::read_to_string(&claude).map_err(|e| Error::io(&claude, e))?;
    Ok(if text.lines().any(|l| l.trim() == "@AGENTS.md") {
        "AGENTS.md"
    } else {
        "CLAUDE.md"
    })
}

fn choose_instructions(root: &Path) -> Result<String> {
    instructions_file(root).map(str::to_string)
}

/// A `region` part in the instructions file Claude Code reads, chosen by
/// [`instructions_file`] when the part is examined.
pub fn instructions(name: impl Into<String>, block: impl Into<String>) -> Part {
    Part::region_chosen(name, choose_instructions, block)
}

/// The tool's command hook for `event` in a `settings.json`:
/// `{"hooks": [{"type": "command", "command": command}]}` under
/// `hooks.<event>`, the tool's hooks being those whose command `owned`
/// matches, e.g. `EntryMatch::Prefix("mytool harness hook ".into())`.
// @zen-impl: KIT-11_AC-5
pub fn hook_command(event: &str, owned: EntryMatch, command: &str) -> MergeOp {
    MergeOp::group_entry(
        ["hooks", event],
        "hooks",
        "command",
        owned,
        json!({ "hooks": [{ "type": "command", "command": command }] }),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::test_support::temp_dir;

    // @zen-test: KIT-11_AC-1
    #[test]
    fn the_instructions_rule() {
        let dir = temp_dir("claude-rule");
        let t = || instructions_file(&dir).unwrap();
        // Neither file: CLAUDE.md, created later.
        assert_eq!(t(), "CLAUDE.md");
        // AGENTS.md alone.
        fs::write(dir.join("AGENTS.md"), "# Agents\n").unwrap();
        assert_eq!(t(), "AGENTS.md");
        // Both, CLAUDE.md without the import.
        fs::write(dir.join("CLAUDE.md"), "# Claude\n").unwrap();
        assert_eq!(t(), "CLAUDE.md");
        // Both, with the import, spaces around it allowed.
        fs::write(dir.join("CLAUDE.md"), "  @AGENTS.md  \n").unwrap();
        assert_eq!(t(), "AGENTS.md");
        assert_eq!(choose_instructions(&dir).unwrap(), "AGENTS.md");
        // CLAUDE.md alone.
        fs::remove_file(dir.join("AGENTS.md")).unwrap();
        assert_eq!(t(), "CLAUDE.md");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_unreadable_claude_md_is_an_io_error() {
        let dir = temp_dir("claude-unreadable");
        fs::write(dir.join("AGENTS.md"), "a").unwrap();
        // A directory named CLAUDE.md is not a file: AGENTS.md alone.
        fs::create_dir(dir.join("CLAUDE.md")).unwrap();
        assert_eq!(instructions_file(&dir).unwrap(), "AGENTS.md");
        fs::remove_dir(dir.join("CLAUDE.md")).unwrap();
        // Invalid UTF-8 cannot be read as text.
        fs::write(dir.join("CLAUDE.md"), [0xff, 0xfe]).unwrap();
        assert!(matches!(instructions_file(&dir), Err(Error::Io { .. })));
        fs::remove_dir_all(&dir).unwrap();
    }

    // @zen-test: KIT-11_AC-5
    #[test]
    fn the_hook_command_group() {
        let op = hook_command("Stop", EntryMatch::Prefix("t hook ".into()), "t hook stop");
        assert_eq!(
            op.value(),
            &json!({ "hooks": [{ "type": "command", "command": "t hook stop" }] })
        );
        assert_eq!(
            op,
            MergeOp::group_entry(
                ["hooks", "Stop"],
                "hooks",
                "command",
                EntryMatch::Prefix("t hook ".into()),
                json!({ "hooks": [{ "type": "command", "command": "t hook stop" }] }),
            )
        );
        assert_eq!(instructions("i", "b").name(), "i");
    }
}
