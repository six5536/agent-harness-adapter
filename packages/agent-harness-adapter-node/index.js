"use strict";
// @zen-component: BND-Node

// agent-harness-adapter for Node: one agent setup, every coding agent.
// Install instructions, skills and hooks, kept once in AGENTS.md and .agents/
// (the config) or declared in a tool's manifest, into Claude Code, Codex,
// Gemini CLI, GitHub Copilot, Cursor, Factory Droid, Pi, OpenCode, Kilo Code,
// Qwen Code, Devin and any agent that reads AGENTS.md. Hooks speak one format
// in every harness, Claude Code's. Errors throw with the library's message.

const { loadAddon } = require("./addon");

const native = loadAddon();
const versions = JSON.parse(native.versions());

/** [path, json] for the addon: a manifest file's path, a manifest object, or neither (the config). */
function source(manifest) {
  if (manifest === null || manifest === undefined) {
    return [null, null];
  }
  if (typeof manifest === "string") {
    return [manifest, null];
  }
  if (manifest && typeof manifest === "object") {
    return [null, JSON.stringify(manifest)];
  }
  throw new TypeError("manifest: give a manifest file's path, a manifest object, or null for the config");
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

/** Install the manifest's integration (null: the config's) for `harnesses` (ids, or "all"). */
function install(manifest, harnesses, opts) {
  return JSON.parse(native.install(...source(manifest), options(harnesses, opts)));
}

/**
 * Take the manifest's integration back out of `harnesses` (ids, or "all":
 * the installed ones). A part edited by hand stays unless `force`.
 */
// @zen-impl: BND-1_AC-4
function uninstall(manifest, harnesses, opts) {
  return JSON.parse(native.uninstall(...source(manifest), options(harnesses, opts)));
}

/** The state of each part, for `harnesses` or the installed ones. */
function status(manifest, harnesses, opts) {
  return JSON.parse(native.status(...source(manifest), options(harnesses, opts)));
}

/** The hook contract's input for what `harness` sent at `event`: Claude Code's input, with `harness` and `tool_kind`. */
function parseHook(harness, event, text) {
  return JSON.parse(native.parseHook(harness, event, text));
}

/** What the hook command writes for Claude Code's answer JSON, as `harness` expects it: {stdout, stderr, exit}. */
function answerHook(harness, event, answer) {
  return JSON.parse(native.answerHook(harness, event, JSON.stringify(answer)));
}

/**
 * Be a hook command: read the harness's input on stdin, call
 * `decide(input)` (Claude Code's answer JSON, null to allow, or a promise of
 * either), write what the harness expects, and resolve to the exit code.
 */
// @zen-impl: BND-2_AC-4
async function runHook(harness, event, decide, io = process) {
  let text = "";
  for await (const chunk of io.stdin) {
    text += chunk;
  }
  const out = answerHook(harness, event, (await decide(parseHook(harness, event, text))) ?? {});
  if (out.stderr) {
    io.stderr.write(out.stderr);
  }
  io.stdout.write(out.stdout);
  return out.exit;
}

/** The JSON Schema of "manifest", "hooks", "hook-input", "hook-answer" or "result". */
function schema(contract) {
  return JSON.parse(native.schema(contract));
}

module.exports = {
  install,
  uninstall,
  status,
  parseHook,
  answerHook,
  runHook,
  schema,
  version: versions.version,
  MANIFEST_VERSION: versions.manifest,
};
