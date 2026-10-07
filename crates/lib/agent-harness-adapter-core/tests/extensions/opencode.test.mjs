// The generated OpenCode plugin, run in Node against a stand-in for
// OpenCode's plugin API (HAR-10_AC-3, HAR-10_AC-4): its hooks are bridged
// through the agent-harness-adapter binary (AHA) to tool.mjs, which answers
// the hook contract.
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

/** Install the manifest's OpenCode hooks into a temp project; return the plugin's hooks and the prompts it sent. */
async function plugin(dir) {
  const manifest = join(dir, "t.harness.toml");
  const tool = `${JSON.stringify(process.execPath)} ${JSON.stringify(join(here, "tool.mjs"))}`;
  const hooks = ["session-start", "prompt-submit", "pre-tool", "post-tool", "stop"]
    .map((e) => `[[hooks]]\nevent = "${e}"\nrun = ${JSON.stringify(tool)}\n`)
    .join("\n");
  writeFileSync(
    manifest,
    `version = 1\nname = "t"\nharnesses = ["opencode"]\nadapter = ${JSON.stringify(process.env.AHA)}\n${hooks}`,
  );
  const proj = join(dir, "proj");
  mkdirSync(proj);
  const r = spawnSync(process.env.AHA, ["install", "--manifest", manifest, "--harness", "opencode", "--root", proj], {
    encoding: "utf8",
    env: { ...process.env, HOME: dir, USERPROFILE: dir },
  });
  assert.strictEqual(r.status, 0, r.stderr);
  const mod = await import(pathToFileURL(join(proj, ".opencode", "plugins", "t.ts")).href);
  const sent = [];
  const client = { session: { promptAsync: async (o) => sent.push(o) } };
  return { hooks: await mod.default({ client, directory: proj }), sent };
}

/** A user message of `text` as `chat.message` gets it. */
function message(sessionID, text) {
  return {
    message: { id: "msg_1" },
    parts: [{ id: "prt_0", sessionID, messageID: "msg_1", type: "text", text }],
  };
}

// @zen-test: HAR-10_AC-3
// @zen-test: HAR-10_AC-4
test("session context reaches the first prompt; stop continues once per prompt", { skip }, async () => {
  const dir = mkdtempSync(join(tmpdir(), "aha-opencode-"));
  const model = { providerID: "p", modelID: "m" };
  try {
    const { hooks, sent } = await plugin(dir);
    const chat = async (id, text) => {
      const out = message(id, text);
      await hooks["chat.message"]({ sessionID: id, agent: "build", model }, out);
      return out.parts.slice(1);
    };
    const idle = (id) => hooks.event({ event: { type: "session.idle", properties: { sessionID: id } } });

    const first = await chat("s1", "go");
    assert.strictEqual(first.length, 1);
    assert.strictEqual(first[0].text, "session context\n\nprompt context for go");
    assert.ok(first[0].synthetic && first[0].id > "prt_0" && first[0].messageID === "msg_1");
    // Kept only once.
    assert.strictEqual((await chat("s1", "again"))[0].text, "prompt context for again");

    await idle("s1");
    assert.deepStrictEqual(sent, [
      { path: { id: "s1" }, body: { agent: "build", model, parts: [{ type: "text", text: "continue once" }] } },
    ]);
    // The continuation is the plugin's own: no prompt hooks, and the next
    // stop is `continuing`, so the tool lets it stop.
    assert.deepStrictEqual(await chat("s1", "continue once"), []);
    await idle("s1");
    assert.strictEqual(sent.length, 1);
    // A new prompt starts over.
    await chat("s1", "third");
    await idle("s1");
    assert.strictEqual(sent.length, 2);

    // A subagent's session has no prompt or stop hooks of its own.
    await hooks.event({ event: { type: "session.created", properties: { info: { id: "c1", parentID: "s1" } } } });
    assert.deepStrictEqual(await chat("c1", "sub"), []);
    await idle("c1");
    assert.strictEqual(sent.length, 2);

    await assert.rejects(
      hooks["tool.execute.before"]({ tool: "bash", sessionID: "s1", callID: "1" }, { args: { command: "rm -rf /" } }),
      /no rm -rf/,
    );
    await hooks["tool.execute.before"]({ tool: "bash", sessionID: "s1", callID: "2" }, { args: { command: "ls" } });
    const after = { title: "", output: "ok", metadata: {} };
    await hooks["tool.execute.after"]({ tool: "bash", sessionID: "s1", callID: "2", args: { command: "ls" } }, after);
    assert.strictEqual(after.output, "ok\n\nafter tool");

    const seen = readFileSync(join(dir, "proj", "seen.jsonl"), "utf8").trim().split("\n").map(JSON.parse);
    const stops = seen.filter((i) => i.event === "stop").map((i) => i.continuing ?? false);
    assert.deepStrictEqual(stops, [false, true, false]);
    assert.strictEqual(seen.filter((i) => i.event === "session-start").length, 1);
    assert.ok(seen.every((i) => i.v === 1 && i.harness === "opencode" && i.session_id === "s1"));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
