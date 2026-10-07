"use strict";

// The Node binding over a built addon (AHK_NODE_ADDON, set by
// `npm run build:node` users and CI). With AHK set to an ahk binary,
// results are also compared with ahk's (BND_P-1).

const { test } = require("node:test");
const assert = require("node:assert");
const { spawnSync } = require("node:child_process");
const { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const { Readable, Writable } = require("node:stream");

const skip = process.env.AHK_NODE_ADDON ? false : "AHK_NODE_ADDON is not set";
const ahk = skip ? null : require("..");

const MANIFEST = { version: 1, name: "t", instructions: "Use t.\n", allow_commands: ["t"] };

function tree(fn) {
  const dir = mkdtempSync(join(tmpdir(), "ahk-node-"));
  const t = { dir, root: join(dir, "proj"), home: join(dir, "home") };
  mkdirSync(t.root);
  mkdirSync(t.home);
  try {
    return fn(t);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// @zen-test: BND-1_AC-1
// @zen-test: BND-1_AC-2
test("install then status", { skip }, () =>
  tree((t) => {
    const out = ahk.install(MANIFEST, ["claude"], { root: t.root, home: t.home });
    assert.strictEqual(out.harnesses[0].parts[0].action, "created");
    assert.match(readFileSync(join(t.root, "CLAUDE.md"), "utf8"), /Use t\./);
    const file = join(t.dir, "t.harness.json");
    writeFileSync(file, JSON.stringify(MANIFEST));
    const st = ahk.status(file, undefined, { root: t.root, home: t.home });
    assert.deepStrictEqual(
      st.harnesses.map((h) => h.harness),
      ["claude"],
    );
    assert.strictEqual(st.harnesses[0].parts[0].state, "current");
    const local = ahk.install(MANIFEST, ["all"], {
      scope: "local",
      root: t.root,
      home: t.home,
      force: true,
      without: ["permissions"],
    });
    assert.deepStrictEqual(
      local.harnesses.map((h) => h.harness),
      ["claude", "copilot"],
    );
  }),
);

// @zen-test: BND-1_AC-3
test("refusals throw the kit's message", { skip }, () =>
  tree((t) => {
    const o = { root: t.root, home: t.home };
    assert.throws(() => ahk.install(MANIFEST, ["vim"], o), /no harness named `vim`/);
    assert.throws(() => ahk.status({ version: 2 }, [], o), /version 2/);
    assert.throws(() => ahk.status(MANIFEST, [], { ...o, scope: "global" }), /no scope named `global`/);
    assert.throws(() => ahk.status(42), TypeError);
  }),
);

// @zen-test: BND_P-1
test("the same results as ahk", { skip: skip || (process.env.AHK ? false : "AHK is not set") }, () =>
  tree((t) => {
    const file = join(t.dir, "t.harness.json");
    writeFileSync(file, JSON.stringify(MANIFEST));
    ahk.install(file, ["claude", "codex"], { root: t.root, home: t.home });
    const theirs = spawnSync(
      process.env.AHK,
      ["status", "--manifest", file, "--root", t.root, "--harness", "claude,codex", "--json"],
      { encoding: "utf8", env: { ...process.env, HOME: t.home, USERPROFILE: t.home } },
    );
    assert.strictEqual(theirs.status, 0, theirs.stderr);
    const mine = ahk.status(file, ["claude", "codex"], { root: t.root, home: t.home });
    assert.deepStrictEqual(JSON.parse(theirs.stdout), mine);
  }),
);

// @zen-test: BND-2_AC-1
// @zen-test: BND-2_AC-2
// @zen-test: BND-2_AC-3
test("hooks", { skip }, () => {
  const input = ahk.parseHook("claude", "stop", '{"session_id":"s","stop_hook_active":true}');
  assert.deepStrictEqual([input.v, input.session_id, input.continuing], [1, "s", true]);
  assert.deepStrictEqual(ahk.parseHook("codex", "stop", "nope"), { v: 1, harness: "codex", event: "stop" });
  assert.deepStrictEqual(ahk.answerHook("claude", "stop", { answer: "continue", reason: "go" }), {
    stdout: '{"decision":"block","reason":"go"}\n',
    stderr: null,
    exit: 0,
  });
  assert.throws(() => ahk.answerHook("claude", "stop", { answer: "deny", reason: "r" }), /cannot answer deny at stop/);
  assert.throws(() => ahk.parseHook("claude", "later", "{}"), /no hook event named `later`/);
});

// @zen-test: BND-2_AC-4
test("runHook is a hook command", { skip }, async () => {
  let out = "";
  let err = "";
  const io = {
    stdin: Readable.from(['{"session_id":', '"s"}']),
    stdout: new Writable({ write: (c, _e, cb) => ((out += c), cb()) }),
    stderr: new Writable({ write: (c, _e, cb) => ((err += c), cb()) }),
  };
  const code = await ahk.runHook(
    "claude",
    "stop",
    async (hook) => ({ answer: "allow", stderr: `saw ${hook.session_id}\n` }),
    io,
  );
  assert.deepStrictEqual([code, out, err], [0, "{}\n", "saw s\n"]);
});

// @zen-test: BND-3_AC-1
// @zen-test: BND-3_AC-2
test("schemas and versions", { skip }, () => {
  for (const c of ["manifest", "hook-input", "hook-answer", "result"]) {
    assert.match(ahk.schema(c).title, /^AHK /);
  }
  assert.throws(() => ahk.schema("nope"), /no contract/);
  assert.deepStrictEqual([ahk.MANIFEST_VERSION, ahk.HOOK_VERSION], [1, 1]);
  assert.match(ahk.version, /^\d+\.\d+\.\d+/);
});
