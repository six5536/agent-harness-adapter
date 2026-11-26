// Tests how cache-s3.mts restores a cache whose archive is broken, or whose restore fails,
// with a double of the commands that it runs.

import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { restore, save } from "./cache-s3.mts";
import { scratch, store } from "./test-runner.mts";

test("a broken archive that the job leaves none of the paths of is deleted", async () => {
  const run = scratch((call) =>
    call.command === "/usr/bin/tar" && call.args[0] === "-xpf"
      ? { status: 2, stderr: "corrupt" }
      : store(() => true)(call),
  );
  await restore("", run.context);
  run.calls.length = 0;
  await save(run.context);
  assert.ok(
    run.calls.some((call) => call.args[1] === "rm"),
    run.lines.join("\n"),
  );
  assert.ok(
    run.lines.some((line) => line.includes("because the restore couldn't extract it")),
    run.lines.join("\n"),
  );
});

test("a broken archive of the repository's cache leaves the files that git checked out", async () => {
  const run = scratch((call) =>
    call.command === "/usr/bin/tar" && call.args[0] === "-xpf"
      ? { status: 2, stderr: "corrupt" }
      : store((key) => key.includes("/web/"))(call),
  );
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/caches.project.json"),
    JSON.stringify([{ name: "web", paths: ["web/.cache"] }]),
  );
  mkdirSync(join(run.dirs.workspace, "web/.cache"), { recursive: true });
  writeFileSync(join(run.dirs.workspace, "web/.cache/committed.txt"), "kept\n");
  await restore("", run.context);
  assert.equal(
    readFileSync(join(run.dirs.workspace, "web/.cache/committed.txt"), "utf8"),
    "kept\n",
  );
  // The save writes nothing, rather than an archive of part of a tree.
  run.calls.length = 0;
  await save(run.context);
  assert.ok(!run.calls.some((call) => call.args[1] === "cp" && call.args[3]?.startsWith("s3://")));
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: web saves nothing")));
});

test("a broken archive is left where another job saved a good one since the restore", async () => {
  let etag = '"broken"';
  const run = scratch((call) => {
    if (call.command === "/usr/bin/tar" && call.args[0] === "-xpf") {
      return { status: 2, stderr: "corrupt" };
    }
    if (call.args[1] === "head-object") {
      return { stdout: JSON.stringify({ ETag: etag }) };
    }
    return {};
  });
  await restore("", run.context);
  etag = '"good"';
  run.calls.length = 0;
  await save(run.context);
  assert.ok(!run.calls.some((call) => call.args[1] === "rm"), run.lines.join("\n"));
});

test("an error of one cache's restore only warns, and the others restore", async () => {
  const run = scratch(store(() => true));
  const answer = run.context.run;
  run.context.run = (command, args, options) => {
    if (command === "aws" && (args[5] ?? "").includes("/tools/")) {
      throw new Error("the runner lost the AWS CLI");
    }
    return answer(command, args, options);
  };
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  await restore("", run.context);
  assert.ok(
    run.lines.some(
      (line) =>
        line.startsWith("::warning::ci-cache-s3: tools couldn't restore") &&
        line.endsWith("the runner lost the AWS CLI"),
    ),
    run.lines.join("\n"),
  );
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: cargo restored")));
});
