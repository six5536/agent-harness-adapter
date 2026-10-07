// Kilo Code (`kilo`), a fork of OpenCode.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { run } from "../lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));
// Kilo's gateway serves this model without an account.
const FREE = "kilo/kilo-auto/free";

export default {
  id: "kilo",
  name: "Kilo Code",
  pkg: "@kilocode/cli",
  version: "7.8.7",
  bin: "kilo",
  login: (a) => run(a.binPath, ["auth", "login"], { env: a.env, stdio: "inherit" }),
  prompt: (a, text) => ["run", "--auto", "-m", a.model ?? FREE, text],
  // As OpenCode: follow the session through Kilo's server, so a stop hook's
  // continuation is seen whenever it starts.
  ask: (a, text, cwd) =>
    run(process.execPath, [join(here, "serve-ask.mjs"), a.binPath, text, a.model ?? FREE], {
      cwd,
      env: a.env,
    }),
  // What Kilo itself adds to a project's `.kilo` (it installs its plugin
  // package there), not the tool's.
  own: (file) =>
    /^\.kilo\/(\.gitignore|package(-lock)?\.json|bun\.lockb?|pnpm-lock\.yaml|yarn\.lock|agent-manager\.json|node_modules\/)/.test(
      file,
    ),
};
