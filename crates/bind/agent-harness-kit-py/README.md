# six5536-agent-harness-kit

Python binding of [agent-harness-kit](https://github.com/six5536/agent-harness-kit):
plug your tool into LLM agent harnesses (Claude Code, OpenAI Codex, Gemini
CLI, GitHub Copilot, Cursor, Factory Droid, Pi, and any agent that reads
`AGENTS.md`).

```sh
pip install six5536-agent-harness-kit
```

```python
import sys
import agent_harness_kit as ahk

# Install the integration declared in a manifest (see the ahk README).
result = ahk.install("mytool.harness.toml", ["claude", "codex"])

# Be a hook command (`command = "mytool hook {harness} {event}"`):
def decide(hook):
    if hook["event"] == "pre-tool" and "rm -rf" in str(hook.get("tool")):
        return {"answer": "deny", "reason": "mytool refuses rm -rf"}
    return {"answer": "allow"}

sys.exit(ahk.run_hook(sys.argv[2], sys.argv[3], decide))
```

Functions: `install`, `status`, `parse_hook`, `answer_hook`, `run_hook`,
`schema`; errors raise `ValueError` with the kit's message. The manifest and
the hook contract are those of the `ahk` command:
<https://github.com/six5536/agent-harness-kit/tree/main/crates/app/agent-harness-kit>.

MIT licensed.
