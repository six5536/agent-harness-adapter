//! Pieces several harnesses share: the Claude-family hook protocol, the
//! hook format of the generated extensions (Pi, OpenCode, Kilo Code), the
//! OpenCode family's plugin, the Gemini family's settings, and the parts
//! built the same way everywhere (instructions, skills, MCP JSON, Markdown
//! agents and commands, group hooks).
// @zen-component: HAR-Common
// @zen-impl: HAR-9_AC-1

pub(crate) mod extension;
pub(crate) mod parts;
pub(crate) mod plugin;
pub(crate) mod protocol;
pub(crate) mod settings;
pub(crate) mod toml_out;
