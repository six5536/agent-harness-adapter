//! Parts several harnesses build the same way, so equal locations get equal
//! parts.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{
    Result,
    harness::{EntryMatch, MergeOp, Part},
    hook::{Event, ToolKind},
    integration::{Hook, Integration, Item},
};

/// The instructions region in `file`.
pub(crate) fn instructions(file: &str, block: &str) -> Part {
    Part::region(Item::Instructions.as_str(), file, block)
}

/// The skills, as skill directories under `dir`.
pub(crate) fn skills(dir: &str, integration: &Integration) -> Part {
    Part::files(
        Item::Skills.as_str(),
        dir,
        integration
            .skills()
            .iter()
            .flat_map(|s| s.dir_files())
            .collect(),
    )
}

/// The MCP servers as members of `key` in `file`, each rendered by
/// `to_json`.
pub(crate) fn mcp(
    file: &str,
    key: &str,
    integration: &Integration,
    to_json: impl Fn(&crate::integration::McpServer) -> Value,
) -> Part {
    Part::merge(
        Item::Mcp.as_str(),
        file,
        integration
            .mcp_servers()
            .iter()
            .map(|s| MergeOp::object_member([key], s.name(), to_json(s)))
            .collect(),
    )
}

/// The agents as `<name><ext>` files under `dir`, each rendered by
/// `render`.
pub(crate) fn agents(
    dir: &str,
    ext: &str,
    integration: &Integration,
    render: impl Fn(&crate::integration::Agent) -> String,
) -> Part {
    Part::files(
        Item::Agents.as_str(),
        dir,
        integration
            .agents()
            .iter()
            .map(|a| (format!("{}{ext}", a.name()), render(a)))
            .collect(),
    )
}

/// The commands as `<name><ext>` files under `dir`, each rendered by
/// `render`.
pub(crate) fn commands(
    dir: &str,
    ext: &str,
    integration: &Integration,
    render: impl Fn(&crate::integration::Command) -> String,
) -> Part {
    Part::files(
        Item::Commands.as_str(),
        dir,
        integration
            .commands()
            .iter()
            .map(|c| (format!("{}{ext}", c.name()), render(c)))
            .collect(),
    )
}

/// The entry match for all of `hooks`.
pub(crate) fn owner(integration: &Integration, hooks: &[&Hook]) -> Result<EntryMatch> {
    Ok(EntryMatch::any_of(
        hooks
            .iter()
            .map(|h| integration.hook_owner(h))
            .collect::<Result<Vec<_>>>()?,
    ))
}

/// How a harness lays out its group hooks.
pub(crate) struct GroupHooks {
    /// The array of an event's groups, from the event's name.
    pub(crate) path: fn(&str) -> Vec<String>,
    /// The harness's name for an event; `None` when it lacks it.
    pub(crate) event: fn(Event) -> Option<&'static str>,
    /// The matcher of a tool kind; `None` when the harness cannot match it.
    pub(crate) matcher: fn(ToolKind) -> Option<&'static str>,
    /// A timeout in the harness's unit.
    pub(crate) timeout: fn(Duration) -> Value,
    /// The harness id, for `{harness}`.
    pub(crate) harness: &'static str,
}

/// One group-entries op per event the harness has, the tool's hooks for it
/// grouped by matcher: `{"matcher"?, "hooks": [{"type": "command",
/// "command", "timeout"?}]}`. An event without hooks still gets an op, so
/// the tool's old entries there are removed.
pub(crate) fn group_hooks(layout: &GroupHooks, integration: &Integration) -> Result<Vec<MergeOp>> {
    let hooks: Vec<&Hook> = integration.hooks().iter().collect();
    let owned = owner(integration, &hooks)?;
    let mut ops = Vec::new();
    for event in Event::ALL {
        let Some(name) = (layout.event)(event) else {
            continue;
        };
        let mut groups: Vec<(Option<&str>, Vec<Value>)> = Vec::new();
        for h in hooks.iter().filter(|h| h.event() == event) {
            let matcher = h.tool_kind().and_then(layout.matcher);
            let mut entry = json!({ "type": "command", "command": h.command(layout.harness) });
            if let Some(t) = h.timeout_value() {
                entry["timeout"] = (layout.timeout)(t);
            }
            match groups.iter_mut().find(|(m, _)| *m == matcher) {
                Some((_, es)) => es.push(entry),
                None => groups.push((matcher, vec![entry])),
            }
        }
        let groups = groups
            .into_iter()
            .map(|(m, es)| match m {
                Some(m) => json!({ "matcher": m, "hooks": es }),
                None => json!({ "hooks": es }),
            })
            .collect();
        ops.push(MergeOp::group_entries(
            (layout.path)(name),
            "hooks",
            "command",
            owned.clone(),
            groups,
        ));
    }
    Ok(ops)
}

/// Seconds, rounded up.
pub(crate) fn seconds(t: Duration) -> Value {
    json!(t.as_secs() + u64::from(t.subsec_nanos() > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_hooks_group_by_matcher_and_cover_every_event() {
        let layout = GroupHooks {
            path: |e| vec!["hooks".into(), e.into()],
            event: |e| match e {
                Event::Stop => Some("Stop"),
                Event::PreTool => Some("PreToolUse"),
                _ => None,
            },
            matcher: |k| (k == ToolKind::Shell).then_some("Bash"),
            timeout: seconds,
            harness: "claude",
        };
        let i = Integration::new()
            .hook(Hook::new(Event::PreTool, "t hook {harness} {event}").tools(ToolKind::Shell))
            .hook(
                Hook::new(Event::PreTool, "t hook {harness} {event} --b")
                    .tools(ToolKind::Shell)
                    .timeout(Duration::from_millis(1500)),
            )
            .hook(Hook::new(Event::PreTool, "t hook {harness} {event} --any"));
        let ops = group_hooks(&layout, &i).unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(
            ops[0].value(),
            &json!([
                { "matcher": "Bash", "hooks": [
                    { "type": "command", "command": "t hook claude pre-tool" },
                    { "type": "command", "command": "t hook claude pre-tool --b", "timeout": 2 }
                ]},
                { "hooks": [{ "type": "command", "command": "t hook claude pre-tool --any" }] }
            ])
        );
        assert_eq!(ops[1].value(), &json!([]));
        assert!(ops[1].expects_nothing());
        assert_eq!(seconds(Duration::from_secs(3)), json!(3));
    }
}
