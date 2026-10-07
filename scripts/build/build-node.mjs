#!/usr/bin/env node
// Build the Node addon for the host and copy it where tests (and a local
// `AHA_NODE_ADDON`) find it: target/<profile>/agent_harness_adapter_node.node.
//
// Usage: node scripts/build/build-node.mjs [--release] [--target <triple>] [--out <file>]

import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const release = args.includes("--release");
const target = args.includes("--target") ? args[args.indexOf("--target") + 1] : null;
const out = args.includes("--out") ? args[args.indexOf("--out") + 1] : null;

const cargo = ["build", "--locked", "-p", "agent-harness-adapter-node"];
if (release) cargo.push("--release");
if (target) cargo.push("--target", target);
execFileSync("cargo", cargo, { stdio: "inherit" });

const dir = join("target", ...(target ? [target] : []), release ? "release" : "debug");
const platform = target ?? process.platform;
const lib = /windows|win32/.test(platform)
  ? "agent_harness_adapter_node.dll"
  : /apple|darwin/.test(platform)
    ? "libagent_harness_adapter_node.dylib"
    : "libagent_harness_adapter_node.so";
const dest = out ?? join(dir, "agent_harness_adapter_node.node");
copyFileSync(join(dir, lib), dest);
console.log(dest);
