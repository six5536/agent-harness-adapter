// Builds the release binary for one Rust target, and puts it in dist/<platform>.
//
// Usage: launcher-build.mts <rust-target> <platform>

import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

interface Manifest {
  bin?: Record<string, string>;
  os?: string[];
  cpu?: string[];
}

function isManifest(value: unknown): value is Manifest {
  return typeof value === "object" && value !== null;
}

function read(directory: string): Manifest {
  const parsed: unknown = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
  if (!isManifest(parsed)) {
    throw new Error(`${directory}/package.json is not an object`);
  }
  return parsed;
}

function fail(message: string): never {
  console.error(`build: ${message}`);
  process.exit(1);
}

function run(command: string, args: string[]): void {
  const result = spawnSync(command, args, { stdio: "inherit" });
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed`);
  }
}

const target = process.argv.at(2);
const platform = process.argv.at(3);
if (target === undefined || platform === undefined) {
  fail("usage: launcher-build.mts <rust-target> <platform>");
}

const command = Object.keys(read(join("packages", "cli")).bin ?? {}).at(0) ?? fail("no bin");
const binary = target.includes("windows") ? `${command}.exe` : command;
const musl = target.endsWith("-musl");

run("rustup", ["target", "add", target]);
run("cargo", [
  musl ? "zigbuild" : "build",
  "--release",
  "--locked",
  "--target",
  target,
  "--bin",
  command,
]);

const built = join("target", target, "release", binary);
if (musl) {
  const headers = spawnSync("readelf", ["-l", built], { encoding: "utf8" });
  if (headers.stdout.includes("program interpreter")) {
    fail(`${built} is dynamically linked, but it should be a static musl build`);
  }
}

mkdirSync(join("dist", platform), { recursive: true });
copyFileSync(built, join("dist", platform, binary));

const packageDirectory = readdirSync("packages")
  .map((name) => join("packages", name))
  .find((directory) => {
    const manifest = read(directory);
    return `${manifest.os?.join("") ?? ""}-${manifest.cpu?.join("") ?? ""}` === platform;
  });
if (packageDirectory === undefined) {
  fail(`no package under packages/ is for ${platform}`);
}
mkdirSync(join(packageDirectory, "bin"), { recursive: true });
copyFileSync(built, join(packageDirectory, "bin", binary));
chmodSync(join(packageDirectory, "bin", binary), 0o755);

// Only the host's own binary can run here.
if (platform === `${process.platform}-${process.arch}`) {
  run("mise", ["run", "-q", "test:smoke"]);
}
