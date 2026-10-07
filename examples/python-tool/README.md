# Example: a Python tool

`mytool.py` refuses destructive shell commands. `mytool.harness.toml`
declares its instructions, a skill and two bridged hooks; `agent-harness-adapter` installs
them and runs the hooks.

```sh
agent-harness-adapter install --manifest examples/python-tool/mytool.harness.toml --harness claude,codex,gemini
```

`test_example.py` installs it into a temporary project and runs each
installed hook command the way Claude Code, Codex and Gemini CLI would, with
their own input. CI runs it:

```sh
cargo build -p agent-harness-adapter
python3 examples/python-tool/test_example.py target/debug/agent-harness-adapter
```
