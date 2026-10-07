#!/usr/bin/env node
// Set one version across the whole project in lockstep: the Cargo workspace
// version and its internal crate pins, every package.json under packages/
// with the launcher's pinned optionalDependencies, and both lockfiles.
//
// Usage: node scripts/set-version.mjs <version>

import { readFileSync, writeFileSync, readdirSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const version = process.argv[2]?.replace(/^v/, "");
if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/.test(version)) {
  console.error("usage: node scripts/set-version.mjs <semver>");
  process.exit(1);
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SCOPE = "@six5536/agent-harness-adapter-";

// Cargo.toml: the workspace version and every internal crate's pin.
const cargoPath = join(root, "Cargo.toml");
let cargo = readFileSync(cargoPath, "utf8");
cargo = cargo.replace(/^version = "[^"]*"$/m, `version = "${version}"`);
cargo = cargo.replace(/(= \{ version = ")[^"]*(", path = "crates\/)/g, `$1${version}$2`);
writeFileSync(cargoPath, cargo);

// Every packages/*/package.json: version, plus the launcher's optionalDependencies.
const pkgsDir = join(root, "packages");
for (const name of readdirSync(pkgsDir)) {
  const p = join(pkgsDir, name, "package.json");
  let json;
  try {
    json = JSON.parse(readFileSync(p, "utf8"));
  } catch {
    continue;
  }
  json.version = version;
  for (const dep of Object.keys(json.optionalDependencies ?? {})) {
    if (dep.startsWith(SCOPE)) {
      json.optionalDependencies[dep] = version;
    }
  }
  writeFileSync(p, `${JSON.stringify(json, null, 2)}\n`);
}

// Lockfiles record the members' own versions, so they go stale on a bump.
// Left stale, `cargo publish --locked` fails; refresh both now.
const run = (cmd, args) => {
  try {
    execFileSync(cmd, args, { cwd: root, stdio: "inherit" });
  } catch {
    console.error(`\nfailed: ${cmd} ${args.join(" ")}`);
    console.error("The manifests were updated but the lockfiles are now stale.");
    process.exit(1);
  }
};

run("cargo", ["update", "--workspace", "--offline"]);
run("npm", ["install", "--package-lock-only", "--ignore-scripts", "--silent"]);

console.log(`set version to ${version} across the Cargo workspace, packages/ and both lockfiles`);
