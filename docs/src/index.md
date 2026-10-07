# agent-harness-adapter

Plug a command-line tool into LLM agent harnesses: Claude Code, OpenAI
Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, OpenCode,
Kilo Code, Qwen Code, Devin, and any agent that reads `AGENTS.md`.

Declare the integration once: instructions for the agent, skills, hooks,
MCP servers, allowed commands, subagents and slash commands. The adapter
writes it into each harness's own files, puts content several harnesses
read in one place, never overwrites what the user changed, takes it all
out again on uninstall, and speaks every harness's hook protocol.

## Which part do I need?

| You are writing | Use |
| --------------- | --- |
| A tool in any language | [the `agent-harness-adapter` command](cli.md) and a manifest file, on npm and crates.io |
| A Rust tool | [the Rust library](rust.md), `agent-harness-adapter-core` |
| A Python tool | [the Python binding](python.md) |
| A Node or TypeScript tool | [the Node binding](node.md) |

[Harnesses](harnesses.md) lists where each agent's files go, and
[JSON schemas](schemas.md) the published formats: the manifest, the hook
contract and the install result.

The source is at
[github.com/six5536/agent-harness-adapter](https://github.com/six5536/agent-harness-adapter),
under the MIT license.
