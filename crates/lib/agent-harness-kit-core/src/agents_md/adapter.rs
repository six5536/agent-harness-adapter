//! Any agent that reads `AGENTS.md` (<https://agents.md>) and the shared
//! skills directory `.agents/skills`.
// @zen-component: HAR-AgentsMd

use crate::{
    Error, Result,
    common::{parts, protocol},
    harness::{Context, Harness, Part, Reads, Scope},
    hook::{Answer, Event, HookInput, Output},
    integration::{Integration, Item},
};

/// The generic `AGENTS.md` agent (`agents`): instructions and skills only.
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentsMd;

const ID: &str = "agents";

fn location(item: Item, cx: &Context) -> Option<&'static str> {
    match item {
        Item::Instructions if cx.scope == Scope::Project => Some("AGENTS.md"),
        Item::Skills => Some(".agents/skills"),
        _ => None,
    }
}

impl Harness for AgentsMd {
    fn id(&self) -> &str {
        ID
    }

    // @zen-impl: HAR-8_AC-1
    fn scopes(&self) -> &[Scope] {
        &[Scope::Project, Scope::User]
    }

    // @zen-impl: HAR-8_AC-2
    // @zen-impl: HAR-8_AC-3
    fn render(&self, integration: &Integration, cx: &Context) -> Result<Vec<Part>> {
        let mut out = Vec::new();
        if let (Some(at), Some(block)) = (
            location(Item::Instructions, cx),
            integration.instructions_block(),
        ) {
            out.push(parts::instructions(at, block));
        }
        if !integration.skills().is_empty() {
            out.push(parts::skills(".agents/skills", integration));
        }
        Ok(out)
    }

    fn hook_events(&self) -> &[Event] {
        &[]
    }

    fn reads(&self, item: Item, cx: &Context) -> Result<Reads> {
        Ok(Reads::always(location(item, cx)))
    }

    /// It installs no hooks; any input reads as an empty one.
    fn parse_hook(&self, event: Event, text: &str) -> serde_json::Result<HookInput> {
        Ok(HookInput::new(ID, event, protocol::raw_object(text)?))
    }

    fn answer(&self, _event: Event, _answer: &Answer) -> Result<Output> {
        Err(Error::Unsupported {
            harness: ID.into(),
            what: "run hooks".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration::{Hook, Skill};

    // @zen-test: HAR-8_AC-1
    // @zen-test: HAR-8_AC-2
    // @zen-test: HAR-8_AC-3
    #[test]
    fn instructions_and_skills_only() {
        let i = Integration::new()
            .instructions("x\n")
            .skill(Skill::new("t", "d", "b"))
            .hook(Hook::new(Event::Stop, "t {event}"));
        let cx = Context::new("t", Scope::Project, "/nowhere", None);
        let names: Vec<_> = AgentsMd
            .render(&i, &cx)
            .unwrap()
            .iter()
            .map(|p| format!("{} {}", p.name(), p.location()))
            .collect();
        assert_eq!(names, ["instructions AGENTS.md", "skills .agents/skills"]);
        let user = Context::new("t", Scope::User, "/nowhere", None);
        assert_eq!(AgentsMd.render(&i, &user).unwrap().len(), 1);
        assert_eq!(
            AgentsMd.reads(Item::Instructions, &user).unwrap(),
            Reads::default()
        );
        assert_eq!(AgentsMd.scopes().len(), 2);
        assert!(AgentsMd.hook_events().is_empty());
        assert_eq!(crate::claude::Claude.hook_events().len(), 7);
        assert_eq!(
            AgentsMd.parse_hook(Event::Stop, "{}").unwrap().harness,
            "agents"
        );
        assert!(
            AgentsMd
                .answer(Event::Stop, &Answer::Allow { stderr: None })
                .is_err()
        );
    }
}
