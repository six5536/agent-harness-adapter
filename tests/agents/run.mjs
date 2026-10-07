#!/usr/bin/env node
// Real-agent checks: install a test tool with agent-harness-adapter into a
// throwaway project, drive each agent non-interactively, and check what
// reached it (checks.mjs). Each agent is a module in agents/.
//
// Every agent runs under the work folder's own HOME, so your agent setup is
// never touched; log in there once with `login`.
//
//   node tests/agents/run.mjs setup [--agents a,b]
//   node tests/agents/run.mjs login <agent>
//   node tests/agents/run.mjs run [--agents a,b] [options]
//
// Options:
//   --work <dir>         work folder (default target/agent-checks)
//   --adapter <path>     the binary under test (default target/debug/agent-harness-adapter)
//   --<agent>-model <m>  the model an agent uses (default: the agent's own)
//   --<agent>-<flag>     an agent's own option (e.g. --codex-full-access)
//
// Exit 0 when every hard check passes. See tests/agents/README.md.

import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { AGENTS } from "./agents/index.mjs";
import { checks } from "./checks.mjs";
import { cleanEnv, fail, run } from "./lib.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const exe = process.platform === "win32" ? ".cmd" : "";

function usage() {
  const flags = Object.values(AGENTS).flatMap((d) =>
    Object.entries(d.flags ?? {}).map(([f, what]) => `  --${d.id}-${f}: ${what}`),
  );
  return [
    "usage: run.mjs setup|login <agent>|run [--agents a,b] [--work <dir>] [--adapter <path>] [--<agent>-model <m>]",
    `agents: ${Object.keys(AGENTS).join(", ")}`,
    ...flags,
    "(see tests/agents/README.md)",
  ].join("\n");
}

function parse(argv) {
  const [command, ...rest] = argv;
  const opts = {
    command,
    agents: Object.keys(AGENTS),
    work: join(repo, "target", "agent-checks"),
    adapter: join(repo, "target", "debug", `agent-harness-adapter${process.platform === "win32" ? ".exe" : ""}`),
    models: {},
    flags: Object.fromEntries(Object.keys(AGENTS).map((id) => [id, {}])),
    positional: [],
  };
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    const value = () => rest[++i] ?? fail(`${a} needs a value`);
    const agentOpt = /^--([a-z]+)-(.+)$/.exec(a);
    if (a === "--agents") opts.agents = value().split(",");
    else if (a === "--work") opts.work = resolve(value());
    else if (a === "--adapter") opts.adapter = resolve(value());
    else if (agentOpt && AGENTS[agentOpt[1]] && agentOpt[2] === "model") opts.models[agentOpt[1]] = value();
    else if (agentOpt && AGENTS[agentOpt[1]]?.flags?.[agentOpt[2]]) opts.flags[agentOpt[1]][agentOpt[2]] = true;
    else if (a.startsWith("--")) fail(`unknown option ${a}\n${usage()}`);
    else opts.positional.push(a);
  }
  for (const id of opts.agents) if (!AGENTS[id]) fail(`no agent ${id}\n${usage()}`);
  return opts;
}

/** The agent `id` as the checks use it. */
function agent(id, opts) {
  const def = AGENTS[id];
  const home = join(opts.work, "home");
  mkdirSync(home, { recursive: true });
  const binPath = join(opts.work, "agents", ...(def.binDir ?? ["node_modules", ".bin"]), `${def.bin}${exe}`);
  if (!existsSync(binPath)) fail(`${id} is not set up: node tests/agents/run.mjs setup --agents ${id}`);
  return {
    def,
    home,
    binPath,
    env: { ...cleanEnv(), ...(def.env ?? {}), HOME: home, USERPROFILE: home },
    model: opts.models[id],
    flags: opts.flags[id],
  };
}

function setup(opts) {
  const dir = join(opts.work, "agents");
  mkdirSync(dir, { recursive: true });
  const defs = opts.agents.map((id) => AGENTS[id]);
  const specs = defs.filter((d) => !d.install).map((d) => `${d.pkg}@${d.version}`);
  if (specs.length > 0) {
    console.log(`installing ${specs.join(", ")} into ${dir}`);
    const npm = process.platform === "win32" ? "npm.cmd" : "npm";
    const r = run(npm, ["install", "--no-audit", "--no-fund", "--prefix", dir, ...specs], {
      stdio: "inherit",
      shell: process.platform === "win32",
    });
    if (r.status !== 0) fail("npm install failed");
  }
  for (const d of defs.filter((d) => d.install)) {
    console.log(`installing ${d.name} ${d.version} into ${dir}`);
    d.install(dir);
  }
}

function runChecks(opts) {
  if (!existsSync(opts.adapter)) fail(`no binary at ${opts.adapter}: cargo build -p agent-harness-adapter`);
  const base = join(opts.work, `run-${new Date().toISOString().replace(/[:.]/g, "-")}`);
  const results = {};
  for (const id of opts.agents) {
    const a = agent(id, opts);
    const version = run(a.binPath, ["--version"], { env: a.env }).stdout.trim().split("\n").pop();
    console.log(`== ${a.def.name} (${id} ${version})`);
    results[id] = checks(a, base, opts.adapter);
    for (const [name, ok, detail, soft] of results[id]) {
      const verdict = ok === null ? "n/a " : ok ? "pass" : soft ? "warn" : "FAIL";
      console.log(`  ${verdict}  ${name}${detail && !ok ? `  (${detail.trim()})` : ""}`);
    }
  }
  writeFileSync(join(base, "results.json"), JSON.stringify(results, null, 2));
  console.log(`\noutputs and hook logs: ${base}`);
  const failed = Object.values(results)
    .flat()
    .some(([, ok, , soft]) => ok === false && !soft);
  process.exit(failed ? 1 : 0);
}

const opts = parse(process.argv.slice(2));
switch (opts.command) {
  case "setup":
    setup(opts);
    break;
  case "login": {
    const id = opts.positional[0] ?? fail(`login which agent?\n${usage()}`);
    if (!AGENTS[id]) fail(`no agent ${id}\n${usage()}`);
    AGENTS[id].login(agent(id, opts));
    break;
  }
  case "run":
    runChecks(opts);
    break;
  default:
    fail(usage());
}
