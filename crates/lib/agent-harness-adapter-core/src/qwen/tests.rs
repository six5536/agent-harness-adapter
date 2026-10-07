use std::{fs, time::Duration};

use serde_json::json;

use super::*;
use crate::{
    harness::{Action, EntryMatch, Profile, State},
    integration::{Agent, Hook, McpServer, Skill},
    test_support::temp_dir,
};

fn full() -> Integration {
    Integration::new()
        .instructions("Use t.\n")
        .skill(Skill::new("t", "Use t.", "# T\n"))
        .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
        .hook(
            Hook::new(Event::PreTool, "t hook {harness} {event}")
                .tools(ToolKind::Shell)
                .timeout(Duration::from_secs(9)),
        )
        .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
        .mcp_server(McpServer::http("w", "https://w").header("A", "b"))
        .allow_command("git status")
        .allow_mcp_tool("t", "run")
        .agent(Agent::new("r", "Reviews.", "Review.\n"))
        .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
}

fn names(p: &Profile) -> Vec<String> {
    p.parts()
        .iter()
        .map(|p| format!("{} {}", p.name(), p.location()))
        .collect()
}

// @zen-test: HAR-13_AC-1
// @zen-test: HAR-13_AC-3
// @zen-test: HAR-13_AC-5
// @zen-test: HAR-13_AC-6
// @zen-test: HAR-13_AC-7
#[test]
fn renders_every_item_per_scope() {
    let cx = Context::new("t", Scope::Project, "/nowhere", None);
    let p = Profile::new(Qwen.render(&full(), &cx).unwrap());
    assert_eq!(
        names(&p),
        [
            "instructions AGENTS.md",
            "skills .agents/skills",
            "hooks .qwen/settings.json",
            "mcp .qwen/settings.json",
            "permissions .qwen/settings.json",
            "agents .qwen/agents",
            "commands .qwen/commands"
        ]
    );
    let hooks = p.part("hooks").unwrap().ops();
    assert_eq!(hooks.len(), 7);
    assert_eq!(
        hooks[3],
        MergeOp::group_entries(
            ["hooks", "PreToolUse"],
            "hooks",
            "command",
            EntryMatch::Prefix("t hook ".into()),
            vec![json!({ "matcher": "^run_shell_command$", "hooks": [
                { "type": "command", "command": "t hook qwen pre-tool", "timeout": 9 }
            ]})],
        )
    );
    let mcp = p.part("mcp").unwrap().ops();
    assert_eq!(
        mcp[1].value(),
        &json!({ "httpUrl": "https://w", "headers": { "A": "b" } })
    );
    let rules: Vec<_> = p
        .part("permissions")
        .unwrap()
        .ops()
        .iter()
        .map(|o| o.value().clone())
        .collect();
    assert_eq!(rules, [json!("Bash(git status *)"), json!("mcp__t__run")]);
    let user = Profile::new(
        Qwen.render(&full(), &Context::new("t", Scope::User, "/nowhere", None))
            .unwrap(),
    );
    assert_eq!(user.parts()[0].location(), ".qwen/AGENTS.md");
    assert_eq!(Qwen.scopes(), [Scope::Project, Scope::User]);
    assert_eq!(
        command_md(&Command::new("c", "Checks.", "Check $ARGUMENTS.\n")),
        "---\ndescription: Checks.\n---\n\nCheck {{args}}.\n"
    );
}

// @zen-test: HAR-13_AC-2
#[test]
fn context_files_follow_the_settings() {
    let dir = temp_dir("qwen-context");
    let cx = Context::new("t", Scope::Project, &dir, None);
    assert_eq!(
        Qwen.reads(Item::Instructions, &cx).unwrap(),
        Reads::always(["QWEN.md", "AGENTS.md"])
    );
    fs::create_dir_all(dir.join(".qwen")).unwrap();
    fs::write(
        dir.join(SETTINGS),
        r#"{"context": {"fileName": ["CTX.md"]}}"#,
    )
    .unwrap();
    assert_eq!(location(Item::Instructions, &cx).unwrap(), "CTX.md");
    assert_eq!(
        Qwen.reads(Item::Skills, &cx).unwrap(),
        Reads::always([".agents/skills", ".qwen/skills"])
    );
    fs::remove_dir_all(&dir).unwrap();
}

// @zen-test: HAR-13_AC-4
#[test]
fn input_and_answers_are_claudes() {
    let i = Qwen
        .parse_hook(
            Event::PreTool,
            r#"{"session_id":"s","cwd":"/w","hook_event_name":"PreToolUse","tool_name":"run_shell_command","tool_input":{"command":"ls"}}"#,
        )
        .unwrap();
    assert_eq!(
        (i.session_id.as_deref(), i.cwd.as_deref()),
        (Some("s"), Some("/w"))
    );
    assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
    for (n, k) in [
        ("grep_search", ToolKind::Read),
        ("edit", ToolKind::Write),
        ("mcp__t__run", ToolKind::Mcp),
        ("web_fetch", ToolKind::Other),
    ] {
        assert_eq!(tool_kind(n), k);
    }
    let deny = Qwen
        .answer(Event::PreTool, &Answer::Deny { reason: "r".into() })
        .unwrap();
    assert_eq!(
        deny.stdout,
        r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"r"}}"#
    );
    let stop = Qwen
        .answer(Event::Stop, &Answer::Continue { reason: "r".into() })
        .unwrap();
    assert_eq!(stop.stdout, r#"{"decision":"block","reason":"r"}"#);
}

// @zen-test: HAR-13_AC-8
#[test]
fn notes_after_changes() {
    let part = |name: &str, action| PartResult {
        part: name.into(),
        state: State::Absent,
        action,
        path: String::new(),
        by: None,
    };
    let cx = Context::new("t", Scope::Project, "/nowhere", None);
    assert!(Qwen.notes(&cx, &[part("hooks", None)]).is_empty());
    assert_eq!(
        Qwen.notes(&cx, &[part("mcp", Some(Action::Created))]).len(),
        3
    );
    assert_eq!(
        Qwen.notes(&cx, &[part("hooks", Some(Action::Removed))]),
        ["restart Qwen Code to load the changes"]
    );
}
