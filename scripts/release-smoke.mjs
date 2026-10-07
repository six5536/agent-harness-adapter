#!/usr/bin/env node
// Behavioural smoke test for a compiled ahk binary: run the real thing and
// assert what a user sees. Release CI runs it against each built artifact its
// runner can execute; locally run `npm run smoke` after
// `cargo build --release -p agent-harness-kit`.
//
// It mirrors crates/app/agent-harness-kit/tests: those tests prove the code is
// right, this proves the shipped artifact is. Extend both together.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
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

const dir = mkdtempSync(join(tmpdir(), "ahk-release-smoke-"));
try {
  // install / status from a manifest.
  const manifest = join(dir, "smoke.harness.toml");
  writeFileSync(
    manifest,
    'version = 1\nname = "smoke"\ninstructions = "Use smoke.\\n"\n[[hooks]]\nevent = "stop"\nrun = "smoke decide"\n',
  );
  const proj = join(dir, "proj");
  mkdirSync(proj);
  const env = { ...process.env, HOME: dir, USERPROFILE: dir };
  const installed = run(["install", "--manifest", manifest, "--harness", "claude", "--root", proj], 0, { env });
  if (!installed.stdout.includes("created CLAUDE.md (instructions)")) {
    fail(`install did not report CLAUDE.md:\n${installed.stdout}`);
  }
  if (!readFileSync(join(proj, ".claude", "settings.json"), "utf8").includes("ahk hook --tool smoke claude stop")) {
    fail("install did not write the bridged hook");
  }
  const status = JSON.parse(run(["status", "--manifest", manifest, "--root", proj, "--json"], 0, { env }).stdout);
  if (status.harnesses[0]?.parts[0]?.state !== "current") {
    fail(`status after install is not current: ${JSON.stringify(status)}`);
  }

  // The hook bridge, with node as the tool: it reads the contract's input
  // and asks Claude Code to continue.
  const tool = [
    "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{",
    "const i=JSON.parse(s);",
    "console.log(JSON.stringify({answer:'continue',reason:i.v+' '+i.harness+' '+i.event}))})",
  ].join("");
  const hook = run(["hook", "claude", "stop", "--", process.execPath, "-e", tool], 0, {
    input: '{"session_id":"s","hook_event_name":"Stop"}',
  });
  const answer = JSON.parse(hook.stdout);
  if (answer.decision !== "block" || answer.reason !== "1 claude stop") {
    fail(`hook answered ${hook.stdout}`);
  }

  // Each schema prints.
  for (const contract of ["manifest", "hook-input", "hook-answer", "result"]) {
    JSON.parse(run(["schema", contract], 0).stdout);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}

console.log(`release-smoke OK: ${version}`);
