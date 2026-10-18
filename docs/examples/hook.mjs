// region: hook
// mytool hook <harness> <event>: Claude Code's input in, its answer out,
// translated for whichever agent ran the hook.
import aha from "@six5536/agent-harness-adapter-node";

const [harness, event] = process.argv.slice(2);
const code = await aha.runHook(harness, event, (hook) =>
  String(hook.tool_input?.command ?? "").includes("rm -rf")
    ? { decision: "block", reason: "mytool: no rm -rf here" }
    : null,
);
process.exit(code);
// endregion: hook
