// Builds the Node addon of the Node binding for the host, or for a target, and
// copies it to where the tests and a local `AHA_NODE_ADDON` find it:
// target/<profile>/agent_harness_adapter_node.node, or the file that --out
// names. With --glibc, it builds a Linux target with zig against that version
// of glibc, such as 2.17, so that one addon serves every distribution with that
// glibc or a later one.
//
// Usage: build-node.mts [--release] [--target <triple>] [--glibc <version>] [--out <file>]

import { copyFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import process from "node:process";
import { parseArgs } from "node:util";
import { fail, run, targetDir } from "./lib/run.mts";

const { values } = parseArgs({
  args: process.argv.slice(2),
  options: {
    release: { type: "boolean", default: false },
    target: { type: "string" },
    glibc: { type: "string" },
    out: { type: "string" },
  },
});

const zig = values.glibc !== undefined;
if (zig && values.target?.endsWith("-linux-gnu") !== true) {
  fail("build-node", "--glibc needs a Linux --target, such as x86_64-unknown-linux-gnu");
}
const cargo = [zig ? "zigbuild" : "build", "--locked", "-p", "agent-harness-adapter-node"];
if (values.release) {
  cargo.push("--release");
}
if (values.target !== undefined) {
  cargo.push("--target", zig ? `${values.target}.${values.glibc}` : values.target);
}
run("build-node", "cargo", cargo);

const profile = values.release ? "release" : "debug";
const directory =
  values.target === undefined ? join(targetDir, profile) : join(targetDir, values.target, profile);
const platform = values.target ?? process.platform;
let library = "libagent_harness_adapter_node.so";
if (/windows|win32/u.test(platform)) {
  library = "agent_harness_adapter_node.dll";
} else if (/apple|darwin/u.test(platform)) {
  library = "libagent_harness_adapter_node.dylib";
}
const destination = values.out ?? join(directory, "agent_harness_adapter_node.node");
mkdirSync(dirname(destination), { recursive: true });
copyFileSync(join(directory, library), destination);
console.log(destination);
