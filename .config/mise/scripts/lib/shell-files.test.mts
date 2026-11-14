// Tests shell-files.mts, with a stand-in for git and shfmt, and with the real tools in
// a scratch git repository.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import {
  batches,
  isRegularFile,
  lines,
  listShellFiles,
  nulFields,
  spawnRunner,
} from "./shell-files.mts";
import type { Outcome } from "./shell-files.mts";
import { inScratch } from "./shell-scratch.mts";

// A runner that answers git with `listed` and shfmt with `found`, or with each path
// that it gets, and records each call.
function fakeRunner(listed: Outcome, found?: Outcome) {
  const calls: { command: string; args: string[]; capture: boolean }[] = [];
  const run = (command: string, args: string[], capture: boolean): Outcome => {
    calls.push({ command, args, capture });
    if (command === "git") {
      return listed;
    }
    return found ?? { status: 0, stdout: args.slice(3).join("\n") };
  };
  return { run, calls };
}

const anyFile = () => true;

test("lines splits on lines, keeps spaces, and drops empty lines", () => {
  assert.deepEqual(lines("a.sh\ndir with space/b c.sh\r\n\n"), ["a.sh", "dir with space/b c.sh"]);
  assert.deepEqual(lines(""), []);
});

test("nulFields splits on NUL, keeps newlines and spaces, and drops the last empty field", () => {
  assert.deepEqual(nulFields("a.sh\0b\nc d.sh\0"), ["a.sh", "b\nc d.sh"]);
  assert.deepEqual(nulFields(""), []);
});

test("batches keeps paths together up to the limit, counting the spaces between them", () => {
  assert.deepEqual(batches(["aa", "bb", "cc"], 5), [["aa", "bb"], ["cc"]]);
  assert.deepEqual(batches(["aa", "bb"], 4), [["aa"], ["bb"]]);
  assert.deepEqual(batches(["a-long-path", "b"], 3), [["a-long-path"], ["b"]]);
  assert.deepEqual(batches([]), []);
});

test("git lists the files that it tracks or would add, and shfmt keeps the shell scripts", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "a.sh\0-n.sh\0" });
  assert.deepEqual(listShellFiles(run, undefined, anyFile), {
    status: 0,
    files: ["a.sh", "-n.sh"],
  });
  assert.deepEqual(calls, [
    {
      command: "git",
      args: ["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
      capture: true,
    },
    { command: "shfmt", args: ["-f", "--apply-ignore", "--", "a.sh", "-n.sh"], capture: true },
  ]);
});

test("a path that the index lists more than once goes to shfmt once", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "a.sh\0a.sh\0a.sh\0" });
  assert.deepEqual(listShellFiles(run, undefined, anyFile).files, ["a.sh"]);
  assert.deepEqual(calls.at(-1)?.args, ["-f", "--apply-ignore", "--", "a.sh"]);
});

test("a path that isn't a regular file stays out, and shfmt doesn't run without a file", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "gone.sh\0" });
  assert.deepEqual(
    listShellFiles(run, undefined, () => false),
    { status: 0, files: [] },
  );
  assert.equal(calls.length, 1);
});

test("a path whose name holds a line break or isn't UTF-8 stays out, with a message", () => {
  const { run, calls } = fakeRunner({ status: 0, stdout: "x\ny.sh\0z\r.sh\0\uFFFD.sh\0a.sh\0" });
  assert.deepEqual(listShellFiles(run, undefined, anyFile).files, ["a.sh"]);
  assert.deepEqual(calls.at(-1)?.args, ["-f", "--apply-ignore", "--", "a.sh"]);
});

test("a failure of git is the result, and shfmt doesn't run", () => {
  const { run, calls } = fakeRunner({ status: 128, stdout: "" });
  assert.deepEqual(listShellFiles(run, undefined, anyFile), { status: 128, files: [] });
  assert.equal(calls.length, 1);
});

test("shfmt gets the paths in batches, and a failure of shfmt is the result", () => {
  const listed = { status: 0, stdout: "aa\0bb\0" };
  const { run, calls } = fakeRunner(listed);
  assert.deepEqual(listShellFiles(run, 2, anyFile).files, ["aa", "bb"]);
  assert.equal(calls.length, 3);
});

// A runner on which shfmt fails on the batch of "aa", as on a file that it can't read,
// and finds each other path that it gets.
function failsOnAa(command: string, args: string[]): Outcome {
  if (command === "git") {
    return { status: 0, stdout: "aa\0bb\0cc\0" };
  }
  const paths = args.slice(3);
  return paths.includes("aa") ? { status: 1, stdout: "" } : { status: 0, stdout: paths.join("\n") };
}

test("a batch on which shfmt fails is the result, and the scripts of the other batches stay", () => {
  assert.deepEqual(listShellFiles(failsOnAa, 2, anyFile), { status: 1, files: ["bb", "cc"] });
});

test("a command that doesn't start has the status 1", () => {
  const outcome = spawnRunner("shell-files-no-such-command", [], true);
  assert.deepEqual(outcome, { status: 1, stdout: "" });
});

test("isRegularFile is false for a folder, a symbolic link, and a missing path", () => {
  inScratch({ "a.sh": "#!/bin/sh\n" }, (dir) => {
    mkdirSync(join(dir, "folder"));
    assert.equal(isRegularFile(join(dir, "a.sh")), true);
    assert.equal(isRegularFile(join(dir, "folder")), false);
    assert.equal(isRegularFile(join(dir, "missing.sh")), false);
    if (process.platform !== "win32") {
      symlinkSync("a.sh", join(dir, "link.sh"));
      assert.equal(isRegularFile(join(dir, "link.sh")), false);
    }
  });
});

// The listing of the shell scripts with the real git and shfmt in `dir`, with its
// files sorted, from a Node.js of its own, whose folder and environment are those of
// the scratch repository.
function listIn(dir: string, env: NodeJS.ProcessEnv): unknown {
  const module = JSON.stringify(pathToFileURL(join(import.meta.dirname, "shell-files.mts")).href);
  const program = `import { listShellFiles, spawnRunner } from ${module};
const { status, files } = listShellFiles(spawnRunner);
console.log(JSON.stringify({ status, files: files.toSorted() }));`;
  const result = spawnSync(process.execPath, ["--input-type=module", "-e", program], {
    cwd: dir,
    env,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("the real tools list the tracked and new scripts, and leave out the ignored ones", () => {
  inScratch(
    {
      ".gitignore": "/consumers/\nnode_modules/\n",
      ".editorconfig": "root = true\n\n[skipped.sh]\nignore = true\n",
      "consumers/other/build.sh": "#!/bin/sh\n",
      "docs/node_modules/lunr/build/release.sh": "#!/bin/sh\n",
      "run.sh": "echo\n",
      "bin/tool": "#!/usr/bin/env bash\n",
      "notes.txt": "no script\n",
      "skipped.sh": "#!/bin/sh\n",
      "deleted.sh": "#!/bin/sh\n",
    },
    (dir, env) => {
      const git = (args: string[]) => {
        const result = spawnSync("git", args, { cwd: dir, env, encoding: "utf8" });
        assert.equal(result.status, 0, result.stderr);
      };
      git(["add", "run.sh", "deleted.sh"]);
      rmSync(join(dir, "deleted.sh"));
      assert.deepEqual(listIn(dir, env), { status: 0, files: ["bin/tool", "run.sh"] });
    },
  );
});
