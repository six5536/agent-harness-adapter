// Checks or prints the release notes of a version, which come from the message of
// its tag or from CHANGELOG.md.
//
// Usage: changelog.mts check|notes <version>

import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import process from "node:process";

function tagNotes(version: string): string {
  const tag = `refs/tags/v${version}`;
  spawnSync("git", ["fetch", "--force", "origin", `${tag}:${tag}`], { stdio: "ignore" });
  return spawnSync("git", ["for-each-ref", tag, "--format=%(contents:body)"], {
    encoding: "utf8",
  }).stdout.trim();
}

function changelogNotes(version: string): string {
  if (!existsSync("CHANGELOG.md")) {
    return "";
  }
  const lines = readFileSync("CHANGELOG.md", "utf8").split("\n");
  const start = lines.findIndex((line) => line.startsWith(`## [${version}]`));
  if (start === -1) {
    return "";
  }
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => line.startsWith("## ["));
  return (end === -1 ? rest : rest.slice(0, end)).join("\n").trim();
}

const mode = process.argv.at(2);
const version = process.argv.at(3);
if (version === undefined) {
  console.error("usage: changelog.mts check|notes <version>");
  process.exit(2);
}
const notes = tagNotes(version) || changelogNotes(version);
if (notes === "") {
  console.error(`changelog: version ${version} has no release notes`);
  process.exit(1);
}
if (mode === "notes") {
  console.log(notes);
}
