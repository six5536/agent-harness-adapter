use std::time::Duration;

use serde_json::json;

use super::*;
use crate::{
    harness::{Action, EntryMatch, Profile, State},
    integration::{Agent, Command, Hook, McpServer, Skill},
};

fn full() -> Integration {
    Integration::new()
        .instructions("Use t.\n")
        .skill(Skill::new("t", "Use t.", "# T\n"))
        .hook(Hook::new(Event::Stop, "t hook {harness} {event}"))
        .hook(Hook::new(Event::PreCompact, "t hook {harness} {event}"))
        .hook(
            Hook::new(Event::PreTool, "t hook {harness} {event}")
                .tools(ToolKind::Shell)
                .timeout(Duration::from_secs(9)),
        )
        .mcp_server(McpServer::stdio("t", "t", ["mcp"]))
        .allow_command("git status")
        .allow_mcp_tool("t", "run")
        .agent(Agent::new("r", "Reviews.", "Review.\n"))
        .command(Command::new("c", "Checks.", "Check.\n"))
}

fn rendered(scope: Scope) -> Profile {
    let cx = Context::new("t", scope, "/nowhere", None);
    Profile::new(Devin.render(&full(), &cx).unwrap())
}

fn names(p: &Profile) -> Vec<String> {
    p.parts()
        .iter()
        .map(|p| format!("{} {}", p.name(), p.location()))
        .collect()
}

// @zen-test: HAR-11_AC-1
// @zen-test: HAR-11_AC-2
// @zen-test: HAR-11_AC-3
// @zen-test: HAR-11_AC-6
// @zen-test: HAR-11_AC-7
// @zen-test: HAR-11_AC-8
#[test]
fn renders_every_item_it_takes_per_scope() {
    let p = rendered(Scope::Project);
    assert_eq!(
        names(&p),
        [
            "instructions AGENTS.md",
            "skills .agents/skills",
            "hooks .devin/hooks.v1.json",
            "mcp .devin/mcp_config.json",
            "permissions .devin/config.json",
            "agents .devin/agents"
        ]
    );
    let hooks = p.part("hooks").unwrap().ops();
    // One op per Devin event, the hooks object at the top of the file; no
    // event before compaction.
    assert_eq!(hooks.len(), 6);
    assert_eq!(
        hooks[3],
        MergeOp::group_entries(
            ["PreToolUse"],
            "hooks",
            "command",
            EntryMatch::Prefix("t hook ".into()),
            vec![json!({ "matcher": "^exec$", "hooks": [
                { "type": "command", "command": "t hook devin pre-tool", "timeout": 9 }
            ]})],
        )
    );
    let rules: Vec<_> = p
        .part("permissions")
        .unwrap()
        .ops()
        .iter()
        .map(|o| o.value().clone())
        .collect();
    assert_eq!(rules, [json!("Exec(git status)"), json!("mcp__t__run")]);
    let user = rendered(Scope::User);
    assert_eq!(
        names(&user),
        [
            "instructions .config/devin/AGENTS.md",
            "skills .agents/skills",
            "hooks .config/devin/config.json",
            "mcp .config/devin/mcp_config.json",
            "permissions .config/devin/config.json",
            "agents .config/devin/agents"
        ]
    );
    assert_eq!(
        user.part("hooks").unwrap().ops()[0].path(),
        ["hooks", "SessionStart"]
    );
    assert_eq!(Devin.scopes(), [Scope::Project, Scope::User]);
}

#[test]
fn reads_claude_codes_files_too() {
    let cx = Context::new("t", Scope::Project, "/nowhere", None);
    let reads = |item| Devin.reads(item, &cx).unwrap();
    assert!(reads(Item::Instructions).loads("CLAUDE.md"));
    assert!(reads(Item::Skills).loads(".claude/skills"));
    assert!(reads(Item::Mcp).loads(".mcp.json"));
    let hooks = reads(Item::Hooks);
    assert!(hooks.loads(".devin/hooks.v1.json") && !hooks.loads(".claude/settings.json"));
    assert!(hooks.may_load(".claude/settings.json"));
    assert!(reads(Item::Agents).may_load(".claude/agents"));
    let user = Context::new("t", Scope::User, "/nowhere", None);
    assert!(
        Devin
            .reads(Item::Instructions, &user)
            .unwrap()
            .may_load(".claude/CLAUDE.md")
    );
    assert!(
        Devin
            .reads(Item::Skills, &user)
            .unwrap()
            .loads(".config/devin/skills")
    );
    assert!(
        Devin
            .reads(Item::Agents, &user)
            .unwrap()
            .may_load(".claude/agents")
    );
}

// @zen-test: HAR-11_AC-4
// @zen-test: HAR-11_AC-5
#[test]
fn input_and_answers() {
    let i = Devin
        .parse_hook(
            Event::PreTool,
            r#"{"hook_event_name":"PreToolUse","session_id":"s","prompt_id":"p","tool_name":"exec","tool_input":{"command":"ls"}}"#,
        )
        .unwrap();
    assert_eq!(i.session_id.as_deref(), Some("s"));
    assert_eq!(i.tool.unwrap().kind, ToolKind::Shell);
    let stop = Devin
        .parse_hook(
            Event::Stop,
            r#"{"stop_hook_active":true,"last_assistant_message":"done","cwd":"/w"}"#,
        )
        .unwrap();
    assert!(stop.continuing);
    assert_eq!(
        (stop.last_message.as_deref(), stop.cwd.as_deref()),
        (Some("done"), Some("/w"))
    );
    for (n, k) in [
        ("grep", ToolKind::Read),
        ("apply_patch", ToolKind::Write),
        ("mcp__t__run", ToolKind::Mcp),
        ("webfetch", ToolKind::Other),
    ] {
        assert_eq!(tool_kind(n), k);
    }
    let out = |e, a: Answer| Devin.answer(e, &a).map(|o| o.stdout);
    assert_eq!(
        out(Event::Stop, Answer::Allow { stderr: None }).unwrap(),
        "{}"
    );
    for e in [Event::PreTool, Event::PromptSubmit] {
        assert_eq!(
            out(e, Answer::Deny { reason: "r".into() }).unwrap(),
            r#"{"decision":"block","reason":"r"}"#
        );
    }
    assert_eq!(
        out(Event::Stop, Answer::Continue { reason: "r".into() }).unwrap(),
        r#"{"decision":"block","reason":"r"}"#
    );
    assert_eq!(
        out(Event::PostTool, Answer::Context { text: "c".into() }).unwrap(),
        r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"c"}}"#
    );
    assert!(out(Event::Stop, Answer::Context { text: "c".into() }).is_err());
}

// @zen-test: HAR-11_AC-9
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
    assert!(Devin.notes(&cx, &[part(None)]).is_empty());
    assert_eq!(Devin.notes(&cx, &[part(Some(Action::Created))]).len(), 2);
    assert_eq!(
        Devin.notes(&cx, &[part(Some(Action::Removed))]),
        ["start a new Devin session to load the changes"]
    );
}
