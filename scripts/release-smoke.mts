// A smoke test of a compiled agent-harness-adapter binary: it runs the binary
// and checks what a user sees, the version line, an install, a status and an
// uninstall from a manifest, the hook bridge, each schema, and the exit code
// of a bad flag. A release build runs it on the binary of the host, and
// `mise run test:smoke` runs it after the launcher's smoke test, which builds
// the host's binary when no release build has staged one.
//
// The script mirrors the tests of crates/app/agent-harness-adapter on purpose:
// those tests check the code, and this script checks the shipped artefact.
// When you extend one, extend the other.
//
// Usage: release-smoke.mts [binary]

import { spawnSync } from "node:child_process";
import type { SpawnSyncOptionsWithStringEncoding, SpawnSyncReturns } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import process from "node:process";
import { targetDir } from "./lib/run.mts";

const exe = process.platform === "win32" ? "agent-harness-adapter.exe" : "agent-harness-adapter";
// A release build stages the binary in the host's platform package, so the
// default is that artefact, and otherwise the local release build.
const staged = join(
  "packages",
  `agent-harness-adapter-${process.platform}-${process.arch}`,
  "bin",
  exe,
);
const bin = resolve(
  process.argv[2] ?? (existsSync(staged) ? staged : join(targetDir, "release", exe)),
);

function fail(message: string): never {
  console.error(`release-smoke: ${message}`);
  process.exit(1);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

// Returns the value at `path` of the parsed JSON `value`, or undefined.
function field(value: unknown, path: (string | number)[]): unknown {
  let current = value;
  for (const key of path) {
    if (Array.isArray(current) && typeof key === "number") {
      current = current[key] as unknown;
    } else if (isRecord(current) && typeof key === "string") {
      current = current[key];
    } else {
      return undefined;
    }
  }
  return current;
}

if (!existsSync(bin)) {
  fail(`no binary at ${bin}: build it first with cargo build --release -p agent-harness-adapter`);
}

// Runs the binary, checks its exit status, and returns the spawn result.
function run(
  args: string[],
  expectStatus: number,
  options: Omit<SpawnSyncOptionsWithStringEncoding, "encoding"> = {},
): SpawnSyncReturns<string> {
  const result = spawnSync(bin, args, { ...options, encoding: "utf8" });
  if (result.error !== undefined) {
    fail(`failed to run ${bin}: ${result.error.message}`);
  }
  if (result.status !== expectStatus) {
    console.error(result.stdout);
    console.error(result.stderr);
    fail(
      `\`agent-harness-adapter ${args.join(" ")}\` exited ${String(result.status)}, expected ${String(expectStatus)}`,
    );
  }
  return result;
}

const version = run(["--version"], 0).stdout.trim();
if (!/^agent-harness-adapter \d+\.\d+\.\d+/u.test(version)) {
  fail(`unexpected --version output: ${version}`);
}
run(["--definitely-not-a-flag"], 2);

const dir = mkdtempSync(join(tmpdir(), "aha-release-smoke-"));
try {
  // install and status from a manifest.
  const manifest = join(dir, "smoke.harness.toml");
  writeFileSync(
    manifest,
    'version = 1\nname = "smoke"\ninstructions = "Use smoke.\\n"\n[[hooks]]\nevent = "stop"\nrun = "smoke decide"\n',
  );
  const project = join(dir, "proj");
  mkdirSync(project);
  const env = { ...process.env, HOME: dir, USERPROFILE: dir };
  const installed = run(
    ["install", "--manifest", manifest, "--harness", "claude", "--root", project],
    0,
    { env },
  );
  if (!installed.stdout.includes("created CLAUDE.md (instructions)")) {
    fail(`install did not report CLAUDE.md:\n${installed.stdout}`);
  }
  const settings = readFileSync(join(project, ".claude", "settings.json"), "utf8");
  if (!settings.includes("agent-harness-adapter hook --tool smoke claude stop")) {
    fail("install did not write the bridged hook");
  }
  const status: unknown = JSON.parse(
    run(["status", "--manifest", manifest, "--root", project, "--json"], 0, { env }).stdout,
  );
  if (field(status, ["harnesses", 0, "parts", 0, "state"]) !== "current") {
    fail(`status after install is not current: ${JSON.stringify(status)}`);
  }

  // uninstall takes it all back out: the project is empty again.
  run(["uninstall", "--manifest", manifest, "--harness", "all", "--root", project], 0, { env });
  if (readdirSync(project).length > 0) {
    fail(`uninstall left ${readdirSync(project).join(", ")}`);
  }

  // The hook bridge, with node as the tool: it reads the contract's input,
  // which is Claude Code's, and asks Codex to continue, in Claude Code's form.
  const tool = [
    "let s='';process.stdin.on('data',d=>s+=d).on('end',()=>{",
    "const i=JSON.parse(s);",
    "console.log(JSON.stringify({decision:'block',reason:i.harness+' '+i.hook_event_name}))})",
  ].join("");
  const hook = run(["hook", "codex", "stop", "--", process.execPath, "-e", tool], 0, {
    input: '{"session_id":"s"}',
  });
  const answer: unknown = JSON.parse(hook.stdout);
  if (field(answer, ["decision"]) !== "block" || field(answer, ["reason"]) !== "codex Stop") {
    fail(`hook answered ${hook.stdout}`);
  }

  // Each schema prints.
  for (const contract of ["manifest", "hooks", "hook-input", "hook-answer", "result"]) {
    JSON.parse(run(["schema", contract], 0).stdout);
  }
} finally {
  rmSync(dir, { recursive: true, force: true });
}

console.log(`release-smoke OK: ${version}`);
