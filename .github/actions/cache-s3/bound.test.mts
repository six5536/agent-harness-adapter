// Tests the bound of the size of ci-cache-s3's archives: the baseline that a save after a
// cold start records, a save within the bound and one past it, and the project's setting,
// and a save that finds the key saved by another job since its restore.

import assert from "node:assert/strict";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { parseSettings } from "./bound.mts";
import { restore, save } from "./cache-s3.mts";
import { memoryStore, scratch, settingsEnv, sizes } from "./test-runner.mts";

const toolsKey = "dev/owner/repo/Linux-X64/tools/v3/current.tar.zst";

// A job that restores the tools, installs a tool, and saves them, with archives of `size`.
async function changedJob(objects: Map<string, string>, size: number) {
  sizes.archive = size;
  const run = scratch(memoryStore(objects));
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools/mise/installs/node/26.13.1"), { recursive: true });
  await save(run.context);
  sizes.archive = 1000;
  return run;
}

test("a save after a cold start records the archive's size as its baseline", async () => {
  const objects = new Map<string, string>();
  const run = await changedJob(objects, 1500);
  assert.equal(objects.get(toolsKey), "1500");
  assert.ok(
    run.lines.includes(`ci-cache-s3: tools saved ${toolsKey}, 1500 bytes, with the baseline 1500`),
    run.lines.join("\n"),
  );
});

test("a save within the bound uploads, and keeps the baseline", async () => {
  const objects = new Map([[toolsKey, "1000"]]);
  const run = await changedJob(objects, 2000);
  assert.equal(objects.get(toolsKey), "1000");
  assert.ok(
    run.lines.includes(`ci-cache-s3: tools saved ${toolsKey}, 2000 bytes, with the baseline 1000`),
  );
});

test("a save past the bound deletes the archive and its baseline, and the next job starts cold", async () => {
  const objects = new Map([[toolsKey, "1000"]]);
  const run = await changedJob(objects, 2001);
  assert.equal(objects.has(toolsKey), false);
  assert.ok(!run.calls.some((call) => call.args[1] === "cp" && !call.args[2]?.startsWith("s3://")));
  assert.ok(
    run.lines.includes(
      `::notice::ci-cache-s3: tools deleted ${toolsKey} and its baseline, because the archive would be 2001 bytes, past its bound of 2000 bytes, 2 times its baseline of 1000 bytes, so the next job builds it cold and sets a new baseline`,
    ),
    run.lines.join("\n"),
  );
  // The next job misses, builds the tools cold, and sets a new baseline.
  const next = await changedJob(objects, 1800);
  assert.ok(next.lines.includes(`ci-cache-s3: tools missed ${toolsKey}`));
  assert.equal(objects.get(toolsKey), "1800");
});

test("a project's bound is a multiple of the baseline or a fixed size", async () => {
  assert.deepEqual(parseSettings("{}"), { bound: { multiple: 2 } });
  assert.deepEqual(parseSettings('{"bound": "3x"}'), { bound: { multiple: 3 } });
  assert.deepEqual(parseSettings('{"bound": "1.5x"}'), { bound: { multiple: 1.5 } });
  assert.deepEqual(parseSettings('{"bound": "4 GiB"}'), { bound: { bytes: 4 * 1024 ** 3 } });
  assert.deepEqual(parseSettings('{"bound": "800MB"}'), { bound: { bytes: 800e6 } });
  // A fixed bound holds a cold save too, which it would delete at once.
  const run = scratch(memoryStore(new Map()));
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/settings.project.json"),
    '{"bound": "999 B"}',
  );
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  await save(run.context);
  assert.ok(
    run.lines.includes(
      "::notice::ci-cache-s3: tools saves no archive, because the archive would be 1000 bytes, past its bound of 999 bytes, the fixed size of 999 bytes",
    ),
    run.lines.join("\n"),
  );
});

test("a malformed setting fails with a message that names the file, even without the store", async () => {
  const cases: [string, RegExp][] = [
    ["{", /not JSON/u],
    ["[]", /not an object of settings/u],
    ['{"limit": "3x"}', /has limit, which the pack doesn't read/u],
    ['{"bound": 3}', /bound is 3, where it must be/u],
    ['{"bound": "1x"}', /bound is "1x"/u],
    ['{"bound": "big"}', /bound is "big"/u],
    ['{"bound": "0 GB"}', /bound is "0 GB"/u],
  ];
  for (const [text, message] of cases) {
    assert.throws(
      () => parseSettings(text),
      (error: Error) =>
        error.message.startsWith(".config/ci-cache-s3/settings.project.json: ") &&
        message.test(error.message),
      text,
    );
  }
  const run = scratch(() => ({}), { ...settingsEnv, CI_CACHE_S3_BUCKET: "" });
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/settings.project.json"),
    '{"bound": "x"}',
  );
  await assert.rejects(restore("", run.context), /settings\.project\.json: bound is "x"/u);
});

test("a save finds the key saved by another job since its restore, and uploads nothing", async () => {
  const objects = new Map([[toolsKey, "1000"]]);
  const etags = new Map<string, string>();
  const run = scratch(memoryStore(objects, etags));
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools/mise/installs/node/26.13.1"), { recursive: true });
  // Another job uploads the key after this one's restore, in the same second.
  etags.set(toolsKey, '"another job"');
  objects.set(toolsKey, "900");
  run.calls.length = 0;
  await save(run.context);
  assert.ok(
    run.lines.includes(
      `ci-cache-s3: tools finds ${toolsKey} saved by another job since the restore, so it stays`,
    ),
    run.lines.join("\n"),
  );
  assert.equal(objects.get(toolsKey), "900");
  assert.ok(!run.calls.some((call) => call.args[1] === "cp" || call.args[1] === "rm"));
});

test("a save after a miss finds the key that another job wrote meanwhile, and uploads nothing", async () => {
  const objects = new Map<string, string>();
  const run = scratch(memoryStore(objects));
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  objects.set(toolsKey, "800");
  await save(run.context);
  assert.ok(run.lines.some((line) => line.includes("saved by another job since the restore")));
  assert.equal(objects.get(toolsKey), "800");
});

test("a broken archive that a save past a fixed bound would replace is deleted", async () => {
  const objects = new Map([[toolsKey, "500"]]);
  const run = scratch((call) =>
    call.command === "/usr/bin/tar" && call.args[0] === "-xpf"
      ? { status: 2, stderr: "broken" }
      : memoryStore(objects)(call),
  );
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/settings.project.json"),
    '{"bound": "999 B"}',
  );
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  await save(run.context);
  assert.equal(objects.has(toolsKey), false);
  assert.ok(
    run.lines.some((line) => line.includes(`tools deleted ${toolsKey}`)),
    run.lines.join("\n"),
  );
});

test("a save uploads where the key holds the archive that its restore found", async () => {
  // The archive that the restore took was written long before, as memoryStore says.
  const objects = new Map([[toolsKey, "1000"]]);
  const run = await changedJob(objects, 1200);
  assert.ok(
    run.lines.includes(`ci-cache-s3: tools saved ${toolsKey}, 1200 bytes, with the baseline 1000`),
  );
});

test("an archive whose baseline the store doesn't give is deleted past a fixed bound", async () => {
  const objects = new Map([[toolsKey, ""]]);
  const run = scratch(memoryStore(objects));
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/settings.project.json"),
    '{"bound": "999 B"}',
  );
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools/new"), { recursive: true });
  await save(run.context);
  assert.equal(objects.has(toolsKey), false);
});

test("a key that another job deletes between the restore's HEAD and its download counts as a miss", async () => {
  const objects = new Map([[toolsKey, "1000"]]);
  const store = memoryStore(objects);
  const run = scratch((call) => {
    if (call.args[1] === "cp" && call.args[2] === `s3://ci-cache/${toolsKey}`) {
      objects.delete(toolsKey);
    }
    return store(call);
  });
  await restore("", run.context);
  assert.ok(run.lines.includes(`ci-cache-s3: tools missed ${toolsKey}`), run.lines.join("\n"));
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  await save(run.context);
  assert.equal(objects.get(toolsKey), "1000");
});
