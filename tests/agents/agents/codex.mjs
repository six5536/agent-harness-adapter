// OpenAI Codex (`codex`).
import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { run } from "../lib.mjs";

export default {
  id: "codex",
  name: "Codex",
  pkg: "@openai/codex",
  version: "0.160.1",
  bin: "codex",
  flags: {
    // Codex's sandbox cannot start where user namespaces are not allowed
    // (e.g. dev containers).
    "full-access": "run Codex without its sandbox",
  },
  login: (a) => run(a.binPath, ["login", "--device-auth"], { env: a.env, stdio: "inherit" }),
  prompt: (a, text) => [
    "exec",
    // Stands in for approving new hooks in Codex's /hooks (check A8).
    "--dangerously-bypass-hook-trust",
    "--sandbox",
    a.flags["full-access"] ? "danger-full-access" : "workspace-write",
    ...(a.model ? ["--model", a.model] : []),
    text,
  ],
  /** Trust the project, as answering Codex's trust prompt does. */
  prepare: (a, proj) => {
    const dir = join(a.home, ".codex");
    mkdirSync(dir, { recursive: true });
    const file = join(dir, "config.toml");
    const table = `[projects.${JSON.stringify(proj)}]`;
    const text = existsSync(file) ? readFileSync(file, "utf8") : "";
    if (!text.includes(table)) appendFileSync(file, `\n${table}\ntrust_level = "trusted"\n`);
  },
};
