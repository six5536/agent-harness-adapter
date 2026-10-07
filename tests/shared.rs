//! Harnesses defined outside the kit: `install` / `status` and the hook
//! functions work with them unchanged, shared locations are written once,
//! and the set, order and record rules hold.

mod common;

use std::sync::Arc;

use agent_harness_kit::{
    Error, Harness, InstallOptions, Integration, Item, Part, PartResult, Result, Scope, State,
    claude::Claude,
    harness::{Context, Reads},
    hook::{Answer, Event, HookInput, Output, emit},
    install,
    integration::Skill,
    status,
};
use common::{TempTree, parts, verbs};
use serde_json::json;

/// A harness that reads `AGENTS.md`, and `.agents/skills`.
#[derive(Debug)]
struct Alpha;

/// A harness that reads `BETA.md` and `AGENTS.md`, and writes `BETA.md`
/// unless `beta-uses-agents` exists; it has no user scope.
#[derive(Debug)]
struct Beta;

/// Like `Alpha`, under another id.
#[derive(Debug)]
struct Gamma;

fn agents_parts(i: &Integration) -> Vec<Part> {
    let mut out = Vec::new();
    if let Some(b) = i.instructions_block() {
        out.push(Part::region("instructions", "AGENTS.md", b));
    }
    if !i.skills().is_empty() {
        out.push(Part::files(
            "skills",
            ".agents/skills",
            i.skills().iter().flat_map(Skill::dir_files).collect(),
        ));
    }
    out
}

fn agents_reads(item: Item) -> Reads {
    match item {
        Item::Instructions => Reads::always(["AGENTS.md"]),
        _ => Reads::always([".agents/skills"]),
    }
}

impl Harness for Alpha {
    fn id(&self) -> &str {
        "alpha"
    }
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }
    fn render(&self, i: &Integration, _cx: &Context) -> Result<Vec<Part>> {
        Ok(agents_parts(i))
    }
    fn reads(&self, item: Item, _cx: &Context) -> Result<Reads> {
        Ok(agents_reads(item))
    }
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        let raw: serde_json::Value = serde_json::from_str(text)?;
        let mut input = HookInput::new("alpha", event, raw.clone());
        input.continuing = raw["again"].as_bool().unwrap_or(false);
        Ok(input)
    }
    fn answer(&self, _event: Event, answer: &Answer) -> Result<Output> {
        Ok(match answer {
            Answer::Continue { reason } => Output::json(json!({ "more": reason })).exit(3),
            _ => Output::json(json!({})),
        })
    }
    fn notes(&self, _cx: &Context, parts: &[PartResult]) -> Vec<String> {
        parts
            .iter()
            .any(|p| p.action.is_some())
            .then(|| "restart alpha".to_string())
            .into_iter()
            .collect()
    }
}

impl Harness for Beta {
    fn id(&self) -> &str {
        "beta"
    }
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project]
    }
    fn render(&self, i: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let file = if cx.is_file("beta-uses-agents") {
            "AGENTS.md"
        } else {
            "BETA.md"
        };
        Ok(i.instructions_block()
            .map(|b| Part::region("instructions", file, b))
            .into_iter()
            .collect())
    }
    fn reads(&self, _item: Item, _cx: &Context) -> Result<Reads> {
        Ok(Reads::always(["BETA.md", "AGENTS.md"]))
    }
    fn parse_hook(&self, event: Event, _text: &str) -> serde_json::Result<HookInput> {
        Ok(HookInput::new("beta", event, json!({})))
    }
    fn answer(&self, _event: Event, _answer: &Answer) -> Result<Output> {
        Ok(Output::json(json!({})))
    }
}

impl Harness for Gamma {
    fn id(&self) -> &str {
        "gamma"
    }
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project]
    }
    fn render(&self, i: &Integration, _cx: &Context) -> Result<Vec<Part>> {
        Ok(agents_parts(i))
    }
    fn reads(&self, item: Item, _cx: &Context) -> Result<Reads> {
        Ok(agents_reads(item))
    }
    fn parse_hook(&self, event: Event, _text: &str) -> serde_json::Result<HookInput> {
        Ok(HookInput::new("gamma", event, json!({})))
    }
    fn answer(&self, _event: Event, _answer: &Answer) -> Result<Output> {
        Ok(Output::json(json!({})))
    }
}

fn harnesses() -> Vec<Arc<dyn Harness>> {
    vec![
        Arc::new(Gamma),
        Arc::new(Alpha),
        Arc::new(Beta),
        Arc::new(Claude),
    ]
}

fn install_set(tree: &TempTree, old: bool, ids: &[&str]) -> agent_harness_kit::InstallResult {
    let tool = if old { tree.old_tool() } else { tree.tool() };
    install(
        &tool.with(harnesses()),
        &InstallOptions::new(ids.iter().copied(), Scope::Project),
    )
    .unwrap()
}

// @zen-test: KIT-18_AC-1
// @zen-test: KIT-18_AC-3
// @zen-test: KIT-19_AC-2
// @zen-test: KIT-19_AC-3
// @zen-test: KIT-17_AC-4
// @zen-test: KIT-20_AC-1
#[test]
fn harnesses_sharing_agents_md_write_it_once() {
    let tree = TempTree::empty("share");
    let out = install_set(&tree, false, &["beta", "alpha"]);
    // Tool order: alpha before beta, whatever the order named.
    let ids: Vec<_> = out.harnesses.iter().map(|h| h.harness.as_str()).collect();
    assert_eq!(ids, ["alpha", "beta"]);
    assert_eq!(verbs(&out, "alpha"), ["created", "created"]);
    let beta = &parts(&out, "beta")[0];
    assert_eq!(
        (beta.state, beta.path.as_str(), beta.by.as_deref()),
        (State::Shared, "AGENTS.md", Some("alpha"))
    );
    assert!(!tree.exists("BETA.md"));
    assert!(tree.read("AGENTS.md").contains("This project uses tool."));
    assert!(tree.exists(".agents/skills/tool/SKILL.md"));
    let record = tree.read(".tool/harness.toml");
    assert!(
        record.contains("[alpha]") && record.ends_with("[beta]\n"),
        "{record}"
    );
    let alpha = out.harness("alpha").unwrap();
    assert_eq!(alpha.notes, ["restart alpha"]);
    assert_eq!(
        alpha.unsupported,
        [Item::Hooks, Item::Mcp, Item::Permissions]
    );
    insta::assert_snapshot!(out.to_text(), @r"
    alpha:
      created AGENTS.md (instructions)
      created .agents/skills (skills)
      unsupported: hooks, mcp, permissions
      note: restart alpha
    beta:
      shared  AGENTS.md (instructions, by alpha)
      unsupported: skills, hooks, mcp, permissions
    ");
    // Again: nothing written, no notes.
    let before = tree.files();
    let again = install_set(&tree, false, &["alpha", "beta"]);
    assert_eq!(tree.files(), before);
    assert_eq!(verbs(&again, "alpha"), ["current", "current"]);
    assert!(again.harness("alpha").unwrap().notes.is_empty());
}

// @zen-test: KIT-19_AC-1
// @zen-test: KIT-19_AC-6
#[test]
fn a_recorded_harness_joins_the_set_and_its_copy_is_warned_about() {
    let tree = TempTree::empty("recorded");
    install_set(&tree, false, &["beta"]);
    assert!(tree.exists("BETA.md"));
    // Alpha now: beta, from the record, is covered by AGENTS.md; its old
    // copy stays and is warned about; only alpha is reported.
    let out = install_set(&tree, false, &["alpha"]);
    assert_eq!(out.harnesses.len(), 1);
    assert_eq!(
        out.warnings,
        [
            "beta: the instructions now come from AGENTS.md (alpha); the earlier copy in BETA.md is left in place"
        ]
    );
    assert!(tree.exists("BETA.md"));
    assert!(tree.read(".tool/harness.toml").contains("[beta]"));
    let st = status(&tree.tool().with(harnesses()), ["beta"], Scope::Project).unwrap();
    assert_eq!(parts(&st, "beta")[0].state, State::Shared);
}

// @zen-test: KIT-19_AC-7
#[test]
fn a_new_writer_takes_over_without_making_the_part_edited() {
    let tree = TempTree::empty("writer");
    install_set(&tree, true, &["alpha"]);
    // Gamma comes first in the tool's order and writes AGENTS.md now; the
    // old block is stale by alpha's hash, not edited.
    let out = install_set(&tree, false, &["gamma"]);
    assert_eq!(parts(&out, "gamma")[0].state, State::Stale);
    assert_eq!(verbs(&out, "gamma")[0], "updated");
    assert!(tree.read("AGENTS.md").contains("This project uses tool."));
}

// @zen-test: KIT-18_AC-4
#[test]
fn a_harness_renders_from_the_tree() {
    let tree = TempTree::empty("tree");
    tree.write("beta-uses-agents", "");
    let out = install_set(&tree, false, &["beta"]);
    assert_eq!(parts(&out, "beta")[0].path, "AGENTS.md");
    assert!(!tree.exists("BETA.md"));
}

// @zen-test: KIT-17_AC-3
// @zen-test: KIT-1_AC-2
#[test]
fn refusals_from_the_set() {
    let tree = TempTree::empty("set-refusals");
    let tool = tree.tool().with(harnesses());
    let e = install(&tool, &InstallOptions::new(["beta"], Scope::User)).unwrap_err();
    assert!(matches!(e, Error::UnsupportedScope { .. }), "{e}");
    assert_eq!(e.to_string(), "harness `beta` has no user scope");
    // A raw part named like an item the harness renders clashes; one named
    // like an item it does not render fills it.
    struct Clash(TempTree, &'static str);
    impl agent_harness_kit::Tool for Clash {
        fn name(&self) -> &str {
            "tool"
        }
        fn harnesses(&self) -> Vec<Arc<dyn Harness>> {
            vec![Arc::new(Alpha)]
        }
        fn integration(&self, _scope: Scope) -> Integration {
            Integration::new()
                .instructions("x\n")
                .part("alpha", Part::region(self.1, "S.md", "s\n"))
        }
        fn root(&self, _scope: Scope) -> Result<std::path::PathBuf> {
            Ok(self.0.dir().to_path_buf())
        }
        fn record_path(&self, _scope: Scope) -> Result<std::path::PathBuf> {
            Ok(self.0.dir().join("r.toml"))
        }
        fn declined_store(
            &self,
            _scope: Scope,
        ) -> Result<Box<dyn agent_harness_kit::DeclinedStore + '_>> {
            Ok(Box::new(agent_harness_kit::TomlDeclined::new(
                self.0.dir().join("c.toml"),
                "c.toml",
            )))
        }
    }
    let clash = Clash(TempTree::empty("clash"), "instructions");
    let e = install(&clash, &InstallOptions::new(["alpha"], Scope::Project)).unwrap_err();
    assert!(matches!(e, Error::Internal(_)), "{e}");
    assert!(clash.0.files().is_empty());
    let fill = Clash(TempTree::empty("fill"), "mcp");
    let out = install(&fill, &InstallOptions::new(["alpha"], Scope::Project)).unwrap();
    let alpha = out.harness("alpha").unwrap();
    assert_eq!(alpha.parts[1].part, "mcp");
    assert!(alpha.unsupported.is_empty(), "{alpha:?}");
    assert!(fill.0.exists("S.md"));
    assert!(tree.files().is_empty());
}

// @zen-test: KIT-18_AC-3
#[test]
fn hooks_go_through_a_harness_from_outside_the_kit() {
    let input = HookInput::parse(&Alpha, Event::Stop, r#"{"again":true}"#).unwrap();
    assert_eq!(input.harness, "alpha");
    assert!(input.continuing);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = emit::<String>(
        &Alpha,
        Event::Stop,
        Ok(Answer::Continue {
            reason: "check".into(),
        }),
        &mut out,
        &mut err,
    )
    .unwrap();
    assert_eq!(code, 3);
    assert_eq!(String::from_utf8(out).unwrap(), "{\"more\":\"check\"}\n");
}
