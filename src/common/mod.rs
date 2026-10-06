//! Pieces several harnesses share: the Claude-family hook protocol and the
//! parts built the same way everywhere (instructions, skills, MCP JSON,
//! Markdown agents and commands, group hooks).

pub(crate) mod parts;
pub(crate) mod protocol;
