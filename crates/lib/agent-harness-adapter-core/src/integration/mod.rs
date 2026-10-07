//! A tool's integration, declared once without naming a harness: an
//! instructions block, skills, hooks, MCP servers, allowed commands,
//! subagents and slash commands, plus raw parts for one harness. Each
//! [`Harness`](crate::harness::Harness) renders it into its own files.

mod declaration;
mod items;
mod skill;

pub use declaration::{Integration, Item};
pub use items::{Agent, Command, Hook, McpServer, Transport};
pub use skill::Skill;
