# @six5536/agent-harness-adapter-node

Node binding of [agent-harness-adapter](https://github.com/six5536/agent-harness-adapter):
one agent setup, every coding agent. Install instructions, skills and hooks
into Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor, Factory
Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin and any agent that reads
`AGENTS.md`, and answer hooks in one format, Claude Code's, in all of them.

```sh
npm install @six5536/agent-harness-adapter-node
```

```js
const aha = require("@six5536/agent-harness-adapter-node");

// Install a tool's manifest, or the repository's config (manifest null).
const result = aha.install("mytool.harness.toml", ["claude", "codex"]);

// Be a hook command (`run = "mytool hook"` in the manifest): Claude Code's
// hook input in, Claude Code's answer JSON out, in every harness.
const [harness, event] = process.argv.slice(3);
aha
  .runHook(harness, event, (hook) =>
    hook.hook_event_name === "PreToolUse" && String(hook.tool_input?.command).includes("rm -rf")
      ? { decision: "block", reason: "mytool refuses rm -rf" }
      : null,
  )
  .then((code) => process.exit(code));
```

Functions: `install`, `uninstall`, `status`, `parseHook`, `answerHook`, `runHook`,
`schema`; errors throw with the library's message. TypeScript types are
included. Guide and API reference:
<https://six5536.github.io/agent-harness-adapter/use/node/>. Prebuilt for Linux (glibc; x64, arm64), macOS (x64, arm64) and
Windows (x64).

MIT licensed.
