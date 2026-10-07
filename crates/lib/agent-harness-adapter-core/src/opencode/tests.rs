use std::{fs, time::Duration};

use super::*;
use crate::{
    harness::{Action, Profile, State},
    hook::ToolKind,
    integration::{Agent, Command, Hook, McpServer, Skill},
    test_support::temp_dir,
};

fn plugin(tool: &str, integration: &Integration) -> String {
    plugin::plugin(ID, "OpenCode", "@opencode-ai/plugin", tool, integration)
}

fn instructions_file(cx: &Context) -> String {
    LAYOUT.location(Item::Instructions, cx)
}

fn full() -> Integration {
    Integration::new()
        .instructions("Use t.\n")
        .skill(Skill::new("t", "Use t.", "# T\n"))
        .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
        .hook(Hook::new(Event::PreTool, "t hook {harness} {event}").timeout(Duration::from_secs(5)))
        .mcp_server(McpServer::stdio("t", "t", ["mcp"]).env("K", "v"))
        .mcp_server(McpServer::http("w", "https://w.example").header("H", "h"))
        .allow_command("t")
        .allow_mcp_tool("t", "run")
        .agent(Agent::new("r", "Reviews.", "Review.\n"))
        .command(Command::new("c", "Checks.", "Check $ARGUMENTS.\n"))
}

// @zen-test: HAR-10_AC-1
// @zen-test: HAR-10_AC-3
// @zen-test: HAR-10_AC-5
// @zen-test: HAR-10_AC-6
// @zen-test: HAR-10_AC-7
#[test]
fn renders_every_item_per_scope() {
    let names = |scope| -> Vec<String> {
        let cx = Context::new("t", scope, "/nowhere", None);
        OpenCode
            .render(&full(), &cx)
            .unwrap()
            .iter()
            .map(|p| format!("{} {}", p.name(), p.location()))
            .collect()
    };
    assert_eq!(
        names(Scope::Project),
        [
            "instructions AGENTS.md",
            "skills .agents/skills",
            "hooks .opencode/plugins",
            "mcp opencode.json",
            "permissions opencode.json",
            "agents .opencode/agents",
            "commands .opencode/commands"
        ]
    );
    assert_eq!(
        names(Scope::User),
        [
            "instructions .config/opencode/AGENTS.md",
            "skills .agents/skills",
            "hooks .config/opencode/plugins",
            "mcp .config/opencode/opencode.json",
            "permissions .config/opencode/opencode.json",
            "agents .config/opencode/agents",
            "commands .config/opencode/commands"
        ]
    );
    assert_eq!(OpenCode.scopes(), [Scope::Project, Scope::User]);
    let cx = Context::new("t", Scope::Project, "/nowhere", None);
    let p = Profile::new(OpenCode.render(&full(), &cx).unwrap());
    assert!(p.part("hooks").unwrap().same_content(&Part::files(
        "hooks",
        ".opencode/plugins",
        vec![("t.ts".into(), plugin("t", &full()))]
    )));
    // No allowed command: no permissions part.
    let none = Integration::new().allow_mcp_tool("t", "run");
    assert!(OpenCode.render(&none, &cx).unwrap().is_empty());
}

#[test]
fn the_plugin_carries_the_hooks() {
    let ts = plugin("my\ntool", &full());
    assert!(
        ts.starts_with("// Written by my tool harness install"),
        "{ts}"
    );
    assert!(ts.contains("const TOOL: string = \"my\\ntool\";"), "{ts}");
    assert!(
        ts.contains(r#"{"event":"pre-tool","command":"t hook opencode pre-tool","timeout":5000}"#),
        "{ts}"
    );
    assert!(
        ts.contains(r#"{"event":"stop","command":"t hook opencode stop"}"#),
        "{ts}"
    );
    assert!(!ts.contains("__"), "every placeholder filled: {ts}");
}

// @zen-test: HAR-10_AC-2
#[test]
fn the_first_instructions_file_wins() {
    let dir = temp_dir("opencode-rule");
    let cx = Context::new("t", Scope::Project, &dir, None);
    assert_eq!(instructions_file(&cx), "AGENTS.md");
    fs::write(dir.join("CLAUDE.md"), "c").unwrap();
    assert_eq!(instructions_file(&cx), "CLAUDE.md");
    fs::write(dir.join("AGENTS.md"), "a").unwrap();
    assert_eq!(instructions_file(&cx), "AGENTS.md");
    assert_eq!(
        OpenCode.reads(Item::Instructions, &cx).unwrap(),
        Reads::always(["AGENTS.md"])
    );
    let user = Context::new("t", Scope::User, &dir, None);
    assert_eq!(instructions_file(&user), ".config/opencode/AGENTS.md");
    fs::create_dir_all(dir.join(".claude")).unwrap();
    fs::write(dir.join(".claude/CLAUDE.md"), "c").unwrap();
    assert_eq!(instructions_file(&user), ".claude/CLAUDE.md");
    fs::create_dir_all(dir.join(".config/opencode")).unwrap();
    fs::write(dir.join(".config/opencode/AGENTS.md"), "a").unwrap();
    assert_eq!(instructions_file(&user), ".config/opencode/AGENTS.md");
    assert_eq!(
        OpenCode.reads(Item::Skills, &user).unwrap(),
        Reads::always([
            ".agents/skills",
            ".claude/skills",
            ".config/opencode/skills"
        ])
    );
    fs::remove_dir_all(&dir).unwrap();
}

// @zen-test: HAR-10_AC-4
#[test]
fn input_and_answers() {
    let i = OpenCode
        .parse_hook(
            Event::PreTool,
            r#"{"event":"pre-tool","session_id":"s","cwd":"/w","tool_name":"bash","tool_input":{"command":"ls"}}"#,
        )
        .unwrap();
    assert_eq!(
        (i.session_id.as_deref(), i.cwd.as_deref()),
        (Some("s"), Some("/w"))
    );
    assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
    assert!(
        OpenCode
            .parse_hook(Event::Stop, r#"{"continuing":true}"#)
            .unwrap()
            .continuing
    );
    for (n, k) in [
        ("grep", ToolKind::Read),
        ("apply_patch", ToolKind::Write),
        ("t_run", ToolKind::Other),
    ] {
        assert_eq!(plugin::tool_kind(n), k);
    }
    let out = |e, a: Answer| OpenCode.answer(e, &a).map(|o| o.stdout);
    assert_eq!(
        out(Event::PreTool, Answer::Deny { reason: "r".into() }).unwrap(),
        r#"{"answer":"deny","reason":"r"}"#
    );
    assert_eq!(
        out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
        r#"{"answer":"continue","reason":"r"}"#
    );
    assert_eq!(
        out(Event::SessionStart, Answer::Context { text: "c".into() }).unwrap(),
        r#"{"answer":"context","text":"c"}"#
    );
    assert!(out(Event::PromptSubmit, Answer::Deny { reason: "r".into() }).is_err());
}

// @zen-test: HAR-10_AC-8
#[test]
fn notes_after_changes() {
    let part = |action| PartResult {
        part: "hooks".into(),
        state: State::Absent,
        action,
        path: String::new(),
        by: None,
    };
    let cx = Context::new("t", Scope::Project, "/nowhere", None);
    assert!(OpenCode.notes(&cx, &[part(None)]).is_empty());
    assert_eq!(
        OpenCode.notes(&cx, &[part(Some(Action::Removed))]),
        ["restart OpenCode to load the changes"]
    );
}
