//! What `install` and `status` report.
// @zen-component: KIT-Results

use std::path::PathBuf;

use serde::Serialize;

use crate::{
    harness::{Scope, State},
    integration::Item,
};

/// What install did to a part.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Action {
    /// Written where nothing was.
    Created,
    /// A `file` part's files written again.
    Rewrote,
    /// A region or merge part written into the existing file, or an
    /// external part written again.
    Updated,
    /// Taken out by uninstall (KIT-22).
    Removed,
    /// Left by uninstall because other installed harnesses, named in `by`,
    /// still read it (KIT-22_AC-4).
    Kept,
}

impl Action {
    /// The action's word in a report.
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Created => "created",
            Action::Rewrote => "rewrote",
            Action::Updated => "updated",
            Action::Removed => "removed",
            Action::Kept => "kept",
        }
    }
}

/// One line of the report.
// @zen-impl: KIT-4_AC-3
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct PartResult {
    /// The part's name.
    pub part: String,
    /// The part's state before any write.
    pub state: State,
    /// What `install` did; `None` when the part was left as found, and
    /// always for `status`. Left out of the JSON when `None`.
    // `default` tells schemars the key is optional; serialisation leaves it
    // out when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schemars", schemars(with = "Action"))]
    pub action: Option<Action>,
    /// The part's path relative to the root, `/`-separated; an external
    /// part's location; a shared part's covering location.
    pub path: String,
    /// The harness that writes a shared part. Left out of the JSON when
    /// `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    pub by: Option<String>,
}

impl PartResult {
    /// The report word: the action, else the state.
    /// Whether this run wrote the part (created, rewrote or updated it).
    pub fn wrote(&self) -> bool {
        matches!(
            self.action,
            Some(Action::Created | Action::Rewrote | Action::Updated)
        )
    }

    /// Whether this run wrote or removed the part.
    pub fn changed(&self) -> bool {
        self.wrote() || self.action == Some(Action::Removed)
    }

    pub fn verb(&self) -> &'static str {
        self.action
            .map_or_else(|| self.state.as_str(), Action::as_str)
    }

    /// One text line: `<verb> <path> (<part>)`, the verb padded to seven
    /// columns; a shared part names its writer: `(<part>, by <harness>)`,
    /// a part uninstall kept its users: `(<part>, used by <harnesses>)`.
    pub fn to_line(&self) -> String {
        match (&self.by, self.action) {
            (Some(by), Some(Action::Kept)) => {
                format!(
                    "{:<7} {} ({}, used by {by})",
                    self.verb(),
                    self.path,
                    self.part
                )
            }
            (Some(by), _) => format!("{:<7} {} ({}, by {by})", self.verb(), self.path, self.part),
            (None, _) => format!("{:<7} {} ({})", self.verb(), self.path, self.part),
        }
    }
}

/// One harness's part of the outcome.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct HarnessResult {
    /// The harness id.
    pub harness: String,
    /// One entry per part, in profile order.
    pub parts: Vec<PartResult>,
    /// The integration's items the harness does not take at the scope.
    pub unsupported: Vec<Item>,
    /// What the user still has to do.
    pub notes: Vec<String>,
}

/// The outcome of install or status.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct InstallResult {
    /// The scope.
    pub scope: Scope,
    /// The root directory the paths are relative to.
    pub root: PathBuf,
    /// One entry per harness named, in the tool's order.
    pub harnesses: Vec<HarnessResult>,
    /// Content an agent may load twice.
    pub warnings: Vec<String>,
}

impl InstallResult {
    /// The JSON Schema of the result's JSON (`--json`).
    #[cfg(feature = "schemars")]
    pub fn schema() -> serde_json::Value {
        let mut s = serde_json::to_value(schemars::schema_for!(InstallResult))
            .expect("a schema serialises");
        s["title"] = "AHA install result".into();
        s
    }

    /// The result of the harness `id`.
    pub fn harness(&self, id: &str) -> Option<&HarnessResult> {
        self.harnesses.iter().find(|h| h.harness == id)
    }

    /// The text report: per harness, `<id>:`, one line per part,
    /// `unsupported: <items>` and `note: <text>` lines; then `warning:
    /// <text>` lines.
    // @zen-impl: KIT-4_AC-4
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for h in &self.harnesses {
            out.push_str(&format!("{}:\n", h.harness));
            for p in &h.parts {
                out.push_str(&format!("  {}\n", p.to_line()));
            }
            if !h.unsupported.is_empty() {
                let items: Vec<&str> = h.unsupported.iter().map(|i| i.as_str()).collect();
                out.push_str(&format!("  unsupported: {}\n", items.join(", ")));
            }
            for n in &h.notes {
                out.push_str(&format!("  note: {n}\n"));
            }
        }
        for w in &self.warnings {
            out.push_str(&format!("warning: {w}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(name: &str, state: State, action: Option<Action>, path: &str) -> PartResult {
        PartResult {
            part: name.into(),
            state,
            action,
            path: path.into(),
            by: None,
        }
    }

    // @zen-test: KIT-4_AC-3
    // @zen-test: KIT-4_AC-4
    #[test]
    fn text_and_json_carry_the_same_content() {
        let r = InstallResult {
            scope: Scope::Project,
            root: "/r".into(),
            harnesses: vec![
                HarnessResult {
                    harness: "claude".into(),
                    parts: vec![
                        part(
                            "hooks",
                            State::Absent,
                            Some(Action::Created),
                            ".claude/settings.json",
                        ),
                        part("instructions", State::Edited, None, "CLAUDE.md"),
                    ],
                    unsupported: vec![],
                    notes: vec![],
                },
                HarnessResult {
                    harness: "codex".into(),
                    parts: vec![PartResult {
                        by: Some("pi".into()),
                        ..part("instructions", State::Shared, None, "AGENTS.md")
                    }],
                    unsupported: vec![Item::Permissions, Item::Commands],
                    notes: vec!["approve the hooks in /hooks".into()],
                },
            ],
            warnings: vec!["copilot may load the instructions twice: CLAUDE.md, AGENTS.md".into()],
        };
        assert_eq!(
            r.to_text(),
            "claude:\n  created .claude/settings.json (hooks)\n  edited  CLAUDE.md (instructions)\ncodex:\n  shared  AGENTS.md (instructions, by pi)\n  unsupported: permissions, commands\n  note: approve the hooks in /hooks\nwarning: copilot may load the instructions twice: CLAUDE.md, AGENTS.md\n"
        );
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            serde_json::json!({
                "scope": "project",
                "root": "/r",
                "harnesses": [
                    {
                        "harness": "claude",
                        "parts": [
                            {"part": "hooks", "state": "absent", "action": "created", "path": ".claude/settings.json"},
                            {"part": "instructions", "state": "edited", "path": "CLAUDE.md"}
                        ],
                        "unsupported": [],
                        "notes": []
                    },
                    {
                        "harness": "codex",
                        "parts": [{"part": "instructions", "state": "shared", "path": "AGENTS.md", "by": "pi"}],
                        "unsupported": ["permissions", "commands"],
                        "notes": ["approve the hooks in /hooks"]
                    }
                ],
                "warnings": ["copilot may load the instructions twice: CLAUDE.md, AGENTS.md"]
            })
        );
        assert_eq!(r.harness("codex").unwrap().parts.len(), 1);
        assert!(r.harness("pi").is_none());
    }

    #[cfg(feature = "schemars")]
    #[test]
    fn the_schema_describes_the_json() {
        let schema = InstallResult::schema();
        assert_eq!(schema["title"], "AHA install result");
        let part = &schema["$defs"]["PartResult"];
        let required: Vec<_> = part["required"].as_array().unwrap().iter().collect();
        assert!(required.contains(&&serde_json::json!("state")));
        assert!(!required.contains(&&serde_json::json!("action")));
        assert!(!required.contains(&&serde_json::json!("by")));
        // `action` and `by` are left out when absent, never `null`.
        assert!(
            !part["properties"]["action"].to_string().contains("null"),
            "{part}"
        );
        assert!(
            !part["properties"]["by"].to_string().contains("null"),
            "{part}"
        );
        let text = schema.to_string();
        assert!(
            !text.contains("[`"),
            "rustdoc links leak into the schema: {text}"
        );
        let state = schema["$defs"]["State"].to_string();
        for word in ["skipped", "shared", "absent", "current", "stale", "edited"] {
            assert!(state.contains(&format!("\"{word}\"")), "{state}");
        }
    }
}
