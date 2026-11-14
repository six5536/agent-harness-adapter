// Lists or sets the version in the workspace's Cargo.toml.
//
// Usage: rust-version.mts list | set <version>

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import process from "node:process";

const manifest = "Cargo.toml";
const internalCrate = /^(?<name>[\w-]+)\s*=\s*\{.*path\s*=/u;
const versionField = /(version\s*=\s*")[^"]*(")/u;

interface VersionLine {
  index: number;
  label: string;
  version: string;
}

function versionLines(lines: string[]): VersionLine[] {
  const found: VersionLine[] = [];
  let section = "";
  for (const [index, line] of lines.entries()) {
    if (line.startsWith("[")) {
      section = line.trim();
    }
    const value = /version\s*=\s*"(?<version>[^"]*)"/u.exec(line)?.groups?.version;
    if (value !== undefined) {
      if (section === "[workspace.package]" && line.startsWith("version")) {
        found.push({ index, label: manifest, version: value });
      }
      const crate = internalCrate.exec(line)?.groups?.name;
      if (section === "[workspace.dependencies]" && crate !== undefined) {
        found.push({ index, label: `${manifest}#${crate}`, version: value });
      }
    }
  }
  return found;
}

const lines = readFileSync(manifest, "utf8").split("\n");
const [mode, version] = process.argv.slice(2);

if (mode === "set") {
  for (const { index } of versionLines(lines)) {
    lines[index] = (lines[index] ?? "").replace(versionField, `$1${version}$2`);
  }
  writeFileSync(manifest, lines.join("\n"));
  if (existsSync("Cargo.lock")) {
    const result = spawnSync("cargo", ["update", "--workspace"], { stdio: "inherit" });
    if (result.status !== 0) {
      process.exitCode = 1;
    }
  }
} else {
  for (const { label, version: found } of versionLines(lines)) {
    console.log(`${label} ${found}`);
  }
}
