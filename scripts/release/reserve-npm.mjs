#!/usr/bin/env node
// Publish a placeholder of every npm package, so each exists on npm and its
// trusted publisher (OIDC from release.yml) can be set up before the first
// real release. Run once, by hand, after `npm login`:
//
//   node scripts/release/reserve-npm.mjs            # list what it would do
//   node scripts/release/reserve-npm.mjs --publish  # publish the placeholders
//
// Each placeholder is version 0.0.0-reserved.0 under the dist-tag `reserved`,
// holding only a package.json and a README; nothing installs it by default.
// A package that already exists on npm is skipped.

import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const VERSION = "0.0.0-reserved.0";
const TAG = "reserved";
const publish = process.argv.includes("--publish");
const npm = process.platform === "win32" ? "npm.cmd" : "npm";

function run(args, opts = {}) {
  return spawnSync(npm, args, { encoding: "utf8", shell: process.platform === "win32", ...opts });
}

const packages = readdirSync(join(root, "packages"))
  .map((d) => JSON.parse(readFileSync(join(root, "packages", d, "package.json"), "utf8")))
  .sort((a, b) => a.name.localeCompare(b.name));

if (publish && run(["whoami"]).status !== 0) {
  console.error("reserve-npm: not logged in to npm; run `npm login` first");
  process.exit(2);
}

let failed = false;
for (const p of packages) {
  const exists = run(["view", p.name, "name"]).status === 0;
  if (exists) {
    console.log(`skip     ${p.name} (already on npm)`);
    continue;
  }
  if (!publish) {
    console.log(`would    ${p.name}@${VERSION}`);
    continue;
  }
  const dir = mkdtempSync(join(tmpdir(), "aha-reserve-"));
  try {
    writeFileSync(
      join(dir, "package.json"),
      `${JSON.stringify(
        {
          name: p.name,
          version: VERSION,
          description: `Placeholder for ${p.name}, part of agent-harness-adapter; the first release is 0.1.0.`,
          license: p.license,
          repository: p.repository,
          publishConfig: { access: "public" },
        },
        null,
        2,
      )}\n`,
    );
    writeFileSync(
      join(dir, "README.md"),
      `# ${p.name}\n\nA placeholder: the first release of this package is 0.1.0.\nSee https://github.com/six5536/agent-harness-adapter.\n`,
    );
    const r = run(["publish", dir, "--tag", TAG, "--access", "public"], { stdio: "inherit" });
    if (r.status === 0) console.log(`reserved ${p.name}@${VERSION}`);
    else {
      console.error(`failed   ${p.name}`);
      failed = true;
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
process.exit(failed ? 1 : 0);
