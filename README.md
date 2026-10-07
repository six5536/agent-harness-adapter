# agent-harness-kit

Plug a command-line tool into LLM agent harnesses: Claude Code, OpenAI
Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, and any agent
that reads `AGENTS.md`. Declare the integration once; the kit writes it into
each harness's own files, puts shared content in one place, never overwrites
what the user changed, and speaks every harness's hook protocol.

## Crates

| Crate | What it is |
| ----- | ---------- |
| [`agent-harness-kit-core`](crates/lib/agent-harness-kit-core) | The Rust library. |
| [`agent-harness-kit`](crates/app/agent-harness-kit) | The `ahk` command, for tools in any language; also on npm as [`@six5536/agent-harness-kit`](packages/agent-harness-kit). |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT. See [LICENSE](LICENSE).
