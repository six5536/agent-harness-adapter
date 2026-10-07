# @six5536/agent-harness-kit-node

Node binding of [agent-harness-kit](https://github.com/six5536/agent-harness-kit):
plug your tool into LLM agent harnesses (Claude Code, OpenAI Codex, Gemini
CLI, GitHub Copilot, Cursor, Factory Droid, Pi, and any agent that reads
`AGENTS.md`).

```sh
npm install @six5536/agent-harness-kit-node
```

```js
const ahk = require("@six5536/agent-harness-kit-node");

// Install the integration declared in a manifest (see the ahk README).
const result = ahk.install("mytool.harness.toml", ["claude", "codex"]);

// Be a hook command (`command = "mytool hook {harness} {event}"`):
const [harness, event] = process.argv.slice(3);
ahk
  .runHook(harness, event, (hook) =>
    hook.event === "pre-tool" && JSON.stringify(hook.tool).includes("rm -rf")
      ? { answer: "deny", reason: "mytool refuses rm -rf" }
      : { answer: "allow" },
  )
  .then((code) => process.exit(code));
```

Functions: `install`, `status`, `parseHook`, `answerHook`, `runHook`,
`schema`; errors throw with the kit's message. TypeScript types are
included. Prebuilt for Linux (glibc; x64, arm64), macOS (x64, arm64) and
Windows (x64).

MIT licensed.
