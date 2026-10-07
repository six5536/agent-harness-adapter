#!/usr/bin/env node
// Behavioural smoke test for a compiled ahk binary: run the real thing and
// assert what a user sees. Release CI runs it against each built artifact its
// runner can execute; locally run `npm run smoke` after
// `cargo build --release -p agent-harness-kit`.
//
// It mirrors crates/app/agent-harness-kit/tests: those tests prove the code is
// right, this proves the shipped artifact is. Extend both together.

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join, resolve } from "node:path";

const bin = resolve(
  process.argv[2] ?? join("target", "release", process.platform === "win32" ? "ahk.exe" : "ahk"),
);

function fail(message) {
  console.error(`release-smoke: ${message}`);
  process.exit(1);
}

if (!existsSync(bin)) {
  fail(`no binary at ${bin}: build first: cargo build --release -p agent-harness-kit`);
}

/** Run the binary, asserting the exit status; returns the spawn result. */
function run(args, expectStatus, opts = {}) {
  const r = spawnSync(bin, args, { encoding: "utf8", ...opts });
  if (r.error) {
    fail(`failed to run ${bin}: ${r.error.message}`);
  }
  if (r.status !== expectStatus) {
    console.error(r.stdout);
    console.error(r.stderr);
    fail(`\`ahk ${args.join(" ")}\` exited ${r.status}, expected ${expectStatus}`);
  }
  return r;
}

const version = run(["--version"], 0).stdout.trim();
if (!/^ahk \d+\.\d+\.\d+/.test(version)) {
  fail(`unexpected --version output: ${version}`);
}
run(["--definitely-not-a-flag"], 2);

console.log(`release-smoke OK: ${version}`);
