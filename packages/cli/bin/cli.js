#!/usr/bin/env node
"use strict";

// Finds the prebuilt binary for this platform and runs it, with the launcher's
// arguments and standard streams. The launcher exits with the binary's code.

const { spawnSync } = require("node:child_process");
const { resolveBinary, exitCode, commandName } = require("../lib/binary");

let binary;
try {
  binary = resolveBinary(process.platform, process.arch);
} catch (err) {
  process.stderr.write(`${err.message}\n`);
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });

if (result.error) {
  process.stderr.write(`failed to run ${commandName()}: ${result.error.message}\n`);
  process.exit(1);
}

process.exit(exitCode(result));
