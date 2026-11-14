// Tests the choice of run-tests.mts: which tasks a tier runs, under which limit, and
// when the CI tier is skipped.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { tierRun } from "./run-tests.mts";

const script = fileURLToPath(new URL("run-tests.mts", import.meta.url));
// An empty folder as the PATH and as the working directory, so that a run that reaches
// mise fails to start it, as an empty PATH would search the working directory.
const empty = mkdtempSync(join(tmpdir(), "run-tests-"));
after(() => {
  rmSync(empty, { recursive: true, force: true });
});

// Runs the script with only the variables `env`, where no mise can start.
function runScript(tier: string, env: Record<string, string>) {
  return spawnSync(process.execPath, [script, tier], {
    cwd: empty,
    encoding: "utf8",
    env: { PATH: empty, SystemRoot: process.env.SystemRoot ?? "", ...env },
    timeout: 30_000,
  });
}

test("the fast tier runs the fast members under their limit", () => {
  assert.deepEqual(tierRun("fast", {}), {
    task: "test:fast:*",
    limit: { variable: "TEST_FAST_TOTAL_SECONDS", seconds: 15 },
  });
});

test("no tier is the fast tier", () => {
  assert.deepEqual(tierRun(undefined, {}), tierRun("fast", {}));
});

test("the long tier runs the long members under the limit that the variable sets", () => {
  assert.deepEqual(tierRun("long", { TEST_LONG_TOTAL_SECONDS: "90" }), {
    task: "test:long:*",
    limit: { variable: "TEST_LONG_TOTAL_SECONDS", seconds: 90 },
  });
});

test("the CI tier runs its members without a limit where the variable CI is set", () => {
  assert.deepEqual(tierRun("ci", { CI: "true" }), { task: "test:ci:*" });
});

test("the CI tier is skipped with a message where the variable CI is unset", () => {
  const run = tierRun("ci", {});
  assert.ok("skipped" in run);
  assert.match(run.skipped, /skipped the CI tier/u);
  assert.match(run.skipped, /CI is unset, empty, false, or 0/u);
});

test("the CI tier is skipped where the variable CI is empty, false, or 0", () => {
  for (const value of ["", " ", "false", "FALSE", "0"]) {
    assert.ok("skipped" in tierRun("ci", { CI: value }), `CI=${value}`);
  }
});

test("the CI tier runs where the variable CI holds any other value", () => {
  for (const value of ["true", "1", "yes", "woodpecker"]) {
    assert.deepEqual(tierRun("ci", { CI: value }), { task: "test:ci:*" }, `CI=${value}`);
  }
});

test("an empty limit counts as unset", () => {
  assert.deepEqual(tierRun("fast", { TEST_FAST_TOTAL_SECONDS: "" }), tierRun("fast", {}));
});

test("a limit that isn't a number of seconds is an error that names the variable", () => {
  for (const value of [" ", "15s", "-1", "0x10", "1e9"]) {
    assert.throws(
      () => tierRun("long", { TEST_LONG_TOTAL_SECONDS: value }),
      /TEST_LONG_TOTAL_SECONDS must be a number of seconds/u,
      `TEST_LONG_TOTAL_SECONDS=${value}`,
    );
  }
});

test("the script prints the skip of the CI tier and passes without starting mise", () => {
  const result = runScript("ci", {});
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /skipped the CI tier/u);
});

test("the script starts mise for the CI tier where the variable CI is set", () => {
  const result = runScript("ci", { CI: "true" });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /mise run test:ci:\* didn't start/u);
});

test("a decimal limit with blanks around it is a number of seconds", () => {
  assert.deepEqual(tierRun("fast", { TEST_FAST_TOTAL_SECONDS: " 2.5 " }), {
    task: "test:fast:*",
    limit: { variable: "TEST_FAST_TOTAL_SECONDS", seconds: 2.5 },
  });
});

test("the script stops at a bad limit with a message that names the variable", () => {
  const result = runScript("fast", { TEST_FAST_TOTAL_SECONDS: "abc" });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /TEST_FAST_TOTAL_SECONDS must be a number of seconds, not "abc"/u);
});

test("the script stops at an unknown tier with the code of a usage error", () => {
  const result = runScript("smoke", {});
  assert.equal(result.status, 2);
  assert.match(result.stderr, /the tier must be fast, long, or ci, not smoke/u);
});

test("an unknown tier is an error", () => {
  assert.throws(() => tierRun("smoke", {}), /fast, long, or ci, not smoke/u);
});
