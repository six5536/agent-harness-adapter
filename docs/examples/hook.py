# region: hook
# mytool hook <harness> <event>: Claude Code's input in, its answer out,
# translated for whichever agent ran the hook.
import sys

import agent_harness_adapter as aha


def decide(hook):
    if "rm -rf" in str((hook.get("tool_input") or {}).get("command", "")):
        return {"decision": "block", "reason": "mytool: no rm -rf here"}
    return None  # allow


sys.exit(aha.run_hook(sys.argv[1], sys.argv[2], decide))
# endregion: hook
