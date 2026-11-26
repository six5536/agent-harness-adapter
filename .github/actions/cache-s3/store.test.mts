// Tests how store.mts calls the AWS CLI: the environment and the settings of every call,
// and the upload that the store refuses, which it tries again.

import assert from "node:assert/strict";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { type Env, type Result, restore, save } from "./cache-s3.mts";
import { type Call, scratch, store } from "./test-runner.mts";

test("every call of the AWS CLI gets the store's keys, checksums only where required, and parts of 64 MB", async () => {
  const envs: { operation: string; env: Env }[] = [];
  const run = scratch(store(() => false));
  const answer = run.context.run;
  run.context.run = (command, args, options) => {
    if (command === "aws") {
      envs.push({ operation: `${args[0] ?? ""} ${args[1] ?? ""}`, env: options.env ?? {} });
    }
    return answer(command, args, options);
  };
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  await save(run.context);
  assert.deepEqual([...new Set(envs.map((call) => call.operation))].toSorted(), [
    "s3 cp",
    "s3api head-bucket",
    "s3api head-object",
  ]);
  for (const { operation, env } of envs) {
    assert.equal(env.AWS_ACCESS_KEY_ID, "key-id", operation);
    assert.equal(env.AWS_SECRET_ACCESS_KEY, "secret", operation);
    assert.equal(env.AWS_DEFAULT_REGION, "auto", operation);
    assert.equal(env.AWS_REQUEST_CHECKSUM_CALCULATION, "when_required", operation);
    assert.equal(env.AWS_RESPONSE_CHECKSUM_VALIDATION, "when_required", operation);
    const config = readFileSync(env.AWS_CONFIG_FILE ?? "", "utf8");
    assert.match(config, /multipart_chunksize = 64MB/u, operation);
    assert.match(config, /multipart_threshold = 64MB/u, operation);
  }
});

// A double of the store that refuses the first uploads, as R2 can.
function refuse(times: number): (call: Call) => Partial<Result> {
  let refused = 0;
  return (call: Call) => {
    if (call.args[1] === "cp" && !(call.args[2] ?? "").startsWith("s3://") && refused < times) {
      refused += 1;
      return {
        status: 1,
        stderr:
          "An error occurred (InvalidPart) when calling the CompleteMultipartUpload operation",
      };
    }
    return store(() => false)(call);
  };
}

test("an upload that the store refuses is tried again, and warns only after the last try", async () => {
  for (const [times, last] of [
    [2, "ci-cache-s3: tools saved"],
    [3, "::warning::ci-cache-s3: tools couldn't write"],
  ] as const) {
    const run = scratch(refuse(times));
    const waits: number[] = [];
    run.context.wait = (milliseconds) => {
      waits.push(milliseconds);
      return Promise.resolve();
    };
    await restore("", run.context);
    mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
    await save(run.context);
    assert.deepEqual(waits, [5000, 15_000]);
    assert.equal(run.lines.filter((line) => line.includes("tries again")).length, 2);
    assert.ok(run.lines.at(-1)?.startsWith(last) === true, run.lines.join("\n"));
  }
});

test("a restore that couldn't read the key's ETag or time takes no later save for another job's", async () => {
  let restored = false;
  const run = scratch((call) => {
    if (call.args[1] === "head-object") {
      // The restore's HEAD prints what doesn't parse, and the save's a full answer.
      return restored
        ? { stdout: JSON.stringify({ ETag: '"e"', LastModified: "2026-01-01T00:00:00+00:00" }) }
        : { stdout: "not JSON" };
    }
    return {};
  });
  await restore("", run.context);
  restored = true;
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  await save(run.context);
  assert.ok(!run.lines.some((line) => line.includes("saved by another job")), run.lines.join("\n"));
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: tools saved")));
});
