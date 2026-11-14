// Tests the jobs of ci-matrix-build.mts. Every task name in it is invented.

import assert from "node:assert/strict";
import { test } from "node:test";
import { buildJob } from "./ci-matrix-build.mts";

test("the third part of the task's name is the runner", () => {
  assert.deepEqual(buildJob("build:release:macos:cli"), {
    task: "build:release:macos:cli",
    runner: "macos-latest",
    artifact: "build-release-macos-cli",
  });
});

test("the artifact replaces every colon of the task's name", () => {
  assert.equal(
    buildJob("build:release:windows:cli:arm64").artifact,
    "build-release-windows-cli-arm64",
  );
});

test("the JSON is compact, with the keys task, runner, and artifact in that order", () => {
  assert.equal(
    JSON.stringify(buildJob("build:release:ubuntu:cli")),
    '{"task":"build:release:ubuntu:cli","runner":"ubuntu-latest","artifact":"build-release-ubuntu-cli"}',
  );
});
