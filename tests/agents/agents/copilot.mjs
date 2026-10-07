// GitHub Copilot CLI (`copilot`).
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import { run } from "../lib.mjs";

export default {
  id: "copilot",
  name: "Copilot CLI",
  pkg: "@github/copilot",
  version: "1.0.92",
  bin: "copilot",
  login: (a) => run(a.binPath, ["login", "--device-code"], { env: a.env, stdio: "inherit" }),
  prompt: (a, text) => [
    "-p",
    text,
    "--allow-all",
    "--no-ask-user",
    "--output-format",
    "json",
    ...(a.model ? ["--model", a.model] : []),
  ],
  /**
   * Trust the project, as answering Copilot's trust prompt does: Copilot
   * loads a repository's hooks only from a trusted folder. The list lives in
   * its managed config.json, which `copilot config` will not set. The file
   * holds the login too: it is edited in place and never printed.
   */
  prepare: (a, proj) => {
    const file = join(a.home, ".copilot", "config.json");
    mkdirSync(dirname(file), { recursive: true });
    const text = existsSync(file) ? readFileSync(file, "utf8") : "{}";
    const lines = text.split("\n");
    const head = lines.filter((l) => l.trimStart().startsWith("//"));
    const body = JSON.parse(lines.filter((l) => !l.trimStart().startsWith("//")).join("\n") || "{}");
    body.trustedFolders = [...new Set([...(body.trustedFolders ?? []), proj])];
    writeFileSync(file, `${[...head, JSON.stringify(body, null, 2)].join("\n")}\n`);
  },
  // Copilot takes no context from a hook (HAR-5_AC-5).
  unsupported: ["A4 session-start context", "A7b post-tool context"],
};
