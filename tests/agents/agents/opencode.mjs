// OpenCode (`opencode`).
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { run } from "../lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));

export default {
  id: "opencode",
  name: "OpenCode",
  pkg: "opencode-ai",
  version: "1.18.35",
  bin: "opencode",
  login: (a) => run(a.binPath, ["auth", "login"], { env: a.env, stdio: "inherit" }),
  prompt: (a, text) => ["run", "--auto", ...(a.model ? ["--model", a.model] : []), text],
  // `opencode run` exits when the session first goes idle, before a stop
  // hook can continue it: ask through OpenCode's server instead.
  ask: (a, text, cwd) =>
    run(process.execPath, [join(here, "opencode-ask.mjs"), a.binPath, text, ...(a.model ? [a.model] : [])], {
      cwd,
      env: a.env,
    }),
  // What OpenCode itself adds to a project's `.opencode` (it installs its
  // plugin package there), not the tool's.
  own: (file) => /^\.opencode\/(\.gitignore|package(-lock)?\.json|bun\.lockb?|node_modules\/)/.test(file),
};
