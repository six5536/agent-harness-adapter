"""mytool: an example tool in Python, plugged into agent harnesses by `agent-harness-adapter`.

`python3 mytool.py hook` is a hook in Claude Code's format: it reads Claude
Code's hook input on stdin and answers as Claude Code reads an answer (exit 2
with the reason on stderr blocks; plain stdout at session start is context).
`agent-harness-adapter hook` translates both for whichever harness ran it.
"""

import json
import sys

REFUSED = ("rm -rf /", "git push --force")


def hook(event):
    """Answer one hook input: (exit code, stdout, stderr)."""
    name = event.get("hook_event_name")
    if name == "SessionStart":
        return 0, "mytool is watching shell commands.", ""
    if name == "PreToolUse" and event.get("tool_name") == "Bash":
        command = (event.get("tool_input") or {}).get("command", "")
        for refused in REFUSED:
            if refused in command:
                return 2, "", f"mytool refuses `{refused}`"
    return 0, "", ""


def main():
    if sys.argv[1:] == ["hook"]:
        code, out, err = hook(json.load(sys.stdin))
        print(out, end="")
        print(err, end="", file=sys.stderr)
        sys.exit(code)
    elif sys.argv[1:2] == ["check"]:
        refused = [r for r in REFUSED if r in " ".join(sys.argv[2:])]
        print("refused" if refused else "allowed")
    else:
        sys.exit("usage: mytool.py hook | check <command>")


if __name__ == "__main__":
    main()
