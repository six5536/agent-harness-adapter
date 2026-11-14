// Tests how rust-lints-check.mts reads a crate's manifest.

import assert from "node:assert/strict";
import { test } from "node:test";
import { inherits } from "./rust-lints-check.mts";

const pkg = '[package]\nname = "demo"\nversion = "0.1.0"\n';

test("a lints table with workspace set inherits the lints", () => {
  assert.equal(inherits(`${pkg}\n[lints]\nworkspace = true\n`), true);
});

test("a dotted key or an inline table before the first table inherits the lints", () => {
  assert.equal(inherits(`lints.workspace = true\n${pkg}`), true);
  assert.equal(inherits(`lints = { workspace = true }\n${pkg}`), true);
  assert.equal(inherits(`lints = {\n  workspace = true,\n}\n${pkg}`), true);
});

test("a manifest without the opt-in doesn't inherit the lints", () => {
  assert.equal(inherits(pkg), false);
  assert.equal(inherits(`${pkg}\n[lints.rust]\nunsafe_code = "deny"\n`), false);
  assert.equal(inherits(`${pkg}\n[lints]\nworkspace = false\n`), false);
});

test("a dotted key inside another table belongs to that table", () => {
  assert.equal(inherits(`${pkg}lints.workspace = true\n`), false);
});

test("the text of a multi-line string counts for nothing", () => {
  assert.equal(inherits(`${pkg}description = """\n[lints]\nworkspace = true\n"""\n`), false);
});
