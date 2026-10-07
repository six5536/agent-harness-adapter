// Claude Code (`claude`).
import { run } from "../lib.mjs";

export default {
  id: "claude",
  name: "Claude Code",
  pkg: "@anthropic-ai/claude-code",
  version: "2.1.292",
  bin: "claude",
  login: (a) => run(a.binPath, ["auth", "login"], { env: a.env, stdio: "inherit" }),
  // -p skips the workspace trust dialog. The hooks refuse what they must, so
  // tool permissions are bypassed. stream-json holds every message.
  prompt: (a, text) => [
    "-p",
    "--output-format",
    "stream-json",
    "--verbose",
    "--dangerously-skip-permissions",
    ...(a.model ? ["--model", a.model] : []),
    text,
  ],
};
