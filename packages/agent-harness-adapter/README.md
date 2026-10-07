# @six5536/agent-harness-adapter

`agent-harness-adapter`: plug a command-line tool, written in any language, into LLM agent
harnesses (Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor,
Factory Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin, and any agent that reads `AGENTS.md`).

## Install

```sh
npm install -g @six5536/agent-harness-adapter
```

This package is a thin launcher. It declares a prebuilt binary for each
supported platform as an `optionalDependency`; npm installs only the one
matching your machine, and a small JS shim runs it.

**Supported platforms:** Linux and macOS (`x64` and `arm64`), and Windows
(`x64`). Elsewhere, build from source with the Rust toolchain:

```sh
cargo install agent-harness-adapter
```

## Documentation

<https://github.com/six5536/agent-harness-adapter#readme>

## License

MIT. See <https://github.com/six5536/agent-harness-adapter/blob/main/LICENSE>.
