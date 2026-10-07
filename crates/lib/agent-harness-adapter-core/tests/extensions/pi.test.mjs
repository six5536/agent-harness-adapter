// The generated Pi extension, run in Node against a stand-in for Pi's API
// (HAR-7_AC-3, HAR-7_AC-4): its hooks are bridged through the
// agent-harness-adapter binary (AHA) to tool.mjs, which answers the hook
// contract. Real Pi reports `context.canContinue: false` at every stop
// (PLAN-013 finding 3), so the stand-in does too.
//
//   AHA=target/debug/agent-harness-adapter node --test crates/lib/agent-harness-adapter-core/tests/extensions/

import { test } from "node:test";
import assert from "node:assert";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const skip = process.env.AHA ? false : "AHA is not set";

/** Install the manifest's Pi hooks into a temp project; return the handlers. */
async function extension(dir) {
  const manifest = join(dir, "t.harness.toml");
  const tool = `${JSON.stringify(process.execPath)} ${JSON.stringify(join(here, "tool.mjs"))}`;
  const hooks = ["session-start", "prompt-submit", "pre-tool", "stop"]
    .map((e) => `[[hooks]]\nevent = "${e}"\nrun = ${JSON.stringify(tool)}\n`)
    .join("\n");
  writeFileSync(
    manifest,
    `version = 1\nname = "t"\nharnesses = ["pi"]\nadapter = ${JSON.stringify(process.env.AHA)}\n${hooks}`,
  );
  const proj = join(dir, "proj");
  mkdirSync(proj);
  const r = spawnSync(process.env.AHA, ["install", "--manifest", manifest, "--harness", "pi", "--root", proj], {
    encoding: "utf8",
    env: { ...process.env, HOME: dir, USERPROFILE: dir },
  });
  assert.strictEqual(r.status, 0, r.stderr);
  const file = join(proj, ".pi", "extensions", "t.ts");
  const handlers = {};
  const mod = await import(pathToFileURL(file).href);
  mod.default({ on: (name, fn) => (handlers[name] = fn) });
  const ctx = {
    cwd: proj,
    sessionManager: { getSessionId: () => "s1", getSessionFile: () => join(dir, "s1.jsonl") },
  };
  return { on: (name, event) => handlers[name](event, ctx) };
}

// @zen-test: HAR-7_AC-3
// @zen-test: HAR-7_AC-4
test("session context reaches the first prompt; stop continues once per prompt", { skip }, async () => {
  const dir = mkdtempSync(join(tmpdir(), "aha-pi-"));
  try {
    const pi = await extension(dir);
    assert.strictEqual(await pi.on("session_start", { reason: "startup" }), undefined);

    const first = await pi.on("before_agent_start", { prompt: "go" });
    assert.deepStrictEqual(first.message.content, "session context\n\nprompt context for go");
    // Kept only once.
    const second = await pi.on("before_agent_start", { prompt: "again" });
    assert.deepStrictEqual(second.message.content, "prompt context for again");

    const settle = { context: { canContinue: false } };
    const stop = await pi.on("agent_before_settle", settle);
    assert.strictEqual(stop.continue, true);
    assert.deepStrictEqual(stop.entries[0].content, "continue once");
    // Asked again with `continuing`: the tool lets it stop.
    assert.strictEqual(await pi.on("agent_before_settle", settle), undefined);
    // A new prompt starts over.
    await pi.on("before_agent_start", { prompt: "third" });
    assert.strictEqual((await pi.on("agent_before_settle", settle)).continue, true);

    const call = await pi.on("tool_call", { toolName: "bash", input: { command: "rm -rf /" } });
    assert.deepStrictEqual(call, { block: true, reason: "no rm -rf" });
    assert.strictEqual(await pi.on("tool_call", { toolName: "bash", input: { command: "ls" } }), undefined);

    const seen = readFileSync(join(dir, "proj", "seen.jsonl"), "utf8").trim().split("\n").map(JSON.parse);
    const stops = seen.filter((i) => i.event === "stop").map((i) => i.continuing ?? false);
    assert.deepStrictEqual(stops, [false, true, false]);
    assert.ok(seen.every((i) => i.v === 1 && i.harness === "pi" && i.session_id === "s1"));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
