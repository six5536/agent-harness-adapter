"use strict";

const assert = require("node:assert");
const { test } = require("node:test");
const {
  packagesFrom,
  commandName,
  selectPackage,
  binaryName,
  resolveBinary,
  exitCode,
} = require("../lib/binary");

const optional = {
  "@example/tool-linux-x64": "1.0.0",
  "@example/tool-linux-arm64": "1.0.0",
  "@example/tool-darwin-x64": "1.0.0",
  "@example/tool-darwin-arm64": "1.0.0",
  "@example/tool-win32-x64": "1.0.0",
  "unrelated-package": "1.0.0",
};
const packages = packagesFrom(optional);

const fakeResolve = (spec) => `/modules/${spec}`;
const missingResolve = () => {
  throw new Error("Cannot find module");
};

test("each of the five platforms selects its package", () => {
  for (const [platform, arch] of [
    ["linux", "x64"],
    ["linux", "arm64"],
    ["darwin", "x64"],
    ["darwin", "arm64"],
    ["win32", "x64"],
  ]) {
    assert.strictEqual(
      selectPackage(platform, arch, packages),
      `@example/tool-${platform}-${arch}`,
    );
  }
});

test("a package that names no platform is ignored", () => {
  assert.strictEqual(Object.keys(packages).length, 5);
});

test("an unsupported platform selects nothing", () => {
  assert.strictEqual(selectPackage("win32", "arm64", packages), null);
});

test("the command name is the first key of bin", () => {
  assert.strictEqual(commandName({ tool: "bin/cli.js" }), "tool");
});

test("the binary has .exe only on Windows", () => {
  assert.strictEqual(binaryName("linux", "tool"), "tool");
  assert.strictEqual(binaryName("win32", "tool"), "tool.exe");
});

test("resolveBinary returns the path inside the platform package", () => {
  assert.strictEqual(
    resolveBinary("win32", "x64", fakeResolve, packages, "tool"),
    "/modules/@example/tool-win32-x64/bin/tool.exe",
  );
});

test("an unsupported platform gives an error that names it", () => {
  assert.throws(
    () => resolveBinary("win32", "arm64", () => "unused", packages, "tool"),
    /no prebuilt tool binary for win32-arm64/u,
  );
});

test("a missing platform package gives an error that names it", () => {
  assert.throws(
    () => resolveBinary("linux", "x64", missingResolve, packages, "tool"),
    /platform package "@example\/tool-linux-x64" is not installed/u,
  );
});

test("each error offers an install from source", () => {
  assert.throws(
    () => resolveBinary("win32", "arm64", () => "unused", packages, "tool"),
    /Install from source instead: cargo install tool$/u,
  );
  assert.throws(
    () => resolveBinary("linux", "x64", missingResolve, packages, "tool"),
    /build from source: cargo install tool$/u,
  );
});

test("exitCode forwards a normal exit status", () => {
  assert.strictEqual(exitCode({ status: 2, signal: null }), 2);
});

test("a process stopped by a signal exits with 128 plus its number", () => {
  assert.strictEqual(exitCode({ status: null, signal: "SIGINT" }, { SIGINT: 2 }), 130);
});

test("an unknown signal or a null status exits with 1", () => {
  assert.strictEqual(exitCode({ status: null, signal: "SIGNOPE" }, {}), 1);
});
