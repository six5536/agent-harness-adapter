// Publishes the platform packages and then the launcher to npm, with the binaries
// from dist/, leaving out any version already published.

import { spawnSync } from "node:child_process";
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";

interface Manifest {
  name: string;
  version: string;
  bin?: Record<string, string>;
  os?: string[];
  cpu?: string[];
}

function isManifest(value: unknown): value is Manifest {
  return typeof value === "object" && value !== null && "name" in value && "version" in value;
}

function read(directory: string): Manifest {
  const parsed: unknown = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
  if (!isManifest(parsed)) {
    throw new Error(`${directory}/package.json has no name or version`);
  }
  return parsed;
}

function fail(message: string): never {
  console.error(`publish:launcher: ${message}`);
  process.exit(1);
}

function npm(args: string[], inherit: boolean): number | null {
  const shell = process.platform === "win32";
  return spawnSync("npm", args, { shell, stdio: inherit ? "inherit" : "ignore" }).status;
}

function stageBinary(directory: string, manifest: Manifest, command: string): void {
  const platform = manifest.os?.join("") ?? "";
  const binary = platform === "win32" ? `${command}.exe` : command;
  const source = join("dist", `${platform}-${manifest.cpu?.join("") ?? ""}`, binary);
  if (!existsSync(source)) {
    fail(`${source} wasn't built`);
  }
  const destination = join(directory, "bin", binary);
  mkdirSync(join(directory, "bin"), { recursive: true });
  copyFileSync(source, destination);
  chmodSync(destination, 0o755);
}

const disabled = process.env.PUBLISH_DISABLED ?? "";
if (disabled !== "") {
  fail(`publishing is disabled here: ${disabled}`);
}

const launcher = join("packages", "cli");
const command = Object.keys(read(launcher).bin ?? {}).at(0) ?? fail("the launcher has no bin");
const platforms = readdirSync("packages")
  .map((name) => join("packages", name))
  .filter((directory) => read(directory).os !== undefined);
const dryRun = process.env.PUBLISH_DRY_RUN === "true";
const token = process.env.NPM_TOKEN;

for (const directory of [...platforms, launcher]) {
  const manifest = read(directory);
  const spec = `${manifest.name}@${manifest.version}`;
  if (npm(["view", spec, "version"], false) === 0) {
    console.log(`publish:launcher: ${spec} is already published`);
  } else {
    if (manifest.os !== undefined) {
      stageBinary(directory, manifest, command);
    }
    const tag = manifest.version.includes("-") ? "next" : "latest";
    const args = ["publish", `./${directory}`, "--tag", tag];
    if (dryRun) {
      args.push("--dry-run");
    }
    if (token !== undefined && token !== "") {
      args.push(`--//registry.npmjs.org/:_authToken=${token}`);
    }
    if (npm(args, true) !== 0) {
      fail(`npm publish failed for ${spec}`);
    }
  }
}
