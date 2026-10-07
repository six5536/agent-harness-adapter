"use strict";
// @zen-component: BND-Node

// agent-harness-adapter for Node: plug a tool into LLM agent harnesses (Claude
// Code, Codex, Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, any
// agent that reads AGENTS.md) from a manifest, and speak every harness's
// hook protocol through one format. Errors throw with the library's message.

const { loadAddon } = require("./addon");

const native = loadAddon();
const versions = JSON.parse(native.versions());

/** [path, json] for the addon: a manifest file's path, or a manifest object. */
function source(manifest) {
  if (typeof manifest === "string") {
    return [manifest, null];
  }
  if (manifest && typeof manifest === "object") {
    return [null, JSON.stringify(manifest)];
  }
  throw new TypeError("manifest: give a manifest file's path or a manifest object");
}

function options(harnesses, opts = {}) {
  return JSON.stringify({
    harnesses: harnesses ?? [],
    scope: opts.scope ?? null,
    root: opts.root ?? null,
    home: opts.home ?? null,
    force: opts.force ?? false,
    without: opts.without ?? null,
  });
}

/** Install the manifest's integration for `harnesses` (ids, or "all"). */
function install(manifest, harnesses, opts) {
  return JSON.parse(native.install(...source(manifest), options(harnesses, opts)));
}

/** The state of each part, for `harnesses` or the installed ones. */
function status(manifest, harnesses, opts) {
  return JSON.parse(native.status(...source(manifest), options(harnesses, opts)));
}

/** The hook contract's input for what `harness` sent at `event`. */
function parseHook(harness, event, text) {
  return JSON.parse(native.parseHook(harness, event, text));
}

/** What the hook command writes for a contract answer: {stdout, stderr, exit}. */
function answerHook(harness, event, answer) {
  return JSON.parse(native.answerHook(harness, event, JSON.stringify(answer)));
}

/**
 * Be a hook command: read the harness's input on stdin, call
 * `decide(input)` (an answer, or a promise of one), write what the harness
 * expects, and resolve to the exit code.
 */
// @zen-impl: BND-2_AC-4
async function runHook(harness, event, decide, io = process) {
  let text = "";
  for await (const chunk of io.stdin) {
    text += chunk;
  }
  const out = answerHook(harness, event, await decide(parseHook(harness, event, text)));
  if (out.stderr) {
    io.stderr.write(out.stderr);
  }
  io.stdout.write(out.stdout);
  return out.exit;
}

/** The JSON Schema of "manifest", "hook-input", "hook-answer" or "result". */
function schema(contract) {
  return JSON.parse(native.schema(contract));
}

module.exports = {
  install,
  status,
  parseHook,
  answerHook,
  runHook,
  schema,
  version: versions.version,
  MANIFEST_VERSION: versions.manifest,
  HOOK_VERSION: versions.hook,
};
