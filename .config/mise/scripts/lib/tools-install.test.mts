// Tests tools-install.mts.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { checkInstall, writeStamp } from "./tools-install.mts";

const manifest = '{ "devDependencies": { "alpha": "1.0.0" } }\n';
const lock = '{ "lockfileVersion": 3 }\n';

// Writes `files` into a new temporary directory with an empty node_modules,
// runs `check` on it, and removes the directory.
function inDir(files: Record<string, string>, check: (dir: string) => void): void {
  const dir = mkdtempSync(join(tmpdir(), "tools-install-"));
  try {
    mkdirSync(join(dir, "node_modules"));
    for (const [name, content] of Object.entries(files)) {
      writeFileSync(join(dir, name), content);
    }
    check(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

test("a folder without package.json has nothing to check", () => {
  inDir({}, (dir) => {
    assert.equal(checkInstall(dir), undefined);
  });
});

test("an install without a stamp isn't installed", () => {
  inDir({ "package.json": manifest, "package-lock.json": lock }, (dir) => {
    assert.match(checkInstall(dir) ?? "", /aren't installed/u);
  });
});

test("an install stamped from the current files is no problem", () => {
  inDir({ "package.json": manifest, "package-lock.json": lock }, (dir) => {
    writeStamp(dir);
    assert.equal(checkInstall(dir), undefined);
  });
  inDir({ "package.json": manifest }, (dir) => {
    writeStamp(dir);
    assert.equal(checkInstall(dir), undefined);
  });
});

test("a change to package.json or the lock file after the install is a problem", () => {
  for (const [name, content] of [
    ["package.json", '{ "devDependencies": { "alpha": "2.0.0" } }\n'],
    ["package-lock.json", '{ "lockfileVersion": 3, "packages": {} }\n'],
  ]) {
    inDir({ "package.json": manifest, "package-lock.json": lock }, (dir) => {
      writeStamp(dir);
      writeFileSync(join(dir, name), content);
      assert.match(checkInstall(dir) ?? "", /out of date/u);
    });
  }
});

test("a lock file that appears after the install is a problem", () => {
  inDir({ "package.json": manifest }, (dir) => {
    writeStamp(dir);
    writeFileSync(join(dir, "package-lock.json"), lock);
    assert.match(checkInstall(dir) ?? "", /out of date/u);
  });
});
