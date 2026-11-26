// Creates the GitHub release of the tag in GITHUB_REF_NAME, with an archive of each
// build in dist/, each file of dist/assets/, their checksums, and the release notes.
// This script puts the files of dist/share/ into every archive, and attaches each file
// of dist/assets/ on its own beside them, such as a schema that a user downloads.

import { spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";
import process from "node:process";

const dist = "dist";
const stage = join(dist, ".stage");
const out = join(dist, ".out");

// The names that the release's own files take beside the assets, in lower case, because
// a file system that ignores case, and GitHub, take two names that differ in case alone
// for one.
const RESERVED = new Set(["sha256sums", "notes.md"]);

// The name of an asset: one that GitHub keeps as it is, that no command reads as an
// option, and that `gh` doesn't split at a `#` into a label.
const ASSET_NAME = /^[A-Za-z0-9_][A-Za-z0-9._-]*$/u;

// Returns the path of each file of `folder` that the release attaches on its own, and
// leaves out a hidden file, as the platforms leave out a hidden folder. It throws on a
// folder that isn't a directory or that holds a folder, and on a file whose name is one
// of the release's own or isn't an asset's name.
export function releaseAssets(folder: string): string[] {
  if (!existsSync(folder)) {
    return [];
  }
  if (!statSync(folder).isDirectory()) {
    throw new Error(`${folder} is a file, not a folder of assets`);
  }
  const files: string[] = [];
  for (const name of readdirSync(folder).toSorted()) {
    if (name.startsWith(".")) {
      continue;
    }
    const path = join(folder, name);
    if (!statSync(path).isFile()) {
      throw new Error(`${path} is no file: an asset is a file of ${folder}`);
    }
    if (RESERVED.has(name.toLowerCase()) || !ASSET_NAME.test(name)) {
      throw new Error(`${path} can't be an asset: rename it`);
    }
    files.push(path);
  }
  return files;
}

// Copies each asset into `folder`, which already holds the release's archives, and throws
// on an asset with the name of a file that `folder` holds, whatever its case.
export function copyAssets(files: readonly string[], folder: string): void {
  const taken = new Set(readdirSync(folder).map((name) => name.toLowerCase()));
  for (const file of files) {
    if (taken.has(basename(file).toLowerCase())) {
      throw new Error(`${file} has the name of an archive of the release: rename it`);
    }
    cpSync(file, join(folder, basename(file)));
  }
}

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

if (import.meta.main) {
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
          (entry) =>
            entry.isDirectory() &&
            !entry.name.startsWith(".") &&
            entry.name !== "share" &&
            entry.name !== "assets",
        )
        .map((entry) => entry.name)
    : [];

  // A run that failed before leaves its archives and assets, which this run replaces.
  rmSync(stage, { force: true, recursive: true });
  rmSync(out, { force: true, recursive: true });
  mkdirSync(out, { recursive: true });
  try {
    const assetFiles = releaseAssets(join(dist, "assets"));
    for (const platform of platforms) {
      packPlatform(platform, version);
    }
    copyAssets(assetFiles, out);
  } catch (error) {
    fail(error instanceof Error ? error.message : String(error));
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
}
