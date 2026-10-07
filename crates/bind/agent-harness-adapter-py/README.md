# six5536-agent-harness-adapter

Python binding of [agent-harness-adapter](https://github.com/six5536/agent-harness-adapter):
plug your tool into LLM agent harnesses (Claude Code, OpenAI Codex, Gemini
CLI, GitHub Copilot, Cursor, Factory Droid, Pi, and any agent that reads
`AGENTS.md`).

```sh
pip install six5536-agent-harness-adapter
```

```python
import sys
import agent_harness_adapter as agent-harness-adapter

# Install the integration declared in a manifest (see the agent-harness-adapter README).
result = agent-harness-adapter.install("mytool.harness.toml", ["claude", "codex"])

# Be a hook command (`command = "mytool hook {harness} {event}"`):
def decide(hook):
    if hook["event"] == "pre-tool" and "rm -rf" in str(hook.get("tool")):
        return {"answer": "deny", "reason": "mytool refuses rm -rf"}
    return {"answer": "allow"}

sys.exit(agent-harness-adapter.run_hook(sys.argv[2], sys.argv[3], decide))
```

Functions: `install`, `uninstall`, `status`, `parse_hook`, `answer_hook`, `run_hook`,
`schema`; errors raise `ValueError` with the library's message. The manifest and
the hook contract are those of the `agent-harness-adapter` command:
<https://github.com/six5536/agent-harness-adapter/tree/main/crates/app/agent-harness-adapter>.

MIT licensed.
