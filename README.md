# agent-harness-adapter

Plug a command-line tool into LLM agent harnesses: Claude Code, OpenAI
Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin, and any agent
that reads `AGENTS.md`. Declare the integration once; the adapter writes it into
each harness's own files, puts shared content in one place, never overwrites
what the user changed, and speaks every harness's hook protocol.

Documentation: <https://six5536.github.io/agent-harness-adapter/>.

## Crates

| Crate | What it is |
| ----- | ---------- |
| [`agent-harness-adapter-core`](crates/lib/agent-harness-adapter-core) | The Rust library. |
| [`agent-harness-adapter`](crates/app/agent-harness-adapter) | The `agent-harness-adapter` command, for tools in any language; also on npm as [`@six5536/agent-harness-adapter`](packages/agent-harness-adapter). |
| [`six5536-agent-harness-adapter`](crates/bind/agent-harness-adapter-py) (PyPI) | The Python binding. |
| [`@six5536/agent-harness-adapter-node`](packages/agent-harness-adapter-node) (npm) | The Node binding, with TypeScript types. |

## Examples

- [A Python tool](examples/python-tool), plugged into Claude Code, Codex and Gemini CLI through `agent-harness-adapter`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
