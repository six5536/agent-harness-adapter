use std::{fs, path::Path, time::Duration};

use serde_json::json;

use super::*;
use crate::{
    harness::{EntryMatch, MergeOp, Part},
    hook::{Event, ToolKind},
    integration::{Agent, Command, Hook, McpServer, Skill},
    test_support::temp_dir,
};

fn load(dir: &Path, name: &str, text: &str) -> Result<Manifest> {
    let path = dir.join(name);
    fs::write(&path, text).unwrap();
    Manifest::load(&path)
}

fn err(dir: &Path, text: &str) -> String {
    load(dir, "m.toml", text).unwrap_err().to_string()
}

const FULL: &str = r##"
version = 1
name = "mytool"
harnesses = ["claude", "codex"]
adapter = "/opt/agent-harness-adapter"
instructions = { file = "instructions.md" }
hook_match = { prefix = "mytool " }
allow_commands = ["mytool"]
allow_mcp_tools = [{ server = "mytool", tool = "run" }]

[[skills]]
name = "mytool"
description = "Use mytool."
body = "# Mytool\n"
files = { "ref.md" = { file = "ref.md" }, "a.md" = "a" }

[[hooks]]
event = "pre-tool"
run = "mytool decide --x {y}"
tools = "shell"
timeout = 10
commands = { claude = "$CLAUDE_PROJECT_DIR/mytool hook {harness} {event}" }
owned = { any = [{ contains = "mytool" }, { prefix = "x" }] }

[[hooks]]
event = "stop"
command = "mytool hook {harness} {event}"

[[mcp_servers]]
name = "mytool"
command = "mytool"
args = ["mcp"]
env = { B = "2", A = "1" }

[[mcp_servers]]
name = "web"
url = "https://x/mcp"
headers = { Authorization = "t" }

[[agents]]
name = "reviewer"
description = "Reviews."
prompt = "You review.\n"

[[commands]]
name = "check"
description = "Run the check."
prompt = "Check $ARGUMENTS.\n"

[[parts]]
harness = "claude"
kind = "merge"
name = "statusline"
file = ".claude/settings.json"
ops = [
  { op = "object_member", path = [], key = "statusLine", value = { type = "command", command = "mytool status" } },
  { op = "array_entry", path = ["a"], value = 1 },
  { op = "owned_entries", path = ["b"], field = "c", owned = { prefix = "m" }, entries = [{ c = "m1" }] },
  { op = "group_entries", path = ["d"], entries = "hooks", field = "command", owned = { contains = "m" }, groups = [] },
]

[[parts]]
harness = "codex"
kind = "region"
name = "notes"
file = "NOTES.md"
block = { file = "notes.md" }

[[parts]]
harness = "claude"
kind = "file"
name = "extra"
dir = ".claude/extra"
files = { "x.md" = "x" }

[scopes.user]
record = ".config/mytool/harness.toml"
hooks = []
"##;

fn full_builder() -> Integration {
    Integration::new()
        .instructions("Use mytool.\n")
        .hook_match(EntryMatch::Prefix("mytool ".into()))
        .skill(
            Skill::new("mytool", "Use mytool.", "# Mytool\n")
                .file("ref.md", "ref\n")
                .file("a.md", "a"),
        )
        .hook(
            Hook::new(
                Event::PreTool,
                "/opt/agent-harness-adapter hook --tool mytool --tools shell {harness} {event} -- mytool decide --x {{y}}",
            )
            .tools(ToolKind::Shell)
            .timeout(Duration::from_secs(10))
            .owned(EntryMatch::Any(vec![
                EntryMatch::Contains("mytool".into()),
                EntryMatch::Prefix("x".into()),
            ]))
            .command_for(
                "claude",
                "$CLAUDE_PROJECT_DIR/mytool hook {harness} {event}",
            ),
        )
        .hook(Hook::new(Event::Stop, "mytool hook {harness} {event}"))
        .mcp_server(
            McpServer::stdio("mytool", "mytool", ["mcp"])
                .env("B", "2")
                .env("A", "1"),
        )
        .mcp_server(McpServer::http("web", "https://x/mcp").header("Authorization", "t"))
        .allow_command("mytool")
        .allow_mcp_tool("mytool", "run")
        .agent(Agent::new("reviewer", "Reviews.", "You review.\n"))
        .command(Command::new(
            "check",
            "Run the check.",
            "Check $ARGUMENTS.\n",
        ))
        .part(
            "claude",
            Part::merge(
                "statusline",
                ".claude/settings.json",
                vec![
                    MergeOp::object_member(
                        Vec::<String>::new(),
                        "statusLine",
                        json!({"type": "command", "command": "mytool status"}),
                    ),
                    MergeOp::array_entry(["a"], 1),
                    MergeOp::owned_entries(
                        ["b"],
                        "c",
                        EntryMatch::Prefix("m".into()),
                        vec![json!({"c": "m1"})],
                    ),
                    MergeOp::group_entries(
                        ["d"],
                        "hooks",
                        "command",
                        EntryMatch::Contains("m".into()),
                        vec![],
                    ),
                ],
            ),
        )
        .part("codex", Part::region("notes", "NOTES.md", "notes\n"))
        .part(
            "claude",
            Part::files("extra", ".claude/extra", vec![("x.md".into(), "x".into())]),
        )
}

fn full(dir: &Path) -> Manifest {
    fs::write(dir.join("instructions.md"), "Use mytool.\n").unwrap();
    fs::write(dir.join("ref.md"), "ref\n").unwrap();
    fs::write(dir.join("notes.md"), "notes\n").unwrap();
    load(dir, "m.toml", FULL).unwrap()
}

// @zen-test: AHA-1_AC-4
// @zen-test: AHA-1_AC-5
// @zen-test: AHA-1_AC-8
#[test]
fn a_manifest_gives_the_builders_integration() {
    let dir = temp_dir("manifest-full");
    let m = full(&dir);
    assert_eq!(m.name(), "mytool");
    assert_eq!(m.harness_ids(), ["claude", "codex"]);
    let built = format!("{:?}", full_builder());
    assert_eq!(format!("{:?}", m.integration(Scope::Project)), built);
    assert_eq!(format!("{:?}", m.integration(Scope::Local)), built);
}

// @zen-test: AHA-1_AC-1
#[test]
fn toml_json_and_a_value_load_alike() {
    let dir = temp_dir("manifest-formats");
    let toml = load(
        &dir,
        "m.toml",
        "version = 1\nname = \"t\"\nallow_commands = [\"t\"]\n",
    )
    .unwrap();
    let json = load(
        &dir,
        "m.json",
        r#"{"version": 1, "name": "t", "allow_commands": ["t"]}"#,
    )
    .unwrap();
    let value = Manifest::from_json(
        json!({"version": 1, "name": "t", "allow_commands": ["t"]}),
        &dir,
        "value",
    )
    .unwrap();
    for m in [&json, &value] {
        assert_eq!(
            format!("{:?}", m.integration(Scope::Project)),
            format!("{:?}", toml.integration(Scope::Project))
        );
    }
    let e = Manifest::load(&dir.join("absent.toml"))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("absent.toml") && e.contains("no such file"),
        "{e}"
    );
}

// @zen-test: AHA-1_AC-2
#[test]
fn another_version_is_refused_for_its_version() {
    let dir = temp_dir("manifest-version");
    let e = err(&dir, "version = 2\nname = \"t\"\nnew_key = 1\n");
    assert!(
        e.contains("version 2 is not one agent-harness-adapter reads (1)"),
        "{e}"
    );
    let e = Manifest::from_json(json!({"version": 3}), &dir, "v")
        .unwrap_err()
        .to_string();
    assert!(e.contains("version 3"), "{e}");
    let e = err(&dir, "name = \"t\"\n");
    assert!(e.contains("missing field `version`"), "{e}");
}

// @zen-test: AHA-1_AC-3
// @zen-test: AHA-1_AC-6
// @zen-test: AHA-1_AC-9
#[test]
fn defaults_scopes_and_the_tool() {
    let dir = temp_dir("manifest-defaults");
    let m = load(&dir, "m.toml", "version = 1\nname = \"t\"\n").unwrap();
    let all: Vec<String> = harness::builtin().iter().map(|h| h.id().into()).collect();
    assert_eq!(m.harness_ids(), all);
    let tool = ManifestTool::new(m, "/p", "/h");
    assert_eq!(tool.manifest().name(), "t");
    assert_eq!(tool.harnesses().len(), all.len());
    assert_eq!(tool.root(Scope::User).unwrap(), Path::new("/h"));
    assert_eq!(tool.root(Scope::Local).unwrap(), Path::new("/p"));
    assert_eq!(
        tool.record_path(Scope::Project).unwrap(),
        Path::new("/p/.t/harness.toml")
    );
    assert_eq!(
        tool.record_path(Scope::Local).unwrap(),
        Path::new("/p/.t/harness.local.toml")
    );
    assert_eq!(
        tool.record_path(Scope::User).unwrap(),
        Path::new("/h/.t/harness.toml")
    );
    assert!(tool.declined_store(Scope::User).is_ok());
    assert!(tool.integration(Scope::Project).items().is_empty());

    let m = full(&dir);
    let tool = ManifestTool::new(m, "/p", "/h");
    assert!(tool.integration(Scope::User).hooks().is_empty());
    assert_eq!(tool.integration(Scope::Project).hooks().len(), 2);
    assert_eq!(
        tool.record_path(Scope::User).unwrap(),
        Path::new("/h/.config/mytool/harness.toml")
    );
    let m = load(
        &dir,
        "m.toml",
        "version = 1\nname = \"t\"\nharnesses = \"all\"\nrecord = \"r.toml\"\ndeclined = \"d.toml\"\n[scopes.local]\ndeclined = \"l.toml\"\n",
    )
    .unwrap();
    assert_eq!(m.harness_ids().len(), all.len());
    let tool = ManifestTool::new(m, "/p", "/h");
    assert_eq!(
        tool.record_path(Scope::User).unwrap(),
        Path::new("/h/r.toml")
    );
    assert_eq!(
        tool.record_path(Scope::Local).unwrap(),
        Path::new("/p/.t/harness.local.toml")
    );
}

// @zen-test: AHA-1_AC-7
#[test]
fn mistakes_are_refused_with_their_place() {
    let dir = temp_dir("manifest-errors");
    for (text, says) in [
        (
            "version = 1\nname = \"t\"\nnmae = 1\n",
            "unknown field `nmae`",
        ),
        ("version = 1\nname = \"t\"\nnmae = 1\n", "line 3"),
        ("version = 1\nname = 3\n", "invalid type"),
        ("version = 1\nname = \"a b\"\n", "`name`: `a b` is not"),
        (
            "version = 1\nname = \"t\"\ninstructions = 3\n",
            "expected a string or { file = \"<path>\" }",
        ),
        (
            "version = 1\nname = \"t\"\ninstructions = { path = \"x\" }\n",
            "unknown field `path`",
        ),
        (
            "version = 1\nname = \"t\"\nharnesses = 3\n",
            "expected \"all\" or a list of harness ids",
        ),
        (
            "version = 1\nname = \"t\"\nharnesses = \"some\"\n",
            "`harnesses`: `some`",
        ),
        (
            "version = 1\nname = \"t\"\nharnesses = [\"vim\"]\n",
            "no harness `vim`",
        ),
        (
            "version = 1\nname = \"t\"\n[[hooks]]\nevent = \"later\"\ncommand = \"x\"\n",
            "unknown variant `later`",
        ),
        (
            "version = 1\nname = \"t\"\n[[hooks]]\nevent = \"stop\"\n",
            "`hooks[0]`: give `command` or `run`",
        ),
        (
            "version = 1\nname = \"t\"\n[[hooks]]\nevent = \"stop\"\ncommand = \"a\"\nrun = \"b\"\n",
            "`hooks[0]`: give `command` or `run`",
        ),
        (
            "version = 1\nname = \"t\"\n[[hooks]]\nevent = \"stop\"\ncommand = \"a\"\ncommands = { vim = \"x\" }\n",
            "`hooks[0].commands.vim`: no harness `vim`",
        ),
        (
            "version = 1\nname = \"t\"\n[[mcp_servers]]\nname = \"m\"\n",
            "`mcp_servers[0]`: give `command`",
        ),
        (
            "version = 1\nname = \"t\"\n[[mcp_servers]]\nname = \"m\"\nurl = \"u\"\nargs = []\n",
            "`mcp_servers[0]`: give `command`",
        ),
        (
            "version = 1\nname = \"t\"\ninstructions = { file = \"none.md\" }\n",
            "`instructions`: no file `none.md`",
        ),
        (
            "version = 1\nname = \"t\"\nharnesses = [\"claude\"]\n[[parts]]\nharness = \"codex\"\nkind = \"region\"\nname = \"n\"\nfile = \"f\"\nblock = \"b\"\n",
            "`parts[0].harness`: no harness `codex`",
        ),
        (
            "version = 1\nname = \"t\"\n[[parts]]\nharness = \"claude\"\nkind = \"file\"\nname = \"n\"\ndir = \"d\"\nfiles = { x = { file = \"none\" } }\n",
            "`parts[0].files.x`: no file `none`",
        ),
        (
            "version = 1\nname = \"t\"\n[scopes.user]\nagents = [{ name = \"a\", description = \"d\", prompt = { file = \"none\" } }]\n",
            "`agents[0].prompt`: no file `none` (at user scope)",
        ),
    ] {
        let e = err(&dir, text);
        assert!(e.contains("m.toml") && e.contains(says), "{text}\n=> {e}");
    }
    let e = Manifest::from_json(json!({"version": 1, "name": "t", "x": 1}), &dir, "value")
        .unwrap_err()
        .to_string();
    assert!(
        e.starts_with("value") && e.contains("unknown field `x`"),
        "{e}"
    );
    // A TEXT path that is a directory is an IO error, named with its key.
    fs::create_dir_all(dir.join("d")).unwrap();
    let e = err(
        &dir,
        "version = 1\nname = \"t\"\ninstructions = { file = \"d\" }\n",
    );
    assert!(e.contains("`instructions`:"), "{e}");
}

#[cfg(feature = "schemars")]
#[test]
fn the_schema_describes_the_file() {
    let s = Manifest::schema();
    assert_eq!(s["title"], "AHA manifest");
    assert_eq!(s["required"], json!(["version", "name"]));
    assert_eq!(s["additionalProperties"], false);
    assert!(s["properties"]["hooks"].is_object());
}
