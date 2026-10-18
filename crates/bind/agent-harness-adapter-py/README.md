# six5536-agent-harness-adapter

Python binding of [agent-harness-adapter](https://github.com/six5536/agent-harness-adapter):
one agent setup, every coding agent. Install instructions, skills and hooks
into Claude Code, OpenAI Codex, Gemini CLI, GitHub Copilot, Cursor, Factory
Droid, Pi, OpenCode, Kilo Code, Qwen Code, Devin and any agent that reads
`AGENTS.md`, and answer hooks in one format, Claude Code's, in all of them.

Not yet on PyPI: build it from the repository with
[maturin](https://www.maturin.rs/) (`maturin develop` in this folder) until the first
PyPI release.

```sh
pip install six5536-agent-harness-adapter   # once published
```

```python
import sys
import agent_harness_adapter as aha

# Install a tool's manifest, or the repository's config (manifest None).
result = aha.install("mytool.harness.toml", ["claude", "codex"])

# Be a hook command (`run = "mytool hook"` in the manifest): Claude Code's
# hook input in, Claude Code's answer JSON out, in every harness.
def decide(hook):
    command = (hook.get("tool_input") or {}).get("command", "")
    if hook["hook_event_name"] == "PreToolUse" and "rm -rf" in command:
        return {"decision": "block", "reason": "mytool refuses rm -rf"}
    return None  # allow

sys.exit(aha.run_hook(sys.argv[2], sys.argv[3], decide))
```

Functions: `install`, `uninstall`, `status`, `parse_hook`, `answer_hook`, `run_hook`,
`schema`; errors raise `ValueError` with the library's message. Guide, the
manifest, the hook contract and the API reference:
<https://six5536.github.io/agent-harness-adapter/use/python/>.

MIT licensed.
