//! Tool calls in Claude Code's terms (KIT-11_AC-9): the name a hook matcher
//! written for Claude Code expects, and the input keys its hooks read.
// @zen-component: KIT-ClaudeNames

use serde_json::Value;

use crate::hook::{ToolCall, ToolKind};

/// `call` named by its kind: shell → `Bash`, read → `Read`, write →
/// `Edit`; MCP and other calls keep their name. The default for a harness
/// without its own table.
// @zen-impl: KIT-11_AC-9
pub fn by_kind(call: &ToolCall) -> ToolCall {
    match call.kind {
        ToolKind::Shell => rename(call, "Bash"),
        ToolKind::Read => rename(call, "Read"),
        ToolKind::Write => rename(call, "Edit"),
        _ => call.clone(),
    }
}

/// `call` named `name`, its input normalised for that name: `command` as
/// text for `Bash`, `file_path` for a file tool; other keys kept.
pub fn rename(call: &ToolCall, name: &str) -> ToolCall {
    let mut input = call.input.clone();
    if let Value::Object(map) = &mut input {
        match name {
            "Bash" => {
                if let Some(c) = shell_command(&call.input) {
                    map.insert("command".into(), Value::String(c));
                }
            }
            "Read" | "Edit" | "Write" | "MultiEdit" | "NotebookEdit" => {
                if let Some(p) = file_path(&call.input) {
                    map.insert("file_path".into(), Value::String(p));
                }
            }
            _ => {}
        }
    }
    ToolCall::new(name, call.kind, input)
}

/// `call` renamed to `name` when the harness's table has one, else as it
/// is: a name not in the table keeps its own (HAR-15_AC-2).
pub fn named(call: &ToolCall, name: Option<&str>) -> ToolCall {
    name.map_or_else(|| call.clone(), |n| rename(call, n))
}

/// A shell call's command as text: `command` or `cmd` as a string; for an
/// array, its last element after `-c` or `-lc`, else its elements joined
/// by spaces.
// @zen-impl: HAR-15_AC-3
pub fn shell_command(input: &Value) -> Option<String> {
    let value = input.get("command").or_else(|| input.get("cmd"))?;
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(parts) => {
            let words: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
            match words.iter().position(|w| *w == "-c" || *w == "-lc") {
                Some(i) if i + 1 < words.len() => Some(words[words.len() - 1].to_string()),
                _ => Some(words.join(" ")),
            }
        }
        _ => None,
    }
}

/// A file tool's path: `file_path`, `path`, `filePath` or `absolute_path`.
pub fn file_path(input: &Value) -> Option<String> {
    ["file_path", "path", "filePath", "absolute_path"]
        .iter()
        .find_map(|k| input.get(*k).and_then(Value::as_str))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // @zen-test: HAR-15_AC-3
    #[test]
    fn shell_commands_become_text() {
        for (input, want) in [
            (json!({"command": "ls -la"}), Some("ls -la")),
            (json!({"cmd": "git status"}), Some("git status")),
            (
                json!({"command": ["bash", "-lc", "rm -rf x"]}),
                Some("rm -rf x"),
            ),
            (json!({"command": ["sh", "-c", "echo hi"]}), Some("echo hi")),
            (json!({"command": ["ls", "-la"]}), Some("ls -la")),
            (json!({"command": 3}), None),
            (json!({}), None),
        ] {
            assert_eq!(shell_command(&input).as_deref(), want, "{input}");
        }
    }

    #[test]
    fn file_paths_from_any_key() {
        for k in ["file_path", "path", "filePath", "absolute_path"] {
            assert_eq!(file_path(&json!({ k: "/a" })).as_deref(), Some("/a"));
        }
        assert_eq!(file_path(&json!({"x": 1})), None);
    }

    // @zen-test: KIT-11_AC-9
    #[test]
    fn by_kind_names_and_normalises() {
        let shell = ToolCall::new("exec", ToolKind::Shell, json!({"cmd": "ls", "cwd": "/p"}));
        let c = by_kind(&shell);
        assert_eq!(c.name, "Bash");
        assert_eq!(c.input, json!({"cmd": "ls", "cwd": "/p", "command": "ls"}));
        let read = by_kind(&ToolCall::new("view", ToolKind::Read, json!({"path": "a"})));
        assert_eq!(
            (read.name.as_str(), &read.input["file_path"]),
            ("Read", &json!("a"))
        );
        let write = by_kind(&ToolCall::new("x", ToolKind::Write, json!("raw")));
        assert_eq!((write.name.as_str(), &write.input), ("Edit", &json!("raw")));
        let other = ToolCall::new("todo", ToolKind::Other, json!({}));
        assert_eq!(by_kind(&other), other);
        let mcp = ToolCall::new("mcp__s__t", ToolKind::Mcp, json!({}));
        assert_eq!(by_kind(&mcp), mcp);
    }

    fn calls() -> impl proptest::strategy::Strategy<Value = ToolCall> {
        use proptest::prelude::*;
        let names = proptest::sample::select(vec![
            "Bash",
            "shell",
            "exec",
            "run_shell_command",
            "bash",
            "Execute",
            "read_file",
            "view",
            "grep",
            "find",
            "list",
            "apply_patch",
            "replace",
            "Create",
            "write_file",
            "edit",
            "notebook_edit",
            "mcp__s__t",
            "todo",
            "x",
        ]);
        let kinds = proptest::sample::select(vec![
            ToolKind::Shell,
            ToolKind::Read,
            ToolKind::Write,
            ToolKind::Mcp,
            ToolKind::Other,
        ]);
        (names, kinds, "[a-z ]{0,12}")
            .prop_map(|(n, k, c)| ToolCall::new(n, k, json!({ "command": c, "path": "p" })))
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(64))]
        // @zen-test: HAR_P-4
        #[test]
        fn claude_names_are_total(call in calls()) {
            for h in crate::harness::builtin() {
                let named = h.claude_tool(&call);
                proptest::prop_assert!(!named.name.is_empty());
                if named.name == "Bash" {
                    proptest::prop_assert_eq!(&named.input["command"], &call.input["command"]);
                }
                if h.id() == "claude" {
                    proptest::prop_assert_eq!(&named, &call);
                }
            }
        }
    }

    // @zen-test: HAR-15_AC-2
    // @zen-test: HAR-15_AC-1
    #[test]
    fn each_harness_names_its_tools() {
        let cases = [
            ("codex", "local_shell", "Bash"),
            ("codex", "apply_patch", "Edit"),
            ("factory", "Execute", "Bash"),
            ("factory", "Create", "Write"),
            ("factory", "ApplyPatch", "Edit"),
            ("gemini", "run_shell_command", "Bash"),
            ("gemini", "search_file_content", "Grep"),
            ("gemini", "list_directory", "LS"),
            ("gemini", "replace", "Edit"),
            ("gemini", "write_file", "Write"),
            ("copilot", "powershell", "Bash"),
            ("copilot", "view", "Read"),
            ("copilot", "create", "Write"),
            ("cursor", "run_terminal_cmd", "Bash"),
            ("cursor", "grep_search", "Grep"),
            ("cursor", "edit_file", "Edit"),
            ("cursor", "write", "Write"),
            ("cursor", "read_file", "Read"),
            ("cursor", "todo", "todo"),
            ("pi", "find", "Glob"),
            ("pi", "ls", "LS"),
            ("pi", "write", "Write"),
            ("opencode", "list", "LS"),
            ("opencode", "apply_patch", "Edit"),
            ("kilo", "bash", "Bash"),
            ("qwen", "grep_search", "Grep"),
            ("qwen", "notebook_edit", "NotebookEdit"),
            ("devin", "exec", "Bash"),
            ("devin", "notebook_read", "Read"),
            ("devin", "mcp__s__t", "mcp__s__t"),
            ("claude", "MultiEdit", "MultiEdit"),
            ("codex", "unknown_tool", "unknown_tool"),
        ];
        for (id, native, want) in cases {
            let h = crate::harness::find(id).unwrap();
            let call = ToolCall::new(native, ToolKind::Other, json!({}));
            assert_eq!(h.claude_tool(&call).name, want, "{id} {native}");
        }
        let programs: Vec<Option<String>> = crate::harness::builtin()
            .iter()
            .map(|h| h.program().map(String::from))
            .collect();
        let want = [
            "claude",
            "codex",
            "droid",
            "gemini",
            "copilot",
            "cursor-agent",
            "pi",
            "opencode",
            "kilo",
            "qwen",
            "devin",
            "",
        ];
        let want: Vec<Option<String>> = want
            .iter()
            .map(|p| (!p.is_empty()).then(|| p.to_string()))
            .collect();
        assert_eq!(programs, want);
    }
}
