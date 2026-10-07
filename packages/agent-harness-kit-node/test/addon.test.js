"use strict";

const { test } = require("node:test");
const assert = require("node:assert");
const { selectPackage, loadAddon, isGlibc } = require("../addon");

// @zen-test: BND-4_AC-2
test("each supported host has its platform package", () => {
  assert.strictEqual(selectPackage("linux", "x64"), "@six5536/agent-harness-kit-node-linux-x64-gnu");
  assert.strictEqual(selectPackage("darwin", "arm64"), "@six5536/agent-harness-kit-node-darwin-arm64");
  assert.strictEqual(selectPackage("win32", "x64"), "@six5536/agent-harness-kit-node-win32-x64");
  assert.strictEqual(selectPackage("linux", "x64", false), null);
  assert.strictEqual(selectPackage("win32", "arm64"), null);
});

test("glibc is read from the process report", () => {
  assert.strictEqual(isGlibc(() => ({ header: { glibcVersionRuntime: "2.39" } })), true);
  assert.strictEqual(isGlibc(() => ({ header: {} })), false);
  assert.strictEqual(
    isGlibc(() => {
      throw new Error("no report");
    }),
    false,
  );
});

test("the loader takes AHK_NODE_ADDON, else the platform package", () => {
  const seen = [];
  const load = (p) => {
    seen.push(p);
    return { ok: p };
  };
  assert.deepStrictEqual(loadAddon({ env: { AHK_NODE_ADDON: "/x.node" }, load }), { ok: "/x.node" });
  loadAddon({ platform: "darwin", arch: "x64", env: {}, load });
  assert.strictEqual(seen[1], "@six5536/agent-harness-kit-node-darwin-x64/index.node");
});

test("the loader names the fix when there is no addon", () => {
  assert.throws(
    () => loadAddon({ platform: "linux", arch: "x64", glibc: false, env: {} }),
    /No prebuilt agent-harness-kit addon for linux-x64 \(musl\)/,
  );
  assert.throws(
    () =>
      loadAddon({
        platform: "win32",
        arch: "x64",
        env: {},
        load: () => {
          throw new Error("Cannot find module");
        },
      }),
    /"@six5536\/agent-harness-kit-node-win32-x64" could not be loaded \(Cannot find module\)/,
  );
});
