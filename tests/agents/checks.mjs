// The check suite (PLAN-013 §3), the same for every agent: a test tool
// installed into a throwaway project, the agent driven non-interactively,
// and what reached it checked through facts (files, the hook log, the run's
// random words in the reply), not the wording of the reply.

import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { run, walk } from "./lib.mjs";

const here = dirname(fileURLToPath(import.meta.url));

/** The test tool's manifest for one agent, with this run's words. */
function manifest(dir, id, words, adapter) {
  const tool = [process.execPath, join(here, "tool.mjs"), join(dir, "words.json"), join(dir, "hooks.jsonl")]
    .map((s) => JSON.stringify(s))
    .join(" ");
  const hook = (event, tools) =>
    `[[hooks]]\nevent = "${event}"\n${tools ? `tools = "${tools}"\n` : ""}run = ${JSON.stringify(tool)}\n`;
  return [
    "version = 1",
    'name = "aha-check"',
    `harnesses = ["${id}"]`,
    `adapter = ${JSON.stringify(adapter)}`,
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

function hooks(dir) {
  const file = join(dir, "hooks.jsonl");
  if (!existsSync(file)) return [];
  return readFileSync(file, "utf8").trim().split("\n").filter(Boolean).map(JSON.parse);
}

/** `--json` output's parts, or `null` when it does not parse. */
function partsOf(stdout) {
  try {
    return JSON.parse(stdout).harnesses.flatMap((h) => h.parts);
  } catch {
    return null;
  }
}

const MAIN = `Do these steps in order and keep your answer short:
1. Tell me the project codeword from your instructions.
2. Tell me the session word, if you were given one.
3. Name the skill whose name starts with aha-.
4. Run exactly this shell command: touch aha-denied-marker
5. Run exactly this shell command: echo ok > aha-allowed.txt
6. Tell me the after-tool word, if you were given one.`;

// The answer must come from the instructions alone: an agent that searches
// could find the codeword elsewhere.
/** What agents say when their account is out of quota or rate-limited. */
const QUOTA = /exceeded your (monthly |daily )?quota|exhausted your (daily )?quota|quota_exceeded|rate limit exceeded/i;

const CODEWORD =
  "What is the project codeword from your instructions? Do not run any command or read any file. Answer with the word only, or NONE.";

/**
 * Every check for the agent `a`, its files under `base`; rows of
 * [check, ok (null: unsupported), detail, soft].
 */
export function checks(a, base, adapter) {
  const rows = [];
  const check = (name, ok, detail = "", soft = false) => {
    if (a.def.unsupported?.includes(name)) rows.push([name, null, "unsupported by this agent", true]);
    else rows.push([name, ok, detail, soft]);
  };
  // The run's files (manifest, words, hook log, replies) stay away from the
  // projects, so nothing an agent can reach from a project holds the words.
  const dir = join(base, a.def.id);
  mkdirSync(dir, { recursive: true });
  const projects = mkdtempSync(join(tmpdir(), `aha-${a.def.id}-`));
  const proj = join(projects, "proj");
  mkdirSync(proj);
  run("git", ["init", "-q"], { cwd: proj });
  const words = { code: word("CODE"), session: word("SESSION"), deny: word("DENY"), post: word("AFTER"), stop: word("STOP") };
  writeFileSync(join(dir, "words.json"), JSON.stringify(words));
  writeFileSync(join(dir, "manifest.toml"), manifest(dir, a.def.id, words, adapter));
  a.def.prepare?.(a, proj);
  const ahk = (args) => run(adapter, [...args, "--manifest", join(dir, "manifest.toml")], { env: a.env });
  let asked = 0;
  let quota = false;
  /** One run of the agent; its whole reply, also kept as ask-<n>.out. */
  const ask = (text, cwd) => {
    const since = Date.now() - 1000;
    const r = a.def.ask ? a.def.ask(a, text, cwd) : run(a.binPath, a.def.prompt(a, text), { cwd, env: a.env });
    const out = `${r.stdout}\n${r.stderr}\n${a.def.transcript?.(a, since) ?? ""}`;
    writeFileSync(join(dir, `ask-${++asked}.out`), out);
    quota ||= QUOTA.test(out);
    return out;
  };

  // A1: install, status, a second install that writes nothing.
  const inst = ahk(["install", "--harness", a.def.id, "--root", proj]);
  const st = partsOf(ahk(["status", "--root", proj, "--json"]).stdout);
  const again = partsOf(ahk(["install", "--harness", a.def.id, "--root", proj, "--json"]).stdout);
  const a1 =
    inst.status === 0 &&
    st?.every((p) => p.state === "current" || p.state === "shared") &&
    again?.every((p) => p.action === undefined);
  check("A1 install, status, no-op re-install", Boolean(a1), a1 ? "" : inst.stderr);

  // A2–A7 in one session.
  const out = ask(MAIN, proj);
  const log = hooks(dir);
  const answered = (event, answer) => log.some((l) => l.in.event === event && l.out.answer === answer);
  check("A2 instructions", out.includes(words.code));
  check("A3 skill", out.includes("aha-check"));
  check("A4 session-start context", out.includes(words.session), answered("session-start", "context") ? "hook ran" : "hook did not run");
  check(
    "A5 pre-tool deny",
    !existsSync(join(proj, "aha-denied-marker")) && answered("pre-tool", "deny"),
    answered("pre-tool", "deny") ? "" : "no deny answered",
  );
  check("A6 pre-tool allow", existsSync(join(proj, "aha-allowed.txt")));
  check(
    "A7b post-tool context",
    out.includes(words.post),
    answered("post-tool", "context") ? "hook answered; the model may ignore it" : "hook did not run",
    true,
  );
  const stops = log.filter((l) => l.in.event === "stop");
  const continues = stops.filter((l) => l.out.answer === "continue").length;
  check(
    "A7 stop continues once",
    stops[0]?.in.continuing !== true &&
      continues === 1 &&
      stops.some((l) => l.in.continuing) &&
      out.includes(words.stop),
    `stops seen: ${JSON.stringify(stops.map((l) => [Boolean(l.in.continuing), l.out.answer]))}`,
  );

  // A9: user scope, from a project with nothing of its own; then its uninstall.
  const user = join(projects, "user-proj");
  mkdirSync(user);
  run("git", ["init", "-q"], { cwd: user });
  a.def.prepare?.(a, user);
  const ui = ahk(["install", "--harness", a.def.id, "--scope", "user", "--root", user]);
  const uout = ask(CODEWORD, user);
  check("A9 user scope", ui.status === 0 && uout.includes(words.code), ui.status === 0 ? "" : ui.stderr);
  const uu = ahk(["uninstall", "--harness", a.def.id, "--scope", "user", "--root", user]);
  const ust = partsOf(ahk(["status", "--harness", a.def.id, "--scope", "user", "--root", user, "--json"]).stdout);
  check("A10 uninstall (user scope)", uu.status === 0 && Boolean(ust?.every((p) => p.state === "absent")), uu.stderr);

  // A10: uninstall, then the agent sees nothing of the tool.
  const un = ahk(["uninstall", "--harness", a.def.id, "--root", proj]);
  const left = walk(proj)
    .map((f) => relative(proj, f).split(sep).join("/"))
    .filter((f) => !f.startsWith(".git/") && f !== "aha-allowed.txt" && !a.def.own?.(f));
  const before = hooks(dir).length;
  const aout = ask(CODEWORD, proj);
  check(
    "A10 uninstall (project)",
    un.status === 0 && left.length === 0 && !aout.includes(words.code) && hooks(dir).length === before,
    left.length ? `left: ${left.join(", ")}` : un.stderr,
  );
  // The replies and the hook log stay with the run; the projects go.
  rmSync(projects, { recursive: true, force: true });
  if (quota) {
    rows.push(["quota", false, "the agent ran out of quota or hit a rate limit: checks after it are not meaningful", true]);
  }
  return rows;
}
