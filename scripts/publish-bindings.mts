// Publishes the bindings, with the artefacts that the builds left in dist/:
//
//   publish-bindings.mts node
//     the Node addon's platform packages, each with the index.node that
//     dist/node-<platform>.node holds, and then the Node binding, which names them as
//     optional dependencies, to npm, leaving out any version already
//     published. NPM_TOKEN is a fallback for trusted publishing.
//   publish-bindings.mts python
//     the wheels and the source distribution in dist/ to PyPI, through
//     its trusted publishing, when PUBLISH_PYPI is true, leaving out any file
//     already published.
//
// PUBLISH_DRY_RUN=true makes each publish a dry run, and PUBLISH_DISABLED, with
// its reason as its value, refuses every publish.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { fail, run, windows } from "./lib/run.mts";

const name = "publish-bindings";
const root = join("bindings", "packages");
const binding = "agent-harness-adapter-node";
const dryRun = process.env.PUBLISH_DRY_RUN === "true";

interface Manifest {
  name: string;
  version: string;
  os?: string[];
}

function isManifest(value: unknown): value is Manifest {
  return typeof value === "object" && value !== null && "name" in value && "version" in value;
}

function read(directory: string): Manifest {
  const parsed: unknown = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
  if (!isManifest(parsed)) {
    fail(name, `${directory}/package.json has no name or version`);
  }
  return parsed;
}

// The token goes to npm in its environment, where no other process and no log
// of npm's arguments sees it.
function npm(args: string[], inherit: boolean, token = ""): number | null {
  const env =
    token === ""
      ? process.env
      : { ...process.env, "npm_config_//registry.npmjs.org/:_authToken": token };
  return spawnSync(windows ? "npm.cmd" : "npm", args, {
    env,
    shell: windows,
    stdio: inherit ? "inherit" : "ignore",
  }).status;
}

function publishNode(): void {
  const platforms = readdirSync(root)
    .filter((entry) => entry.startsWith(`${binding}-`))
    .map((entry) => join(root, entry));
  const token = process.env.NPM_TOKEN ?? "";
  for (const directory of [...platforms, join(root, binding)]) {
    const manifest = read(directory);
    const spec = `${manifest.name}@${manifest.version}`;
    if (npm(["view", spec, "version"], false) === 0) {
      console.log(`${name}: ${spec} is already published`);
      continue;
    }
    if (manifest.os !== undefined) {
      const platform = directory.slice(join(root, `${binding}-`).length);
      const addon = join("dist", `node-${platform}.node`);
      if (!existsSync(addon)) {
        fail(name, `${addon} wasn't built`);
      }
      copyFileSync(addon, join(directory, "index.node"));
    }
    const tag = manifest.version.includes("-") ? "next" : "latest";
    const args = ["publish", `./${directory}`, "--tag", tag];
    if (dryRun) {
      args.push("--dry-run");
    }
    if (npm(args, true, token) !== 0) {
      fail(name, `npm publish failed for ${spec}`);
    }
  }
}

function publishPython(): void {
  if (process.env.PUBLISH_PYPI !== "true") {
    console.log(`${name}: PyPI is off until PUBLISH_PYPI is true, so the wheels stay unpublished`);
    return;
  }
  const files = existsSync("dist")
    ? readdirSync("dist")
        .filter((file) => file.endsWith(".whl") || file.endsWith(".tar.gz"))
        .map((file) => join("dist", file))
    : [];
  if (
    !files.some((file) => file.endsWith(".whl")) ||
    !files.some((file) => file.endsWith(".tar.gz"))
  ) {
    fail(name, "dist/ holds no wheel or no source distribution");
  }
  run(name, "uv", [
    "publish",
    "--trusted-publishing",
    "always",
    "--check-url",
    "https://pypi.org/simple/",
    ...(dryRun ? ["--dry-run"] : []),
    ...files,
  ]);
}

const disabled = process.env.PUBLISH_DISABLED ?? "";
if (disabled !== "") {
  fail(name, `publishing is disabled here: ${disabled}`);
}

const kind = process.argv[2];
if (kind === "node") {
  publishNode();
} else if (kind === "python") {
  publishPython();
} else {
  fail(name, "usage: publish-bindings.mts node | python");
}
