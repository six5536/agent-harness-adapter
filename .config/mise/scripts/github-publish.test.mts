// Tests the assets of github-publish.mts: node --test on this file.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { test } from "node:test";
import { copyAssets, releaseAssets } from "./github-publish.mts";

function folder(files: Record<string, string>): string {
  const root = mkdtempSync(join(tmpdir(), "github-publish-"));
  for (const [name, text] of Object.entries(files)) {
    writeFileSync(join(root, name), text);
  }
  return root;
}

test("each file of the assets joins the release, a link to a file too, without a hidden one", () => {
  const root = folder({ "schema.json": "{}", "b.txt": "b", ".gitkeep": "" });
  try {
    // Windows makes a link only with a privilege that a runner may lack.
    const linked = process.platform !== "win32";
    if (linked) {
      symlinkSync(join(root, "b.txt"), join(root, "c.txt"));
    }
    assert.deepEqual(releaseAssets(root), [
      join(root, "b.txt"),
      ...(linked ? [join(root, "c.txt")] : []),
      join(root, "schema.json"),
    ]);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("a release without assets attaches none", () => {
  const root = folder({});
  try {
    assert.deepEqual(releaseAssets(join(root, "assets")), []);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("a folder in the assets, a file in their place, and a name of the release's own fail", () => {
  const root = folder({ "notes.md": "notes" });
  try {
    assert.throws(() => releaseAssets(root), /notes\.md can't be an asset/);
    rmSync(join(root, "notes.md"));
    writeFileSync(join(root, "a#b.txt"), "");
    assert.throws(() => releaseAssets(root), /a#b\.txt can't be an asset/);
    rmSync(join(root, "a#b.txt"));
    for (const name of ["-schema.json", "my schema.json", "Notes.md"]) {
      writeFileSync(join(root, name), "");
      assert.throws(() => releaseAssets(root), /can't be an asset/);
      rmSync(join(root, name));
    }
    mkdirSync(join(root, "schemas"));
    assert.throws(() => releaseAssets(root), /schemas is no file/);
    writeFileSync(join(root, "file"), "");
    assert.throws(() => releaseAssets(join(root, "file")), /is a file, not a folder/);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("an asset with the name of an archive fails, and the others are copied", () => {
  const root = folder({ "schema.json": "{}", "tool-1.0.0-linux-x64.tar.gz": "asset" });
  const out = folder({ "tool-1.0.0-linux-x64.tar.gz": "archive" });
  try {
    copyAssets([join(root, "schema.json")], out);
    assert.equal(readFileSync(join(out, "schema.json"), "utf8"), "{}");
    assert.throws(() => {
      copyAssets([join(root, "tool-1.0.0-linux-x64.tar.gz")], out);
    }, /has the name of an archive/);
    assert.equal(readFileSync(join(out, "tool-1.0.0-linux-x64.tar.gz"), "utf8"), "archive");
    writeFileSync(join(root, "TOOL-1.0.0-linux-x64.tar.gz"), "asset");
    assert.throws(() => {
      copyAssets([join(root, "TOOL-1.0.0-linux-x64.tar.gz")], out);
    }, /has the name of an archive/);
  } finally {
    rmSync(root, { force: true, recursive: true });
    rmSync(out, { force: true, recursive: true });
  }
});
