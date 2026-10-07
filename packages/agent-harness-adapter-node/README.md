# @six5536/agent-harness-adapter-node

Node binding of [agent-harness-adapter](https://github.com/six5536/agent-harness-adapter):
plug your tool into LLM agent harnesses (Claude Code, OpenAI Codex, Gemini
CLI, GitHub Copilot, Cursor, Factory Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin, and any agent that reads
`AGENTS.md`).

```sh
npm install @six5536/agent-harness-adapter-node
```

```js
const agent-harness-adapter = require("@six5536/agent-harness-adapter-node");

// Install the integration declared in a manifest (see the agent-harness-adapter README).
const result = agent-harness-adapter.install("mytool.harness.toml", ["claude", "codex"]);

// Be a hook command (`command = "mytool hook {harness} {event}"`):
const [harness, event] = process.argv.slice(3);
agent-harness-adapter
  .runHook(harness, event, (hook) =>
    hook.event === "pre-tool" && JSON.stringify(hook.tool).includes("rm -rf")
      ? { answer: "deny", reason: "mytool refuses rm -rf" }
      : { answer: "allow" },
  )
  .then((code) => process.exit(code));
```

Functions: `install`, `uninstall`, `status`, `parseHook`, `answerHook`, `runHook`,
`schema`; errors throw with the library's message. TypeScript types are
included. Prebuilt for Linux (glibc; x64, arm64), macOS (x64, arm64) and
Windows (x64).

MIT licensed.
