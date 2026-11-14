// Tests lint-shell.mts, with a stand-in for git, shfmt, and shellcheck, and with the
// real tools in a scratch git repository.

import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { lintShell } from "./lint-shell.mts";
import type { Outcome } from "./lib/shell-files.mts";
import { inScratch, runScript } from "./lib/shell-scratch.mts";

const script = join(import.meta.dirname, "lint-shell.mts");

// A runner that answers git with `listed`, shfmt with each path that it gets, and each
// run of shellcheck with the next status of `checks`, and records each call.
function fakeRunner(listed: Outcome, checks: number[] = []) {
  const calls: { command: string; args: string[]; capture: boolean }[] = [];
  const statuses = [...checks];
  const run = (command: string, args: string[], capture: boolean): Outcome => {
    calls.push({ command, args, capture });
    if (command === "git") {
      return listed;
    }
    if (command === "shfmt") {
      return { status: 0, stdout: args.slice(3).join("\n") };
    }
    return { status: statuses.shift() ?? 0, stdout: "" };
  };
  return { run, calls };
}

const anyFile = () => true;

test("a failure of the listing is the result, and shellcheck doesn't run", () => {
  const { run, calls } = fakeRunner({ status: 128, stdout: "" });
  assert.equal(lintShell(run, undefined, anyFile), 128);
  assert.equal(calls.length, 1);
});

test("shellcheck doesn't run when git lists no script", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "" });
  assert.equal(lintShell(run, undefined, anyFile), 0);
  assert.equal(calls.length, 1);
});

test("shellcheck gets each listed path as one argument", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "a.sh\0b c.sh\0" });
  assert.equal(lintShell(run, undefined, anyFile), 0);
  assert.deepEqual(calls.at(-1), {
    command: "shellcheck",
    args: ["--", "a.sh", "b c.sh"],
    capture: false,
  });
});

test("every batch runs, and the first failure of shellcheck is the result", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "aa\0bb\0cc\0" }, [0, 3, 1]);
  assert.equal(lintShell(run, 2, anyFile), 3);
  assert.deepEqual(
    calls.filter(({ command }) => command === "shellcheck").map(({ args }) => args),
    [
      ["--", "aa"],
      ["--", "bb"],
      ["--", "cc"],
    ],
  );
});

function lintIn(files: Record<string, string>) {
  return inScratch(files, (dir, env) => runScript(script, [], dir, env));
}

test("a project without shell scripts passes", () => {
  const result = lintIn({ "notes.txt": "no script here\n" });
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("clean scripts whose names hold a space or start with a dash pass", () => {
  const result = lintIn({
    "clean script.sh": '#!/bin/sh\necho "hello"\n',
    "-n.sh": '#!/bin/sh\necho "hello"\n',
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("a finding of shellcheck in a script whose name holds a space fails, and names it", () => {
  const result = lintIn({ "bad script.sh": "#!/bin/sh\necho $1\n" });
  assert.notEqual(result.status, 0);
  assert.match(result.stdout, /bad script\.sh/u);
  assert.match(result.stdout, /SC2086/u);
});

test("a script with a finding that git ignores passes", () => {
  const result = lintIn({
    ".gitignore": "node_modules/\n",
    "docs/node_modules/lunr/build/release.sh": "#!/bin/sh\necho $1\n",
  });
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("a finding of shellcheck in a script whose name starts with a dash fails, and names it", () => {
  const result = lintIn({ "-n.sh": "#!/bin/sh\necho $1\n" });
  assert.notEqual(result.status, 0);
  assert.match(result.stdout, /-n\.sh/u);
  assert.match(result.stdout, /SC2086/u);
});

test("outside a git repository the lint fails, and says that it needs one", () => {
  const result = inScratch(
    { "bad.sh": "#!/bin/sh\necho $1\n" },
    (dir, env) => runScript(script, [], dir, env),
    false,
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /only in a git repository/u);
});
