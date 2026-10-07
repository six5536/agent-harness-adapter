"""mytool: an example tool in Python, plugged into agent harnesses by `ahk`.

`python3 mytool.py hook` reads one hook input (the hook contract, version 1)
on stdin and writes one answer on stdout. `ahk hook` translates both for
whichever harness ran it.
"""

import json
import sys

REFUSED = ("rm -rf /", "git push --force")


def answer(event):
    """The answer to one hook input."""
    if event.get("v") != 1:
        return {"answer": "allow", "stderr": "mytool: unknown hook contract\n"}
    if event["event"] == "session-start":
        return {"answer": "context", "text": "mytool is watching shell commands."}
    tool = event.get("tool") or {}
    if event["event"] == "pre-tool" and tool.get("kind") == "shell":
        command = json.dumps(tool.get("input"))
        for refused in REFUSED:
            if refused in command:
                return {"answer": "deny", "reason": f"mytool refuses `{refused}`"}
    return {"answer": "allow"}


def main():
    if sys.argv[1:] == ["hook"]:
        print(json.dumps(answer(json.load(sys.stdin))))
    elif sys.argv[1:2] == ["check"]:
        refused = [r for r in REFUSED if r in " ".join(sys.argv[2:])]
        print("refused" if refused else "allowed")
    else:
        sys.exit("usage: mytool.py hook | check <command>")


if __name__ == "__main__":
    main()
