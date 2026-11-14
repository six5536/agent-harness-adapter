// Lists or sets the version in each npm package of the project.
//
// Usage: launcher-version.mts list | set <version>

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

interface Manifest {
  name: string;
  version: string;
  optionalDependencies?: Record<string, string>;
}

const packagesDir = "packages";

function isManifest(value: unknown): value is Manifest {
  return typeof value === "object" && value !== null && "name" in value && "version" in value;
}

function manifestPaths(): string[] {
  return readdirSync(packagesDir)
    .map((name) => join(packagesDir, name, "package.json"))
    .filter((path) => existsSync(path));
}

function read(path: string): Manifest {
  const parsed: unknown = JSON.parse(readFileSync(path, "utf8"));
  if (!isManifest(parsed)) {
    throw new Error(`${path} has no name or version`);
  }
  return parsed;
}

function list(): void {
  for (const path of manifestPaths()) {
    const manifest = read(path);
    console.log(`${path} ${manifest.version}`);
    for (const [name, version] of Object.entries(manifest.optionalDependencies ?? {})) {
      console.log(`${path}#${name} ${version}`);
    }
  }
}

function set(version: string): void {
  for (const path of manifestPaths()) {
    const manifest = read(path);
    manifest.version = version;
    for (const name of Object.keys(manifest.optionalDependencies ?? {})) {
      if (manifest.optionalDependencies !== undefined) {
        manifest.optionalDependencies[name] = version;
      }
    }
    writeFileSync(path, `${JSON.stringify(manifest, null, 2)}\n`);
  }
  if (existsSync("package-lock.json")) {
    const npm = process.platform === "win32" ? "npm.cmd" : "npm";
    const args = ["install", "--package-lock-only", "--ignore-scripts", "--no-audit", "--no-fund"];
    const result = spawnSync(npm, args, { stdio: "inherit", shell: process.platform === "win32" });
    if (result.status !== 0) {
      process.exitCode = 1;
    }
  }
}

const [mode, version] = process.argv.slice(2);
if (mode === "set") {
  set(version);
} else {
  list();
}
