#!/usr/bin/env node
// Real-agent checks (PLAN-013 §3): install a test tool with
// agent-harness-adapter into a throwaway project, drive each agent
// non-interactively, and check what reached it: instructions, skill,
// session context, a denied and an allowed shell command, after-tool
// context, a stop that continues once, user scope, and uninstall.
//
// Each agent runs under its own HOME in the work folder, so your own
// agent setup is never touched; log in there once with `login`.
//
//   node scripts/agents/run.mjs setup [--agents claude,codex,gemini,copilot,pi]
//   node scripts/agents/run.mjs login <agent>
//   node scripts/agents/run.mjs run [--agents ...] [options]
//
// Options:
//   --work <dir>        work folder (default target/agent-checks)
//   --adapter <path>    the binary under test (default target/debug/agent-harness-adapter)
//   --<agent>-model <m> the model an agent uses (default: the agent's own)
//   --codex-full-access run Codex without its sandbox (needed where it cannot
//                       start, e.g. dev containers without user namespaces)
//
// Exit 0 when every hard check passes. See scripts/agents/README.md.

import { spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "..", "..");
const exe = process.platform === "win32" ? ".cmd" : "";

/** The agents, pinned to the versions last checked (PLAN-013 §6). */
const AGENTS = {
  claude: {
    pkg: "@anthropic-ai/claude-code",
    version: "2.1.292",
    bin: "claude",
    login: (a) => run(a.binPath, ["auth", "login"], { env: a.env, stdio: "inherit" }),
    // -p skips the workspace trust dialog; the hooks refuse what they must,
    // so tool permissions are bypassed. stream-json holds every message.
    prompt: (a, text) => [
      "-p",
      "--output-format",
      "stream-json",
      "--verbose",
      "--dangerously-skip-permissions",
      ...(a.opts.claudeModel ? ["--model", a.opts.claudeModel] : []),
      text,
    ],
    prepare: () => {},
  },
  gemini: {
    pkg: "@google/gemini-cli",
    version: "0.63.0",
    bin: "gemini",
    login: (a) =>
      console.log(
        `Gemini CLI logs in from its own screen. In a terminal, run:\n\n` +
          `  HOME=${JSON.stringify(a.home)} ${JSON.stringify(a.binPath)}\n\n` +
          `pick "Sign in with Google" (or set GEMINI_API_KEY), then quit.`,
      ),
    // yolo approves every tool (the hooks refuse what they must);
    // GEMINI_CLI_TRUST_WORKSPACE trusts the folder in a headless run.
    prompt: (a, text) => [
      "-p",
      text,
      "--approval-mode",
      "yolo",
      "-o",
      "stream-json",
      ...(a.opts.geminiModel ? ["-m", a.opts.geminiModel] : []),
    ],
    env: { GEMINI_CLI_TRUST_WORKSPACE: "true" },
    prepare: () => {},
  },
  copilot: {
    pkg: "@github/copilot",
    version: "1.0.92",
    bin: "copilot",
    login: (a) => run(a.binPath, ["login", "--device-code"], { env: a.env, stdio: "inherit" }),
    prompt: (a, text) => [
      "-p",
      text,
      "--allow-all",
      "--no-ask-user",
      "--output-format",
      "json",
      ...(a.opts.copilotModel ? ["--model", a.opts.copilotModel] : []),
    ],
    prepare: () => {},
  },
  codex: {
    pkg: "@openai/codex",
    version: "0.160.1",
    bin: "codex",
    login: (a) => run(a.binPath, ["login", "--device-auth"], { env: a.env, stdio: "inherit" }),
    prompt: (a, text) => [
      "exec",
      // Stands in for approving new hooks in Codex's /hooks (check A8).
      "--dangerously-bypass-hook-trust",
      "--sandbox",
      a.opts.codexFullAccess ? "danger-full-access" : "workspace-write",
      ...(a.opts.codexModel ? ["--model", a.opts.codexModel] : []),
      text,
    ],
    /** Trust the project, as answering Codex's trust prompt does. */
    prepare: (a, proj) => {
      const dir = join(a.home, ".codex");
      mkdirSync(dir, { recursive: true });
      const file = join(dir, "config.toml");
      const table = `[projects.${JSON.stringify(proj)}]`;
      const text = existsSync(file) ? readFileSync(file, "utf8") : "";
      if (!text.includes(table)) appendFileSync(file, `\n${table}\ntrust_level = "trusted"\n`);
    },
  },
  pi: {
    pkg: "@earendil-works/pi-coding-agent",
    version: "1.0.4",
    bin: "pi",
    login: (a) =>
      console.log(
        `Pi logs in from its own screen. In a terminal, run:\n\n` +
          `  HOME=${JSON.stringify(a.home)} ${JSON.stringify(a.binPath)}\n\n` +
          `then type /login, pick a provider, and quit.`,
      ),
    // --approve trusts the project's files for this run, as answering Pi's
    // trust prompt does.
    prompt: (a, text) => ["--approve", ...(a.opts.piModel ? ["--model", a.opts.piModel] : []), "-p", text],
    prepare: () => {},
    // `pi -p` prints only the last message (after a stop that continued,
    // the reply to it): read every assistant message of the session file.
    transcript: (a, since) => {
      const files = walk(join(a.home, ".pi", "agent", "sessions")).filter(
        (f) => f.endsWith(".jsonl") && statSync(f).mtimeMs >= since,
      );
      const newest = files.sort((x, y) => statSync(y).mtimeMs - statSync(x).mtimeMs)[0];
      if (!newest) return "";
      return readFileSync(newest, "utf8")
        .split("\n")
        .filter(Boolean)
        .map((l) => JSON.parse(l).message)
        .filter((m) => m?.role === "assistant")
        .flatMap((m) => (Array.isArray(m.content) ? m.content : []))
        .filter((c) => c.type === "text")
        .map((c) => c.text)
        .join("\n");
    },
  },
};

/**
 * The environment without a surrounding Claude Code session's variables
 * (`CLAUDE_CONFIG_DIR` would point an agent at the real config), so each
 * agent sees only the checks' HOME.
 */
function cleanEnv() {
  return Object.fromEntries(Object.entries(process.env).filter(([k]) => !/^CLAUDE/.test(k)));
}

/** Every file under `dir`. */
function walk(dir) {
  if (!existsSync(dir)) return [];
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? walk(join(dir, e.name)) : [join(dir, e.name)],
  );
}

function fail(message) {
  console.error(`agent checks: ${message}`);
  process.exit(2);
}

function run(cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { encoding: "utf8", input: "", timeout: 600_000, ...opts });
  if (r.error) fail(`could not run ${cmd}: ${r.error.message}`);
  return r;
}

function parse(argv) {
  const [command, ...rest] = argv;
  const opts = {
    command,
    agents: Object.keys(AGENTS),
    work: join(repo, "target", "agent-checks"),
    adapter: join(repo, "target", "debug", `agent-harness-adapter${process.platform === "win32" ? ".exe" : ""}`),
    positional: [],
  };
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i];
    const value = () => rest[++i] ?? fail(`${a} needs a value`);
    if (a === "--agents") opts.agents = value().split(",");
    else if (a === "--work") opts.work = resolve(value());
    else if (a === "--adapter") opts.adapter = resolve(value());
    else if (a === "--codex-model") opts.codexModel = value();
    else if (a === "--pi-model") opts.piModel = value();
    else if (a === "--claude-model") opts.claudeModel = value();
    else if (a === "--gemini-model") opts.geminiModel = value();
    else if (a === "--copilot-model") opts.copilotModel = value();
    else if (a === "--codex-full-access") opts.codexFullAccess = true;
    else if (a.startsWith("--")) fail(`unknown option ${a}`);
    else opts.positional.push(a);
  }
  for (const id of opts.agents) if (!AGENTS[id]) fail(`no agent ${id}: ${Object.keys(AGENTS).join(", ")}`);
  return opts;
}

/** An agent as the checks use it: its binary and environment. */
function agent(id, opts) {
  const def = AGENTS[id];
  const home = join(opts.work, "home");
  mkdirSync(home, { recursive: true });
  const bin = join(opts.work, "agents", "node_modules", ".bin", `${def.bin}${exe}`);
  return {
    id,
    def,
    opts,
    home,
    binPath: bin,
    env: { ...cleanEnv(), ...(def.env ?? {}), HOME: home, USERPROFILE: home },
  };
}

function setup(opts) {
  const dir = join(opts.work, "agents");
  mkdirSync(dir, { recursive: true });
  const specs = opts.agents.map((id) => `${AGENTS[id].pkg}@${AGENTS[id].version}`);
  console.log(`installing ${specs.join(", ")} into ${dir}`);
  const r = run(process.platform === "win32" ? "npm.cmd" : "npm", ["install", "--no-audit", "--no-fund", "--prefix", dir, ...specs], {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (r.status !== 0) fail("npm install failed");
}

/** The test tool's manifest for one agent, with this run's words. */
function manifest(dir, id, words, opts) {
  const tool = `${JSON.stringify(process.execPath)} ${JSON.stringify(join(here, "tool.mjs"))} ${JSON.stringify(join(dir, "words.json"))} ${JSON.stringify(join(dir, "hooks.jsonl"))}`;
  const hook = (event, tools) =>
    `[[hooks]]\nevent = "${event}"\n${tools ? `tools = "${tools}"\n` : ""}run = ${JSON.stringify(tool)}\n`;
  return [
    "version = 1",
    'name = "aha-check"',
    `harnesses = ["${id}"]`,
    `adapter = ${JSON.stringify(opts.adapter)}`,
    `instructions = ${JSON.stringify(`This project's codeword is ${words.code}.\n`)}`,
    "",
    "[[skills]]",
    'name = "aha-check"',
    'description = "Explains the aha-check test tool."',
    'body = "The aha-check tool checks agent harness integrations.\\n"',
    "",
    hook("session-start"),
    hook("pre-tool", "shell"),
    hook("post-tool", "shell"),
    hook("stop"),
  ].join("\n");
}

function word(prefix) {
  return `${prefix}-${randomBytes(3).toString("hex")}`;
}

function adapterRun(opts, dir, args, env) {
  return run(opts.adapter, [...args, "--manifest", join(dir, "manifest.toml")], { env });
}

function hooks(dir) {
  const file = join(dir, "hooks.jsonl");
  if (!existsSync(file)) return [];
  return readFileSync(file, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
}

const MAIN = `Do these steps in order and keep your answer short:
1. Tell me the project codeword from your instructions.
2. Tell me the session word, if you were given one.
3. Name the skill whose name starts with aha-.
4. Run exactly this shell command: touch aha-denied-marker
5. Run exactly this shell command: echo ok > aha-allowed.txt
6. Tell me the after-tool word, if you were given one.`;

/** Run every check for one agent; returns rows of [check, ok, detail, soft]. */
function checks(a, base) {
  const rows = [];
  const check = (name, ok, detail = "", soft = false) => rows.push([name, ok, detail, soft]);
  const dir = join(base, a.id);
  const proj = join(dir, "proj");
  mkdirSync(proj, { recursive: true });
  run("git", ["init", "-q"], { cwd: proj });
  const words = { code: word("CODE"), session: word("SESSION"), deny: word("DENY"), post: word("AFTER"), stop: word("STOP") };
  writeFileSync(join(dir, "words.json"), JSON.stringify(words));
  writeFileSync(join(dir, "manifest.toml"), manifest(dir, a.id, words, a.opts));
  a.def.prepare(a, proj);
  const env = a.env;
  const ask = (text, cwd) => {
    const since = Date.now() - 1000;
    const r = run(a.binPath, a.def.prompt(a, text), { cwd, env });
    const extra = a.def.transcript ? a.def.transcript(a, since) : "";
    return `${r.stdout}\n${r.stderr}\n${extra}`;
  };

  // A1: install, status, a second install that writes nothing.
  const inst = adapterRun(a.opts, dir, ["install", "--harness", a.id, "--root", proj], env);
  const st = adapterRun(a.opts, dir, ["status", "--root", proj, "--json"], env);
  const again = adapterRun(a.opts, dir, ["install", "--harness", a.id, "--root", proj, "--json"], env);
  let a1 = inst.status === 0 && st.status === 0 && again.status === 0;
  try {
    const states = JSON.parse(st.stdout).harnesses.flatMap((h) => h.parts.map((p) => p.state));
    const actions = JSON.parse(again.stdout).harnesses.flatMap((h) => h.parts.map((p) => p.action ?? null));
    a1 &&= states.every((s) => s === "current" || s === "shared") && actions.every((x) => x === null);
  } catch {
    a1 = false;
  }
  check("A1 install, status, no-op re-install", a1, a1 ? "" : `${inst.stderr}${st.stderr}${again.stderr}`);

  // A2–A7 in one session.
  const out = ask(MAIN, proj);
  writeFileSync(join(dir, "main.out"), out);
  const log = hooks(dir);
  const answered = (event, answer) => log.some((l) => l.in.event === event && l.out.answer === answer);
  check("A2 instructions", out.includes(words.code));
  check("A3 skill", out.includes("aha-check"));
  check("A4 session-start context", out.includes(words.session), answered("session-start", "context") ? "hook ran" : "hook did not run");
  check("A5 pre-tool deny", !existsSync(join(proj, "aha-denied-marker")) && answered("pre-tool", "deny"), answered("pre-tool", "deny") ? "" : "no deny answered");
  check("A6 pre-tool allow", existsSync(join(proj, "aha-allowed.txt")));
  check(
    "A7b post-tool context",
    out.includes(words.post),
    answered("post-tool", "context") ? "hook answered; the model may ignore it" : "hook did not run",
    true,
  );
  const stops = log.filter((l) => l.in.event === "stop").map((l) => Boolean(l.in.continuing));
  check("A7 stop continues once", stops[0] === false && stops.includes(true) && out.includes(words.stop), `stops seen: ${JSON.stringify(stops)}`);

  // A9: user scope, from a project with nothing of its own.
  const user = join(dir, "user-proj");
  mkdirSync(user, { recursive: true });
  run("git", ["init", "-q"], { cwd: user });
  a.def.prepare(a, user);
  const ui = adapterRun(a.opts, dir, ["install", "--harness", a.id, "--scope", "user", "--root", user], env);
  const uout = ask("What is the project codeword from your instructions? Answer with the word only, or NONE.", user);
  check("A9 user scope", ui.status === 0 && uout.includes(words.code), ui.status === 0 ? "" : ui.stderr);
  const uu = adapterRun(a.opts, dir, ["uninstall", "--harness", a.id, "--scope", "user", "--root", user], env);
  const ust = adapterRun(a.opts, dir, ["status", "--harness", a.id, "--scope", "user", "--root", user, "--json"], env);
  let gone = uu.status === 0 && ust.status === 0;
  try {
    gone &&= JSON.parse(ust.stdout).harnesses.every((h) => h.parts.every((p) => p.state === "absent"));
  } catch {
    gone = false;
  }
  check("A10 uninstall (user scope)", gone, `${uu.stderr}${ust.stdout}`);

  // A10: uninstall, then the agent sees nothing of the tool.
  const un = adapterRun(a.opts, dir, ["uninstall", "--harness", a.id, "--root", proj], env);
  const left = readdirSync(proj).filter((f) => ![".git", "aha-allowed.txt"].includes(f));
  const before = hooks(dir).length;
  const aout = ask("What is the project codeword from your instructions? Answer with the word only, or NONE.", proj);
  check(
    "A10 uninstall (project)",
    un.status === 0 && left.length === 0 && !aout.includes(words.code) && hooks(dir).length === before,
    left.length ? `left: ${left.join(", ")}` : un.stderr,
  );
  return rows;
}

function runChecks(opts) {
  if (!existsSync(opts.adapter)) fail(`no binary at ${opts.adapter}: cargo build -p agent-harness-adapter`);
  const base = join(opts.work, `run-${new Date().toISOString().replace(/[:.]/g, "-")}`);
  const results = {};
  for (const id of opts.agents) {
    const a = agent(id, opts);
    if (!existsSync(a.binPath)) fail(`${id} is not set up: node scripts/agents/run.mjs setup --agents ${id}`);
    const version = run(a.binPath, ["--version"], { env: a.env }).stdout.trim().split("\n").pop();
    console.log(`== ${id} (${version})`);
    results[id] = checks(a, base);
    for (const [name, ok, detail, soft] of results[id]) {
      console.log(`  ${ok ? "pass" : soft ? "warn" : "FAIL"}  ${name}${detail && !ok ? `  (${detail.trim()})` : ""}`);
    }
  }
  writeFileSync(join(base, "results.json"), JSON.stringify(results, null, 2));
  console.log(`\noutputs and hook logs: ${base}`);
  const failed = Object.values(results).flat().some(([, ok, , soft]) => !ok && !soft);
  process.exit(failed ? 1 : 0);
}

const opts = parse(process.argv.slice(2));
switch (opts.command) {
  case "setup":
    setup(opts);
    break;
  case "login": {
    const id = opts.positional[0] ?? fail("login which agent? codex or pi");
    if (!AGENTS[id]) fail(`no agent ${id}`);
    const a = agent(id, opts);
    if (!existsSync(a.binPath)) fail(`${id} is not set up: node scripts/agents/run.mjs setup --agents ${id}`);
    AGENTS[id].login(a);
    break;
  }
  case "run":
    runChecks(opts);
    break;
  default:
    fail("usage: run.mjs setup|login <agent>|run [--agents codex,pi] (see scripts/agents/README.md)");
}
