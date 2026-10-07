# agent-harness-kit

[![crates.io](https://img.shields.io/crates/v/agent-harness-kit.svg)](https://crates.io/crates/agent-harness-kit)

`ahk`: plug a command-line tool, written in any language, into LLM agent
harnesses (Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor,
Factory Droid, Pi, and any agent that reads `AGENTS.md`).

## Install

```sh
cargo install agent-harness-kit      # or
npm install -g @six5536/agent-harness-kit
```

Both install the `ahk` command. Rust tools can use the library
[`agent-harness-kit-core`](https://crates.io/crates/agent-harness-kit-core)
directly instead.

## License

MIT. See [LICENSE](https://github.com/six5536/agent-harness-kit/blob/main/LICENSE).
