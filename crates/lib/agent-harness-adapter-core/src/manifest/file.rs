//! The manifest file's shapes, as serde reads them. Unknown keys are
//! errors; the same shapes give the manifest's JSON Schema.

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, Visitor},
};
use serde_json::Value;

use crate::hook::{Event, ToolKind};

/// Only the version, read first so a manifest of another version is
/// refused for that, not for its other keys.
#[derive(Debug, Deserialize)]
pub(crate) struct VersionOnly {
    pub version: u64,
}

/// Text inline, or the contents of a file relative to the manifest.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(untagged))]
#[derive(Debug, Clone)]
pub(crate) enum Text {
    /// The text itself.
    Inline(String),
    /// A file's contents.
    File(FileRef),
}

/// `{ file = "<path>" }`: a file relative to the manifest's directory.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FileRef {
    /// The path, relative to the manifest's directory.
    pub file: String,
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Text;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or { file = \"<path>\" }")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Text, E> {
                Ok(Text::Inline(s.to_string()))
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Text, A::Error> {
                FileRef::deserialize(de::value::MapAccessDeserializer::new(map)).map(Text::File)
            }
        }
        d.deserialize_any(V)
    }
}

impl<'de> Deserialize<'de> for Harnesses {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Harnesses;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("\"all\" or a list of harness ids")
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Harnesses, E> {
                Ok(Harnesses::All(s.to_string()))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Harnesses, A::Error> {
                Vec::deserialize(de::value::SeqAccessDeserializer::new(seq)).map(Harnesses::List)
            }
        }
        d.deserialize_any(V)
    }
}

/// A map that keeps its keys in the order written.
#[derive(Debug, Clone)]
pub(crate) struct Ordered<V>(pub Vec<(String, V)>);

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Ordered<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Pairs<V>(PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for Pairs<V> {
            type Value = Ordered<V>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a table")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(kv) = map.next_entry::<String, V>()? {
                    out.push(kv);
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(Pairs(PhantomData))
    }
}

#[cfg(feature = "schemars")]
impl<V: schemars::JsonSchema> schemars::JsonSchema for Ordered<V> {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("Ordered_{}", V::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <std::collections::BTreeMap<String, V>>::json_schema(generator)
    }
}

/// `"all"`, or harness ids in the order the tool prefers them.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schemars", schemars(untagged))]
#[derive(Debug, Clone)]
pub(crate) enum Harnesses {
    /// `"all"`: every built-in harness.
    All(String),
    /// These harnesses.
    List(Vec<String>),
}

/// Which hook entries are the tool's: `{ prefix = ".." }`,
/// `{ contains = ".." }` or `{ any = [..] }`.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Match {
    /// The command starts with the text.
    Prefix(String),
    /// The command contains the text.
    Contains(String),
    /// Any of these matches.
    Any(Vec<Match>),
}

/// An agent skill: `dir`, or `name`, `description`, `body` and `files`.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkillFile {
    /// A skill directory, relative to the manifest's directory: its
    /// `SKILL.md`, written as it is, and every other file in it but hidden
    /// ones. Not with the other fields.
    #[serde(default)]
    pub dir: Option<String>,
    /// The skill's name: lowercase, digits and `-`.
    #[serde(default)]
    pub name: Option<String>,
    /// When the agent should use it.
    #[serde(default)]
    pub description: Option<String>,
    /// The body after the frontmatter.
    #[serde(default)]
    pub body: Option<Text>,
    /// Extra files beside `SKILL.md`: path → text.
    #[serde(default)]
    pub files: Option<Ordered<Text>>,
}

/// A hook: `command` (a template the harness runs) or `run` (a command
/// `agent-harness-adapter hook` runs with the hook contract), not both.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HookFile {
    /// The event.
    pub event: Event,
    /// A hook command template: `{harness}` and `{event}` are filled in.
    #[serde(default)]
    pub command: Option<String>,
    /// A command bridged through `agent-harness-adapter hook`: it reads the hook input on
    /// stdin and writes an answer on stdout.
    #[serde(default)]
    pub run: Option<String>,
    /// Only for tool calls of this kind (pre tool, post tool).
    #[serde(default)]
    pub tools: Option<ToolKind>,
    /// A timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Per-harness command templates: harness id → template.
    #[serde(default)]
    pub commands: Option<Ordered<String>>,
    /// Which entries in a harness's file are this hook's.
    #[serde(default)]
    pub owned: Option<Match>,
}

/// An MCP server: `command` (with `args`, `env`) or `url` (with `headers`).
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct McpFile {
    /// The server's name.
    pub name: String,
    /// The program of a local server.
    #[serde(default)]
    pub command: Option<String>,
    /// Its arguments.
    #[serde(default)]
    pub args: Option<Vec<String>>,
    /// Its environment variables.
    #[serde(default)]
    pub env: Option<Ordered<String>>,
    /// The URL of a remote server.
    #[serde(default)]
    pub url: Option<String>,
    /// Its HTTP headers.
    #[serde(default)]
    pub headers: Option<Ordered<String>>,
}

/// A tool of an MCP server the agent may call without asking.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct McpToolFile {
    /// The server's name.
    pub server: String,
    /// The tool's name.
    pub tool: String,
}

/// A subagent or a slash command.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PromptFile {
    /// Its name.
    pub name: String,
    /// Its description.
    pub description: String,
    /// Its prompt (a command's `$ARGUMENTS` stands for what the user typed).
    pub prompt: Text,
}

/// A raw part for one harness.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum PartFile {
    /// Whole files under a directory.
    File {
        /// The harness id.
        harness: String,
        /// The part's name.
        name: String,
        /// The directory, relative to the root.
        dir: String,
        /// Path (relative to `dir`) → text.
        files: Ordered<Text>,
    },
    /// The block between the tool's markers in a file.
    Region {
        /// The harness id.
        harness: String,
        /// The part's name.
        name: String,
        /// The file, relative to the root.
        file: String,
        /// The block.
        block: Text,
    },
    /// Entries in a JSON file, or members in a TOML file (`.toml`).
    Merge {
        /// The harness id.
        harness: String,
        /// The part's name.
        name: String,
        /// The file, relative to the root.
        file: String,
        /// The entries.
        ops: Vec<OpFile>,
    },
}

/// One merge operation on the file's JSON or TOML.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum OpFile {
    /// `value` once in the array at `path`.
    ArrayEntry {
        /// Keys from the top level.
        path: Vec<String>,
        /// The entry.
        value: Value,
    },
    /// The member `key` of the object at `path` is `value`.
    ObjectMember {
        /// Keys from the top level.
        path: Vec<String>,
        /// The member's key.
        key: String,
        /// Its value.
        value: Value,
    },
    /// The tool's entries (by `field`) in the array at `path`.
    OwnedEntries {
        /// Keys from the top level.
        path: Vec<String>,
        /// The field that marks an entry as the tool's.
        field: String,
        /// Which values of the field are the tool's.
        owned: Match,
        /// The tool's entries.
        entries: Vec<Value>,
    },
    /// The tool's entries in the groups of the array at `path`.
    GroupEntries {
        /// Keys from the top level.
        path: Vec<String>,
        /// The key of a group's entries.
        entries: String,
        /// The field that marks an entry as the tool's.
        field: String,
        /// Which values of the field are the tool's.
        owned: Match,
        /// The groups, each holding the tool's entries under `entries`.
        groups: Vec<Value>,
    },
}

/// The item fields, written once: the top level and each scope table hold
/// them, and [`Items`] borrows them.
macro_rules! with_items {
    ($($(#[doc = $doc:literal])* $field:ident: $ty:ty,)*) => {
        /// The items of a manifest or of one scope table, borrowed.
        #[derive(Debug, Default, Clone, Copy)]
        pub(crate) struct Items<'a> {
            $(pub $field: Option<&'a $ty>,)*
        }

        impl<'a> Items<'a> {
            /// These items, each replaced by `scope`'s when it has one.
            pub fn with(self, scope: Items<'a>) -> Items<'a> {
                Items { $($field: scope.$field.or(self.$field),)* }
            }
        }

        /// A manifest file (version 1).
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[cfg_attr(feature = "schemars", schemars(title = "AHA manifest"))]
        #[derive(Debug, Clone, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub(crate) struct ManifestFile {
            /// The manifest format's version: 1.
            pub version: u64,
            /// The tool's name: letters, digits, `-`, `_` and `.`.
            pub name: String,
            /// `"all"` (the default), or harness ids in preference order.
            #[serde(default)]
            pub harnesses: Option<Harnesses>,
            /// The command bridged hooks run: `agent-harness-adapter` by default.
            #[serde(default)]
            pub adapter: Option<String>,
            /// The record file, relative to the scope's root.
            #[serde(default)]
            pub record: Option<String>,
            /// The declined parts' file, relative to the scope's root.
            #[serde(default)]
            pub declined: Option<String>,
            $($(#[doc = $doc])* #[serde(default)] pub $field: Option<$ty>,)*
            /// Per scope: the items and paths that replace the top level's.
            #[serde(default)]
            pub scopes: Option<ScopesFile>,
        }

        /// What one scope replaces.
        #[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
        #[derive(Debug, Clone, Default, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub(crate) struct ScopeFile {
            /// The record file at this scope.
            #[serde(default)]
            pub record: Option<String>,
            /// The declined parts' file at this scope.
            #[serde(default)]
            pub declined: Option<String>,
            $($(#[doc = $doc])* #[serde(default)] pub $field: Option<$ty>,)*
        }

        impl ManifestFile {
            pub fn items(&self) -> Items<'_> {
                Items { $($field: self.$field.as_ref(),)* }
            }
        }

        impl ScopeFile {
            pub fn items(&self) -> Items<'_> {
                Items { $($field: self.$field.as_ref(),)* }
            }
        }
    };
}

with_items! {
    /// The instructions block, placed between the tool's markers in the
    /// file each harness reads.
    instructions: Text,
    /// Which hook entries are the tool's, for hooks without `owned`.
    hook_match: Match,
    /// Agent skills.
    skills: Vec<SkillFile>,
    /// Hooks.
    hooks: Vec<HookFile>,
    /// MCP servers.
    mcp_servers: Vec<McpFile>,
    /// Command prefixes the agent may run without asking.
    allow_commands: Vec<String>,
    /// MCP tools the agent may call without asking.
    allow_mcp_tools: Vec<McpToolFile>,
    /// Subagents.
    agents: Vec<PromptFile>,
    /// Slash commands.
    commands: Vec<PromptFile>,
    /// Raw parts for one harness.
    parts: Vec<PartFile>,
}

/// The scope tables.
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScopesFile {
    /// Project scope.
    #[serde(default)]
    pub project: Option<ScopeFile>,
    /// User scope.
    #[serde(default)]
    pub user: Option<ScopeFile>,
    /// Local scope.
    #[serde(default)]
    pub local: Option<ScopeFile>,
}
