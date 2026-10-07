#!/usr/bin/env node
// Assert that one version is used everywhere: the Cargo workspace and its
// internal crate pins, every package.json under packages/, the launcher's
// optionalDependencies, and both lockfiles.
//
// Usage: node scripts/verify-version.mjs [expected-version]
//
// With no argument it checks only that they agree. With one, it also checks
// they match it: the release workflow verifies a tag against the tree so.

import { readFileSync, readdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const expected = process.argv[2]?.replace(/^v/, "");
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SCOPE = "@six5536/agent-harness-kit-";

const found = [];
const problems = [];
const record = (where, version) => {
  if (version === undefined) {
    problems.push(`could not read a version from ${where}`);
    return;
  }
  found.push({ where, version });
};

const cargo = readFileSync(join(root, "Cargo.toml"), "utf8");
record("Cargo.toml [workspace.package] version", cargo.match(/^version = "([^"]*)"$/m)?.[1]);
const crates = [];
for (const [, name, version] of cargo.matchAll(/^([\w-]+) = \{ version = "([^"]*)", path = "crates\//gm)) {
  record(`Cargo.toml ${name} dependency pin`, version);
}
for (const [, path] of cargo.matchAll(/"(crates\/[^"]+)"/g)) {
  const manifest = readFileSync(join(root, path, "Cargo.toml"), "utf8");
  const name = manifest.match(/^name = "([^"]*)"$/m)?.[1];
  if (name && !crates.includes(name)) crates.push(name);
}

for (const name of readdirSync(join(root, "packages"))) {
  let json;
  try {
    json = JSON.parse(readFileSync(join(root, "packages", name, "package.json"), "utf8"));
  } catch {
    continue;
  }
  record(`packages/${name}/package.json version`, json.version);
  for (const [dep, range] of Object.entries(json.optionalDependencies ?? {})) {
    if (dep.startsWith(SCOPE)) {
      record(`packages/${name}/package.json optionalDependencies["${dep}"]`, range);
    }
  }
}

const cargoLock = readFileSync(join(root, "Cargo.lock"), "utf8");
for (const crate of crates) {
  const re = new RegExp(`name = "${crate}"\\nversion = "([^"]*)"`);
  record(`Cargo.lock ${crate}`, cargoLock.match(re)?.[1]);
}

const npmLock = JSON.parse(readFileSync(join(root, "package-lock.json"), "utf8"));
for (const [key, entry] of Object.entries(npmLock.packages ?? {})) {
  if (key.startsWith("packages/") && entry.version) {
    record(`package-lock.json ${key}`, entry.version);
  }
}

const versions = [...new Set(found.map((f) => f.version))];
const target = expected ?? versions[0];
if (versions.length !== 1) {
  problems.push(`inconsistent versions across the tree: ${versions.join(", ")}`);
}
for (const { where, version } of found) {
  if (version !== target) {
    problems.push(`${where}: found ${version}, expected ${target}`);
  }
}

if (problems.length > 0) {
  console.error("version check failed:");
  for (const p of problems) console.error(`  - ${p}`);
  console.error("\nRun `npm run set-version <version>` to fix.");
  process.exit(1);
}

console.log(`version ${target} is consistent across ${found.length} locations`);
