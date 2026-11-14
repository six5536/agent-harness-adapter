// Tests format-shell.mts, with a stand-in for git and shfmt, and with the real tools in
// a scratch git repository.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { formatShell } from "./format-shell.mts";
import type { Outcome } from "./lib/shell-files.mts";
import { inScratch, runScript } from "./lib/shell-scratch.mts";

const script = join(import.meta.dirname, "format-shell.mts");

// A runner that answers git with `listed`, the listing of shfmt -f with each path that
// it gets, and each other run of shfmt with the next status of `formats`, and records
// each call.
function fakeRunner(listed: Outcome, formats: number[] = []) {
  const calls: { command: string; args: string[]; capture: boolean }[] = [];
  const statuses = [...formats];
  const run = (command: string, args: string[], capture: boolean): Outcome => {
    calls.push({ command, args, capture });
    if (command === "git") {
      return listed;
    }
    if (args[0] === "-f") {
      return { status: 0, stdout: args.slice(3).join("\n") };
    }
    return { status: statuses.shift() ?? 0, stdout: "" };
  };
  return { run, calls };
}

const anyFile = () => true;

test("a failure of git is the result, and shfmt doesn't run", () => {
  const { run, calls } = fakeRunner({ status: 128, stdout: "" });
  assert.equal(formatShell(run, "check", undefined, anyFile), 128);
  assert.equal(calls.length, 1);
});

test("shfmt doesn't format when git lists no shell file, because it would read stdin", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "" });
  assert.equal(formatShell(run, "write", undefined, anyFile), 0);
  assert.equal(calls.length, 1);
});

test("write runs shfmt -w and check runs shfmt -d, with the paths after --", () => {
  for (const [mode, flag] of [
    ["write", "-w"],
    ["check", "-d"],
  ] as const) {
    const { run, calls } = fakeRunner({ status: 0, stdout: "a.sh\0b c.sh\0" });
    assert.equal(formatShell(run, mode, undefined, anyFile), 0);
    assert.deepEqual(calls.at(-1), {
      command: "shfmt",
      args: [flag, "--", "a.sh", "b c.sh"],
      capture: false,
    });
  }
});

test("every batch is formatted, and the first failure of shfmt is the result", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "aa\0bb\0cc\0" }, [0, 3, 1]);
  assert.equal(formatShell(run, "check", 2, anyFile), 3);
  assert.deepEqual(
    calls.filter(({ args }) => args[0] === "-d").map(({ args }) => args),
    [
      ["-d", "--", "aa"],
      ["-d", "--", "bb"],
      ["-d", "--", "cc"],
    ],
  );
});

// The arguments of each check of failsOnAa.
const checked: string[][] = [];

// A runner on which the listing's shfmt fails on the batch of "aa" and finds "bb", and
// which records each check.
function failsOnAa(command: string, args: string[]): Outcome {
  if (command === "git") {
    return { status: 0, stdout: "aa\0bb\0" };
  }
  if (args[0] === "-f") {
    return args.includes("aa") ? { status: 1, stdout: "" } : { status: 0, stdout: "bb" };
  }
  checked.push(args);
  return { status: 0, stdout: "" };
}

test("a failure of the listing's shfmt is the result, and the scripts that it found are formatted", () => {
  assert.equal(formatShell(failsOnAa, "check", 2, anyFile), 1);
  assert.deepEqual(checked, [["-d", "--", "bb"]]);
});

// A script that shfmt formats differently, with a space before the parentheses and
// without an indent.
const unformatted = "#!/bin/sh\nf () {\necho hi\n}\n";
const formatted = "#!/bin/sh\nf() {\n\techo hi\n}\n";

test("check passes over an ignored script that shfmt would change", () => {
  const result = inScratch(
    {
      ".gitignore": "/consumers/\n",
      "consumers/other/build.sh": unformatted,
      "clean.sh": formatted,
    },
    (dir, env) => runScript(script, ["check"], dir, env),
  );
  assert.equal(result.status, 0, result.stdout + result.stderr);
});

test("check fails on a script that git would add whose name starts with a dash, and names it", () => {
  const result = inScratch({ "-n.sh": unformatted }, (dir, env) =>
    runScript(script, ["check"], dir, env),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stdout, /-n\.sh/u);
});

test("write formats the scripts that git would add and leaves the ignored ones", () => {
  inScratch(
    {
      ".gitignore": "/consumers/\n",
      "consumers/build.sh": unformatted,
      "dir with space/run.sh": unformatted,
    },
    (dir, env) => {
      const result = runScript(script, ["write"], dir, env);
      assert.equal(result.status, 0, result.stdout + result.stderr);
      assert.equal(readFileSync(join(dir, "dir with space/run.sh"), "utf8"), formatted);
      assert.equal(readFileSync(join(dir, "consumers/build.sh"), "utf8"), unformatted);
    },
  );
});

test("outside a git repository the script fails, and says that it needs one", () => {
  const result = inScratch(
    { "clean.sh": formatted },
    (dir, env) => runScript(script, ["check"], dir, env),
    false,
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /only in a git repository/u);
});

test("a mode other than write or check is a usage error", () => {
  const result = inScratch({}, (dir, env) => runScript(script, ["fix"], dir, env));
  assert.equal(result.status, 2);
  assert.match(result.stderr, /Usage: format-shell\.mts write\|check/u);
});
