# agent-harness-adapter

**One agent setup. Every coding agent.**

Write your instructions, skills and hooks once, in `AGENTS.md` and
`.agents/`, and Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot,
Cursor, Factory Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin and any
agent that reads `AGENTS.md` use them. Hooks are written once, in Claude
Code's format, and run in every agent. Bring your Claude Code setup with one
command. Use it from the command line, Rust, Node or Python; tools ship
their own integration the same way.

```sh
npm install -g @six5536/agent-harness-adapter   # or: cargo install agent-harness-adapter
cd my-repo
agent-harness-adapter install                    # picks your agents, moves Claude Code's setup, writes the config
```

**Documentation:** <https://six5536.github.io/agent-harness-adapter/>

## Packages

| Package | What it is |
| ----- | ---------- |
| [`agent-harness-adapter`](crates/app/agent-harness-adapter) (crates.io, npm [`@six5536/agent-harness-adapter`](packages/agent-harness-adapter)) | The command. |
| [`agent-harness-adapter-core`](crates/lib/agent-harness-adapter-core) (crates.io) | The Rust library. |
| [`@six5536/agent-harness-adapter-node`](packages/agent-harness-adapter-node) (npm) | The Node binding, with TypeScript types. |
| [`six5536-agent-harness-adapter`](crates/bind/agent-harness-adapter-py) (PyPI, not yet published) | The Python binding. |

## Examples

- [A Python tool](examples/python-tool), plugged into Claude Code, Codex and Gemini CLI through `agent-harness-adapter`.
- [The site's examples](docs/examples), in each language; CI runs them.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The site lives in [`docs/`](docs)
(Astro Starlight): `cd docs && npm ci && npm run dev`.

## License

MIT. See [LICENSE](LICENSE).
