// Tests the choice of pack-tests.mts, whether a pack's tests run by pakf's report and
// PACK_TESTS, and its runs with a double of pakf on the PATH.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { isCode, packTests, parseReport } from "./pack-tests.mts";
import type { PackFile, PackReport } from "./pack-tests.mts";

const script = fileURLToPath(new URL("pack-tests.mts", import.meta.url));
const core = "@six5536/project-core";
const ts = "@six5536/typescript";
const task = { MISE_TASK_NAME: "test:fast:core" };
const skipped = `test:fast:core: skipped the tests of ${core}, because no file of its code differs from the pack; PACK_TESTS=${core} runs them.`;

function report(...files: PackFile[]): PackReport[] {
  return [
    { name: core, files },
    { name: ts, files: [{ path: "/.config/vitest/vitest.base.mts", state: "edited" }] },
  ];
}

test("an unedited pack skips its tests and says why, and how to run them", () => {
  assert.deepEqual(packTests(core, report(), task), { run: false, skipped });
});

test("an edited, conflicted, or deleted file of the pack's code runs its tests", () => {
  for (const state of ["edited", "conflicted", "deleted"]) {
    const files = report({ path: "/.config/mise/scripts/run-tests.mts", state });
    assert.deepEqual(packTests(core, files, task), { run: true }, state);
  }
});

test("an accepted edit runs the tests, and an accepted file that holds its base doesn't", () => {
  assert.deepEqual(packTests(core, report({ path: "/.gitignore", state: "edited" }), task), {
    run: true,
  });
  assert.deepEqual(packTests(core, report({ path: "/.gitignore", state: "unchanged" }), task), {
    run: false,
    skipped,
  });
});

test("an edited editable file runs no test, and the skip names it", () => {
  const files = report(
    { path: "/Cargo.toml", state: "edited", kind: "editable" },
    { path: "/LICENSE", state: "edited", kind: "editable" },
  );
  assert.deepEqual(packTests(core, files, task), {
    run: false,
    skipped: `test:fast:core: skipped the tests of ${core}, because no file of its code differs from the pack other than the editable /Cargo.toml, /LICENSE; PACK_TESTS=${core} runs them.`,
  });
});

test("a conflicted or deleted editable file runs the tests", () => {
  for (const state of ["conflicted", "deleted"]) {
    const files = report({ path: "/Cargo.toml", state, kind: "editable" });
    assert.deepEqual(packTests(core, files, task), { run: true }, state);
  }
});

test("an edited editable file beside another edited file of the code runs the tests", () => {
  const files = report(
    { path: "/Cargo.toml", state: "edited", kind: "editable" },
    { path: "/.gitignore", state: "edited", kind: "file" },
  );
  assert.deepEqual(packTests(core, files, task), { run: true });
});

test("an edited file without a kind, as pakf 0.1.1 reports it, runs the tests", () => {
  assert.deepEqual(packTests(core, report({ path: "/Cargo.toml", state: "edited" }), task), {
    run: true,
  });
});

test("an edit of a document, under /knowledge/ or in Markdown, runs no test", () => {
  assert.equal(isCode("/knowledge/recipes/recipe-faster-ci.md"), false);
  assert.equal(isCode("/knowledge/schemas/issue.yaml"), false);
  assert.equal(isCode("/CONTRIBUTING.md"), false);
  assert.equal(isCode("/.config/mise/scripts/run-tests.mts"), true);
  assert.equal(isCode("/knowledge.toml"), true);
  const files = report(
    { path: "/knowledge/schemas/issue.yaml", state: "edited" },
    { path: "/CONTRIBUTING.md", state: "edited" },
  );
  assert.deepEqual(packTests(core, files, task), { run: false, skipped });
});

test("another pack's edit runs none of this pack's tests", () => {
  assert.equal(packTests(core, report(), task).run, false);
  assert.equal(packTests(ts, report(), { MISE_TASK_NAME: "test:fast:ts-scripts" }).run, true);
});

test("PACK_TESTS=all runs every pack's tests, and a list runs only the packs that it names", () => {
  assert.deepEqual(packTests(core, report(), { ...task, PACK_TESTS: "all" }), { run: true });
  const list = { ...task, PACK_TESTS: ` ${ts} , ${core} ` };
  assert.deepEqual(packTests(core, report(), list), { run: true });
  assert.deepEqual(packTests(core, report(), { ...task, PACK_TESTS: ts }), {
    run: false,
    skipped,
  });
});

test("an empty PACK_TESTS counts as unset", () => {
  for (const value of ["", " ", ","]) {
    assert.deepEqual(packTests(core, report(), { ...task, PACK_TESTS: value }), {
      run: false,
      skipped,
    });
  }
});

test("an id of PACK_TESTS that pakf doesn't report fails with a message that names it", () => {
  assert.throws(
    () => packTests(core, report(), { ...task, PACK_TESTS: `${core},@six5536/nope` }),
    /PACK_TESTS names @six5536\/nope, which pakf status doesn't report/u,
  );
  assert.throws(
    () => packTests(core, [], { ...task, PACK_TESTS: core }),
    /PACK_TESTS names @six5536\/project-core/u,
  );
});

test("a pack that pakf records no base for runs its tests and says so", () => {
  assert.deepEqual(packTests(core, [], task), {
    run: true,
    note: `test:fast:core: runs the tests of ${core}, because pakf records no base for the pack.`,
  });
  assert.deepEqual(packTests(core, [], { PACK_TESTS: "all" }), { run: true });
});

test("a run outside a mise task names the script", () => {
  const decision = packTests(core, report(), {});
  assert.ok(
    !decision.run && decision.skipped.startsWith(`pack-tests: skipped the tests of ${core},`),
  );
});

test("the report keeps each pack's name and its files' paths and states", () => {
  const text = JSON.stringify([
    {
      name: core,
      version: "0.3.0",
      source: "./",
      ref: "main",
      files: [{ path: "/Cargo.toml", state: "edited", acceptance: "accepted", note: "ours" }],
      shadowed: [{ path: "/LICENSE", owner: "@six5536/license-mit" }],
    },
  ]);
  assert.deepEqual(parseReport(text), [
    { name: core, files: [{ path: "/Cargo.toml", state: "edited" }] },
  ]);
});

test("the report keeps each file's kind where pakf gives one", () => {
  const text = JSON.stringify([
    {
      name: core,
      files: [
        { path: "/Cargo.toml", kind: "editable", state: "edited", acceptance: null, note: null },
        { path: "/.gitignore", kind: "file", state: "deleted", acceptance: null, note: null },
      ],
      shadowed: [],
    },
  ]);
  assert.deepEqual(parseReport(text), [
    {
      name: core,
      files: [
        { path: "/Cargo.toml", state: "edited", kind: "editable" },
        { path: "/.gitignore", state: "deleted", kind: "file" },
      ],
    },
  ]);
});

test("a report that isn't pakf's is refused", () => {
  assert.throws(() => parseReport("not json"), SyntaxError);
  assert.throws(() => parseReport("{}"), /isn't a list of packs/u);
  assert.throws(() => parseReport('[{ "name": "x" }]'), /no name or no list of files/u);
  assert.throws(
    () => parseReport('[{ "name": "x", "files": [{ "path": "/a" }] }]'),
    /a file of x/u,
  );
  assert.throws(
    () =>
      parseReport('[{ "name": "x", "files": [{ "path": "/a", "state": "edited", "kind": 1 }] }]'),
    /the file \/a of x has a kind that isn't a string/u,
  );
});

// A folder with a double of pakf, which prints PAKF_OUTPUT and PAKF_ERROR and exits with
// PAKF_EXIT, and the test files that a run passes to `node --test`. On Windows, the
// script runs pakf through a shell, which finds the double's .cmd file.
const scratch = mkdtempSync(join(tmpdir(), "pack-tests-"));
after(() => {
  rmSync(scratch, { recursive: true, force: true });
});
const double = join(scratch, "pakf.mjs");
writeFileSync(
  double,
  [
    'import process from "node:process";',
    'process.stdout.write(process.env.PAKF_OUTPUT ?? "[]");',
    'process.stderr.write(process.env.PAKF_ERROR ?? "");',
    'process.exitCode = Number(process.env.PAKF_EXIT ?? "0");',
    "",
  ].join("\n"),
);
if (process.platform === "win32") {
  writeFileSync(join(scratch, "pakf.cmd"), `@"${process.execPath}" "${double}" %*\r\n`);
} else {
  writeFileSync(join(scratch, "pakf"), `#!/bin/sh\nexec "${process.execPath}" "${double}" "$@"\n`);
  chmodSync(join(scratch, "pakf"), 0o755);
}
writeFileSync(
  join(scratch, "pass.test.mjs"),
  'import { test } from "node:test";\ntest("passes", () => {});\n',
);
writeFileSync(
  join(scratch, "fail.test.mjs"),
  'import { test } from "node:test";\ntest("fails", () => { throw new Error("ran"); });\n',
);

// Runs the script in the scratch folder with only the variables `env`, and the double
// of pakf as the only program on the PATH.
function runScript(args: string[], env: Record<string, string>) {
  return spawnSync(process.execPath, [script, ...args], {
    cwd: scratch,
    encoding: "utf8",
    env: {
      PATH: scratch,
      SystemRoot: process.env.SystemRoot ?? "",
      ComSpec: process.env.ComSpec ?? "",
      PATHEXT: process.env.PATHEXT ?? "",
      MISE_TASK_NAME: "test:fast:core",
      ...env,
    },
    timeout: 60_000,
  });
}

const edited = JSON.stringify([
  { name: core, files: [{ path: "/.config/mise/scripts/run-tests.mts", state: "edited" }] },
]);
const unedited = JSON.stringify([{ name: core, files: [] }]);

test("the script runs node --test on its arguments while the pack is edited", () => {
  const pass = runScript([core, "pass.test.mjs"], { PAKF_OUTPUT: edited });
  assert.equal(pass.status, 0, pass.stderr);
  assert.match(pass.stdout, /\bpass 1\b/u);
  const fail = runScript([core, "fail.test.mjs"], { PAKF_OUTPUT: edited });
  assert.notEqual(fail.status, 0);
  assert.match(fail.stdout, /\bfail 1\b/u);
});

test("the script prints the skip and runs no test while the pack is unedited", () => {
  const result = runScript([core, "fail.test.mjs"], { PAKF_OUTPUT: unedited });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, `${skipped}\n`);
});

test("the script prints the skip with the editable file and runs no test", () => {
  const output = JSON.stringify([
    { name: core, files: [{ path: "/Cargo.toml", kind: "editable", state: "edited" }] },
  ]);
  const result = runScript([core, "fail.test.mjs"], { PAKF_OUTPUT: output });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    result.stdout,
    `test:fast:core: skipped the tests of ${core}, because no file of its code differs from the pack other than the editable /Cargo.toml; PACK_TESTS=${core} runs them.\n`,
  );
});

test("the script runs the tests of a pack that pakf records no base for, with the note", () => {
  const result = runScript([core, "pass.test.mjs"], { PAKF_OUTPUT: "[]" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(
    result.stdout,
    /^test:fast:core: runs the tests of @six5536\/project-core, because pakf records no base for the pack\.\n/u,
  );
  assert.match(result.stdout, /\bpass 1\b/u);
});

test("a failure of pakf fails the task with a message that names the pack and the cause", () => {
  const result = runScript([core, "pass.test.mjs"], {
    PAKF_OUTPUT: "",
    PAKF_ERROR: "error: the lock /.pakf/lock.toml doesn't parse",
    PAKF_EXIT: "2",
  });
  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /pakf failed to report the state of the files of @six5536\/project-core: error: the lock \/\.pakf\/lock\.toml doesn't parse/u,
  );
  const broken = runScript([core, "pass.test.mjs"], { PAKF_OUTPUT: "{" });
  assert.equal(broken.status, 1);
  assert.match(
    broken.stderr,
    /pakf failed to report the state of the files of @six5536\/project-core: /u,
  );
});

test("PACK_TESTS=all runs the tests without a report of pakf", () => {
  const result = runScript([core, "pass.test.mjs"], {
    PAKF_OUTPUT: "",
    PAKF_ERROR: "error: not in a Git repository",
    PAKF_EXIT: "2",
    PACK_TESTS: "all",
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /\bpass 1\b/u);
});

test("an id of PACK_TESTS that pakf doesn't report fails the script", () => {
  const result = runScript([core, "pass.test.mjs"], {
    PAKF_OUTPUT: unedited,
    PACK_TESTS: "@six5536/nope",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /PACK_TESTS names @six5536\/nope/u);
});

test("the script without a pack or a test file prints its usage", () => {
  for (const args of [[], [core]]) {
    const result = runScript(args, { PAKF_OUTPUT: edited });
    assert.equal(result.status, 2);
    assert.match(result.stderr, /usage: pack-tests\.mts <pack id>/u);
  }
});
