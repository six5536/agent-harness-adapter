// Helpers the agent checks and the agents share.

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import { join } from "node:path";

/** Stop the checks with a message (exit 2: the checks could not run). */
export function fail(message) {
  console.error(`agent checks: ${message}`);
  process.exit(2);
}

/** Run a program to completion; a program that cannot start stops the checks. */
export function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { encoding: "utf8", input: "", timeout: 600_000, ...opts });
  if (r.error) fail(`could not run ${cmd}: ${r.error.message}`);
  return r;
}

/**
 * The environment without a surrounding Claude Code session's variables
 * (`CLAUDE_CONFIG_DIR` would point an agent at the real config), so each
 * agent sees only the checks' HOME.
 */
export function cleanEnv() {
  return Object.fromEntries(Object.entries(process.env).filter(([k]) => !/^CLAUDE/.test(k)));
}

/** Every file under `dir`. */
export function walk(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)],
  );
}

/** A login that happens on the agent's own screen: say how to start it there. */
export function loginOnScreen(a, steps) {
  console.log(
    `${a.def.name} logs in from its own screen. In a terminal, run:\n\n` +
      `  HOME=${JSON.stringify(a.home)} ${JSON.stringify(a.binPath)}\n\n` +
      `then ${steps}, and quit.`,
  );
}
