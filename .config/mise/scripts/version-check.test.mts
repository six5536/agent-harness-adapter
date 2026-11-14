// Tests the comparison of version-check.mts. Every name in it is invented.

import assert from "node:assert/strict";
import { test } from "node:test";
import { versionProblem } from "./version-check.mts";

function versions(entries: Record<string, string>): Map<string, string> {
  return new Map(Object.entries(entries));
}

test("a given version that no task reports is a problem", () => {
  const problem = versionProblem(versions({}), "1.2.3");
  assert.match(problem ?? "", /no task reports a version/u);
  assert.match(problem ?? "", /1\.2\.3/u);
});

test("no given version and no reported version is no problem", () => {
  assert.equal(versionProblem(versions({})), undefined);
});

test("versions that agree are no problem, with and without a given version", () => {
  const reported = versions({ "Cargo.toml": "1.2.3", "package.json": "1.2.3" });
  assert.equal(versionProblem(reported), undefined);
  assert.equal(versionProblem(reported, "1.2.3"), undefined);
});

test("versions that differ are listed with their files", () => {
  const reported = versions({ "Cargo.toml": "1.2.3", "package.json": "1.2.4" });
  const problem = versionProblem(reported) ?? "";
  assert.match(problem, /the versions differ:/u);
  assert.match(problem, /Cargo\.toml 1\.2\.3/u);
  assert.match(problem, /package\.json 1\.2\.4/u);
});

test("a given version that differs from the reported ones is a problem", () => {
  const problem = versionProblem(versions({ "Cargo.toml": "1.2.3" }), "1.3.0") ?? "";
  assert.match(problem, /differ from 1\.3\.0/u);
});
