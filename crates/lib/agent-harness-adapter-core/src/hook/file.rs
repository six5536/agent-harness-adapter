//! The hooks file (KIT-24): hooks kept once, in Claude Code's settings
//! format, e.g. `.agents/hooks.json`. Its events become dispatch hooks
//! (`agent-harness-adapter hook --hooks <path>`), which run its entries at
//! hook time in every harness.
// @zen-component: KIT-HooksFile

use std::{collections::BTreeMap, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    Error, Result,
    common::parts,
    fs,
    harness::{MergeOp, Part},
    hook::{Event, wire},
    integration::{Integration, Item},
};

/// A hooks file: Claude Code's `hooks` object, event name to groups.
// @zen-impl: KIT-24_AC-1
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HooksFile {
    /// Claude Code's event names (`PreToolUse`, …) to their groups.
    #[serde(default)]
    pub hooks: BTreeMap<String, Vec<Group>>,
}

/// A group: a matcher on Claude Code's tool name, and its entries.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Group {
    /// A regular expression matching the whole tool name; absent, empty or
    /// `*` matches every call.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    /// The entries, run in order.
    pub hooks: Vec<Entry>,
}

/// One hook entry.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Entry {
    /// `command`: a shell command line. Claude Code's other types (e.g.
    /// `prompt`) are kept but not run.
    #[serde(rename = "type", default = "command_type")]
    pub kind: String,
    /// The shell command line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Seconds before the command is stopped (60 when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    /// The tool that wrote the entry; such entries are installed by that
    /// tool into every harness itself, so dispatch skips them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
}

fn command_type() -> String {
    "command".into()
}

/// An entry's timeout when it gives none, in seconds, as Claude Code's.
pub const DEFAULT_TIMEOUT: u64 = 60;

impl Entry {
    /// A command entry.
    pub fn command(command: impl Into<String>) -> Self {
        Entry {
            kind: command_type(),
            command: Some(command.into()),
            timeout: None,
            tool: None,
        }
    }

    /// Whether dispatch runs it: a command written by no tool.
    pub fn dispatched(&self) -> bool {
        self.kind == "command" && self.command.is_some() && self.tool.is_none()
    }
}

impl HooksFile {
    /// `text`, the hooks file `path` (for messages), read and checked.
    // @zen-impl: KIT-24_AC-4
    pub fn parse(text: &str, path: &str) -> Result<Self> {
        let value: Value =
            serde_json::from_str(text).map_err(|e| Error::file(path, e.to_string()))?;
        let file: HooksFile = serde_json::from_value(value)
            .map_err(|e| Error::file(path, format!("not a hooks file: {e}")))?;
        for (event, groups) in &file.hooks {
            for (g, group) in groups.iter().enumerate() {
                for (i, entry) in group.hooks.iter().enumerate() {
                    if entry.kind == "command" && entry.command.is_none() {
                        return Err(Error::file(
                            path,
                            format!(
                                "hooks.{event}[{g}].hooks[{i}]: a command entry needs `command`"
                            ),
                        ));
                    }
                }
            }
        }
        Ok(file)
    }

    /// The hooks file at `path`, or `None` when there is none.
    pub fn read(path: &Path) -> Result<Option<Self>> {
        match fs::read_text(path)? {
            Some(text) => Self::parse(&text, &path.display().to_string()).map(Some),
            None => Ok(None),
        }
    }

    /// The events with an entry dispatch runs, in [`Event::ALL`] order.
    // @zen-impl: KIT-24_AC-2
    pub fn events(&self) -> Vec<Event> {
        Event::ALL
            .into_iter()
            .filter(|e| {
                self.groups(*e)
                    .iter()
                    .any(|g| g.hooks.iter().any(Entry::dispatched))
            })
            .collect()
    }

    /// The groups of `event`.
    pub fn groups(&self, event: Event) -> &[Group] {
        self.hooks
            .get(wire::event_name(event))
            .map_or(&[], Vec::as_slice)
    }

    /// The event names it has that are not one of the seven, which nothing
    /// runs (KIT-24_AC-7).
    // @zen-impl: KIT-24_AC-7
    pub fn other_events(&self) -> Vec<&str> {
        self.hooks
            .keys()
            .map(String::as_str)
            .filter(|n| wire::event_from_name(n).is_none())
            .collect()
    }

    /// The entries of `event` that are not command entries, which nothing
    /// runs.
    pub fn other_types(&self) -> usize {
        self.hooks
            .values()
            .flatten()
            .flat_map(|g| &g.hooks)
            .filter(|e| e.kind != "command")
            .count()
    }
}

/// A tool's declared hooks as its part in the hooks file `location`, for a
/// harness (id `harness`, for `{harness}`) that loads that file natively:
/// owned group entries under Claude Code's event names and matchers, each
/// entry carrying `tool` (KIT-24_AC-5), so dispatch skips them and uninstall
/// removes only them. A harness from outside the library uses it in its
/// `render`.
// @zen-impl: KIT-24_AC-5
pub fn tool_part(
    integration: &Integration,
    tool: &str,
    harness: &str,
    location: &str,
) -> Result<Part> {
    let hooks: Vec<_> = integration.hooks().iter().collect();
    let owned = parts::owner(integration, &hooks)?;
    let mut ops = Vec::new();
    for event in Event::ALL {
        let mut groups: Vec<(Option<&str>, Vec<Value>)> = Vec::new();
        for h in hooks.iter().filter(|h| h.event() == event) {
            let matcher = h.tool_kind().and_then(crate::claude::matcher);
            let mut entry = serde_json::json!({ "type": "command", "command": h.command(harness), "tool": tool });
            if let Some(t) = h.timeout_value() {
                entry["timeout"] = parts::seconds(t);
            }
            match groups.iter_mut().find(|(m, _)| *m == matcher) {
                Some((_, es)) => es.push(entry),
                None => groups.push((matcher, vec![entry])),
            }
        }
        let groups = groups
            .into_iter()
            .map(|(m, es)| match m {
                Some(m) => serde_json::json!({ "matcher": m, "hooks": es }),
                None => serde_json::json!({ "hooks": es }),
            })
            .collect();
        ops.push(MergeOp::group_entries(
            ["hooks", wire::event_name(event)],
            "hooks",
            "command",
            owned.clone(),
            groups,
        ));
    }
    Ok(Part::merge(Item::Hooks.as_str(), location, ops))
}

/// The JSON Schema of a hooks file.
// @zen-impl: KIT-24_AC-6
#[cfg(feature = "schemars")]
pub fn schema() -> Value {
    let mut schema =
        serde_json::to_value(schemars::schema_for!(HooksFile)).expect("a schema serialises");
    schema["title"] = "AHA hooks file".into();
    schema
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use serde_json::json;

    use super::*;

    // @zen-test: KIT-24_AC-1
    // @zen-test: KIT-24_AC-4
    #[test]
    fn reads_claude_codes_format() {
        let f = HooksFile::parse(
            &json!({"hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard.sh", "timeout": 5}]}],
                "Stop": [{"hooks": [{"type": "command", "command": "t", "tool": "sokf"}]}],
                "Notification": [{"hooks": [{"type": "command", "command": "n"}]}],
                "PostToolUse": [{"hooks": [{"type": "prompt", "prompt": "check"}]}]
            }})
            .to_string(),
            "h.json",
        )
        .unwrap();
        assert_eq!(f.events(), vec![Event::PreTool]);
        assert_eq!(f.groups(Event::PreTool)[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(f.groups(Event::PreTool)[0].hooks[0].timeout, Some(5));
        assert!(f.groups(Event::SessionEnd).is_empty());
        assert_eq!(f.other_events(), vec!["Notification"]);
        assert_eq!(f.other_types(), 1);
        for (bad, says) in [
            ("nope", "h.json"),
            (r#"{"hooks": []}"#, "not a hooks file"),
            (
                r#"{"hooks": {"Stop": [{"matcher": "x"}]}}"#,
                "not a hooks file",
            ),
            (
                r#"{"hooks": {"Stop": [{"hooks": [{"type": "command"}]}]}}"#,
                "hooks.Stop[0].hooks[0]",
            ),
        ] {
            let e = HooksFile::parse(bad, "h.json").unwrap_err().to_string();
            assert!(e.contains(says), "{bad}: {e}");
        }
        assert_eq!(HooksFile::parse("{}", "h").unwrap(), HooksFile::default());
    }

    // @zen-test: KIT-24_AC-6
    #[cfg(feature = "schemars")]
    #[test]
    fn the_schema_names_the_hooks_file() {
        let s = schema();
        assert_eq!(s["title"], "AHA hooks file");
        assert!(s["properties"]["hooks"].is_object());
    }

    #[test]
    fn a_missing_file_is_none() {
        let dir = std::env::temp_dir().join(format!("aha-hooks-file-{}", std::process::id()));
        assert_eq!(HooksFile::read(&dir.join("none.json")).unwrap(), None);
    }

    fn files() -> impl Strategy<Value = HooksFile> {
        let entry = (
            "[a-z ]{1,10}",
            proptest::option::of(1u64..100),
            proptest::option::of("[a-z]{1,5}"),
        )
            .prop_map(|(c, timeout, tool)| Entry {
                timeout,
                tool,
                ..Entry::command(c)
            });
        let group = (
            proptest::option::of("[A-Za-z|]{0,8}"),
            proptest::collection::vec(entry, 1..3),
        )
            .prop_map(|(matcher, hooks)| Group { matcher, hooks });
        let event = proptest::sample::select(Event::ALL.map(wire::event_name).to_vec());
        proptest::collection::btree_map(
            event.prop_map(String::from),
            proptest::collection::vec(group, 0..3),
            0..4,
        )
        .prop_map(|hooks| HooksFile { hooks })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        // @zen-test: KIT_P-17
        #[test]
        fn hooks_files_round_trip(f in files()) {
            let text = serde_json::to_string(&f).unwrap();
            prop_assert_eq!(&HooksFile::parse(&text, "h").unwrap(), &f);
            let want: Vec<Event> = Event::ALL
                .into_iter()
                .filter(|e| f.groups(*e).iter().flat_map(|g| &g.hooks).any(|x| x.tool.is_none()))
                .collect();
            prop_assert_eq!(f.events(), want);
        }
    }
}
