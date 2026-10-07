"""End to end: install mytool with `ahk`, then run each installed hook
command as the harness would, with that harness's own input.

    python3 examples/python-tool/test_example.py <path to ahk>
"""

import json
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))

# Per harness: the settings file, the event keys, and a shell tool call in
# that harness's own shape.
HARNESSES = {
    "claude": (".claude/settings.json", "SessionStart", "PreToolUse",
               {"hook_event_name": "PreToolUse", "tool_name": "Bash",
                "tool_input": {"command": "rm -rf /"}}),
    "codex": (".codex/hooks.json", "SessionStart", "PreToolUse",
              {"hook_event_name": "PreToolUse", "tool_name": "shell",
               "tool_input": {"command": ["bash", "-lc", "rm -rf /"]}}),
    "gemini": (".gemini/settings.json", "SessionStart", "BeforeTool",
               {"hook_event_name": "BeforeTool", "tool_name": "run_shell_command",
                "tool_input": {"command": "rm -rf /"}}),
}


def commands(settings, event):
    """The hook commands under `event` in a Claude-style settings file."""
    groups = settings["hooks"][event]
    return [h["command"] for g in groups for h in g["hooks"]]


def run(command, cwd, stdin):
    out = subprocess.run(command, shell=True, cwd=cwd, input=json.dumps(stdin),
                         capture_output=True, text=True, check=False)
    return out.returncode, out.stdout, out.stderr


def main():
    ahk = os.path.abspath(sys.argv[1])
    env_path = os.path.dirname(ahk) + os.pathsep + os.environ.get("PATH", "")
    os.environ["PATH"] = env_path
    with tempfile.TemporaryDirectory() as proj:
        shutil.copy(os.path.join(HERE, "mytool.py"), proj)
        manifest = os.path.join(HERE, "mytool.harness.toml")
        subprocess.run([ahk, "install", "--manifest", manifest, "--harness",
                        ",".join(HARNESSES), "--root", proj], check=True)
        for harness, (file, start, pre_tool, call) in HARNESSES.items():
            with open(os.path.join(proj, file), encoding="utf-8") as f:
                settings = json.load(f)
            [start_cmd] = commands(settings, start)
            [pre_cmd] = commands(settings, pre_tool)
            code, out, err = run(start_cmd, proj, {"hook_event_name": start})
            assert code == 0 and "mytool is watching" in out, (harness, out, err)
            code, out, err = run(pre_cmd, proj, call)
            assert "mytool refuses `rm -rf /`" in out + err, (harness, code, out, err)
            safe = json.loads(json.dumps(call).replace("rm -rf /", "ls"))
            code, out, err = run(pre_cmd, proj, safe)
            assert code == 0 and "refuses" not in out, (harness, out, err)
            print(f"{harness}: ok")
    print("example OK")


if __name__ == "__main__":
    main()
