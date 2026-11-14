// Creates the GitHub release of the tag in GITHUB_REF_NAME, with an archive of each
// build in dist/, their checksums, and the release notes.

import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

const dist = "dist";
const stage = join(dist, ".stage");
const out = join(dist, ".out");

function fail(message: string): never {
  console.error(`publish:github: ${message}`);
  process.exit(1);
}

function run(command: string, args: string[], cwd = "."): string {
  const result = spawnSync(command, args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed: ${result.stderr}`);
  }
  return result.stdout;
}

function packPlatform(platform: string, version: string): void {
  const binary = readdirSync(join(dist, platform)).at(0);
  if (binary === undefined) {
    fail(`${join(dist, platform)} is empty`);
  }
  const name = `${binary.replace(/\.exe$/u, "")}-${version}-${platform}`;
  const folder = join(stage, name);
  mkdirSync(folder, { recursive: true });
  cpSync(join(dist, platform), folder, { recursive: true });
  for (const file of ["README.md", "LICENSE"].filter((path) => existsSync(path))) {
    cpSync(file, join(folder, file));
  }
  if (existsSync(join(dist, "share"))) {
    cpSync(join(dist, "share"), join(folder, "share"), { recursive: true });
  }
  run("chmod", ["-R", "a+rx", folder]);
  if (platform.startsWith("win32")) {
    run("zip", ["-qr", join("..", ".out", `${name}.zip`), name], stage);
  } else {
    run("tar", ["-czf", join("..", ".out", `${name}.tar.gz`), name], stage);
  }
}

const disabled = process.env.PUBLISH_DISABLED ?? "";
if (disabled !== "") {
  fail(`publishing is disabled here: ${disabled}`);
}

const tag = process.env.GITHUB_REF_NAME ?? fail("GITHUB_REF_NAME names no tag");
const version = tag.replace(/^v/u, "");
const dryRun = process.env.PUBLISH_DRY_RUN === "true";
const platforms = existsSync(dist)
  ? readdirSync(dist, { withFileTypes: true })
      .filter(
        (entry) => entry.isDirectory() && !entry.name.startsWith(".") && entry.name !== "share",
      )
      .map((entry) => entry.name)
  : [];

mkdirSync(out, { recursive: true });
for (const platform of platforms) {
  packPlatform(platform, version);
}
const files = readdirSync(out);
if (files.length > 0) {
  const sums = run("sha256sum", files, out);
  writeFileSync(join(out, "SHA256SUMS"), sums);
}

const notes = join(out, "notes.md");
writeFileSync(notes, run("node", [".config/mise/scripts/changelog.mts", "notes", version]));
const assets = readdirSync(out)
  .filter((file) => file !== "notes.md")
  .map((file) => join(out, file));
console.log(`publish:github: ${tag} with ${String(assets.length)} files`);

if (!dryRun) {
  const exists = spawnSync("gh", ["release", "view", tag], { stdio: "ignore" }).status === 0;
  if (exists) {
    console.log(`publish:github: ${tag} is already published`);
  } else {
    const prerelease = version.includes("-") ? ["--prerelease"] : [];
    run("gh", [
      "release",
      "create",
      tag,
      "--title",
      tag,
      "--notes-file",
      notes,
      ...prerelease,
      ...assets,
    ]);
  }
}
rmSync(stage, { force: true, recursive: true });
rmSync(out, { force: true, recursive: true });
