// Packs the launcher and this machine's platform package, then runs the built
// program through the launcher.

import { spawnSync } from "node:child_process";
import type { SpawnSyncReturns } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import process from "node:process";

interface Manifest {
  name: string;
  bin?: Record<string, string>;
  os?: string[];
  cpu?: string[];
}

function isManifest(value: unknown): value is Manifest {
  return typeof value === "object" && value !== null && "name" in value;
}

function read(directory: string): Manifest {
  const parsed: unknown = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
  if (!isManifest(parsed)) {
    throw new Error(`${directory}/package.json has no name`);
  }
  return parsed;
}

function fail(message: string): never {
  console.error(`smoke: ${message}`);
  process.exit(1);
}

function run(command: string, args: string[], cwd = "."): SpawnSyncReturns<string> {
  const shell = process.platform === "win32" && command === "npm";
  const result = spawnSync(command, args, { cwd, encoding: "utf8", shell });
  if (result.error !== undefined) {
    fail(`failed to run ${command}: ${result.error.message}`);
  }
  return result;
}

function hostPackageDirectory(): string {
  const found = readdirSync("packages")
    .map((name) => join("packages", name))
    .find((directory) => {
      const manifest = read(directory);
      return (
        manifest.os?.includes(process.platform) === true &&
        manifest.cpu?.includes(process.arch) === true
      );
    });
  return found ?? fail(`no package under packages/ is for ${process.platform}-${process.arch}`);
}

function checkLauncher(
  launcherDirectory: string,
  hostDirectory: string,
  binary: string,
  base: string,
): void {
  const launcherName = read(launcherDirectory).name;
  const hostName = read(hostDirectory).name;
  const packed = run("npm", [
    "pack",
    `./${launcherDirectory}`,
    `./${hostDirectory}`,
    "--pack-destination",
    base,
  ]);
  if (packed.status !== 0) {
    fail(`npm pack failed: ${packed.stderr}`);
  }
  const tarballs = packed.stdout.trim().split("\n").slice(-2);
  const destinations = [launcherName, hostName].map((name) => join(base, "node_modules", name));
  for (const [index, tarball] of tarballs.entries()) {
    const destination = destinations[index] ?? fail("npm pack wrote too few tarballs");
    const scratch = `extract-${String(index)}`;
    mkdirSync(join(base, scratch));
    // A relative path, because GNU tar reads the colon of C:\ as a remote host.
    const extracted = run("tar", ["-xzf", tarball, "-C", scratch], base);
    if (extracted.status !== 0) {
      fail(`tar failed on ${tarball}: ${extracted.stderr}`);
    }
    mkdirSync(dirname(destination), { recursive: true });
    renameSync(join(base, scratch, "package"), destination);
  }

  const launcher = join(destinations[0] ?? "", "bin", "cli.js");
  const direct = run(binary, ["--version"]);
  const viaLauncher = run(process.execPath, [launcher, "--version"]);
  if (viaLauncher.status !== 0 || viaLauncher.stdout !== direct.stdout) {
    fail(`the launcher printed "${viaLauncher.stdout}${viaLauncher.stderr}" for --version`);
  }
  const directBad = run(binary, ["--definitely-not-a-flag"]);
  const launcherBad = run(process.execPath, [launcher, "--definitely-not-a-flag"]);
  if (directBad.status === 0 || launcherBad.status !== directBad.status) {
    fail(`the launcher exited with ${String(launcherBad.status)} for a bad flag`);
  }
}

const launcherDirectory = join("packages", "cli");
const command =
  Object.keys(read(launcherDirectory).bin ?? {})[0] ?? fail("the launcher has no bin");
const binaryName = process.platform === "win32" ? `${command}.exe` : command;
const hostDirectory = hostPackageDirectory();
const staged = join(hostDirectory, "bin", binaryName);
const stagedHere = !existsSync(staged);

if (stagedHere) {
  const built = spawnSync("cargo", ["build", "--release", "--locked", "--bin", command], {
    stdio: "inherit",
  });
  if (built.status !== 0) {
    fail("cargo build failed");
  }
  mkdirSync(dirname(staged), { recursive: true });
  copyFileSync(join("target", "release", binaryName), staged);
}

const base = mkdtempSync(join(tmpdir(), "launcher-smoke-"));
try {
  const version = run(resolve(staged), ["--version"]);
  if (version.status !== 0) {
    fail(`${staged} --version exited with ${String(version.status)}`);
  }
  checkLauncher(launcherDirectory, hostDirectory, resolve(staged), base);
  console.log(`smoke OK: ${version.stdout.trim()}, directly and through the launcher`);
} finally {
  rmSync(base, { force: true, recursive: true });
  if (stagedHere) {
    rmSync(staged, { force: true });
  }
}
