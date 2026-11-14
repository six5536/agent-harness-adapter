// Tells whether the install of the shared TypeScript tools was made from their
// current package.json and lock file. After it installs them, `mise run setup`
// writes a stamp of both files into node_modules.

import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

// The folder of the shared tools, from the root of the project.
export const toolsDir = ".config/typescript";

const notInstalled = "the TypeScript tools aren't installed";
const outOfDate = "the install of the TypeScript tools is out of date";

// Returns the path of the stamp of the install in `dir`.
function stampPath(dir: string): string {
  return join(dir, "node_modules", ".setup-stamp");
}

// Returns a hash of the package.json and the lock file in `dir`.
function fingerprint(dir: string): string {
  const hash = createHash("sha256");
  for (const name of ["package.json", "package-lock.json"]) {
    const path = join(dir, name);
    hash.update(`${name}\0${existsSync(path) ? readFileSync(path, "utf8") : ""}\0`);
  }
  return hash.digest("hex");
}

// Records that the install in `dir` was made from its current package.json and
// lock file.
export function writeStamp(dir: string): void {
  writeFileSync(stampPath(dir), `${fingerprint(dir)}\n`);
}

// Returns what is wrong with the install in `dir`, or undefined when it is up
// to date or `dir` holds no tools.
export function checkInstall(dir: string): string | undefined {
  if (!existsSync(join(dir, "package.json"))) {
    return undefined;
  }
  if (!existsSync(stampPath(dir))) {
    return notInstalled;
  }
  return readFileSync(stampPath(dir), "utf8").trim() === fingerprint(dir) ? undefined : outOfDate;
}
