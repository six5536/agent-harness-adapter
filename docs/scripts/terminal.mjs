// The landing page's terminal (PLAN-016 D16-4): runs the built command in a
// temporary repository with a Claude Code setup and records each scene's real
// output in src/data/terminal.json.
//
//   node docs/scripts/terminal.mjs <agent-harness-adapter> [--check]
//
// With --check it writes nothing and fails when the recorded file is out of
// date, so the page never shows output the command no longer gives.

import { execFileSync, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const out = path.join(here, "../src/data/terminal.json");
const [binArg, flag] = process.argv.slice(2);
if (!binArg) {
  console.error("usage: terminal.mjs <agent-harness-adapter> [--check]");
  process.exit(2);
}
const bin = path.resolve(binArg);
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "aha-terminal-"));
const repo = path.join(tmp, "my-repo");
const env = {
  ...process.env,
  HOME: tmp,
  PATH: `${path.dirname(bin)}${path.delimiter}${process.env.PATH}`,
  GIT_CONFIG_NOSYSTEM: "1",
};
delete env.USERPROFILE;

function write(rel, text, mode) {
  const p = path.join(repo, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, text, mode ? { mode } : undefined);
}

function git(...args) {
  execFileSync(
    "git",
    ["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", ...args],
    { cwd: repo, env, stdio: "ignore" },
  );
}

// Runs `command` (shell text) in the repository: its output, stdout and
// stderr together as a terminal shows them, and its exit code.
function sh(command, input) {
  const r = spawnSync("sh", ["-c", `exec 2>&1\n${command}`], { cwd: repo, env, input, encoding: "utf8" });
  const text = r.stdout.replaceAll(`${repo}${path.sep}`, "").replaceAll(repo, ".");
  return { text: text.trimEnd(), code: r.status };
}

// A scene line: a prompt with a command, the output, or a comment.
function run(lines, command, { shown = command, input, exit = false } = {}) {
  lines.push({ kind: "cmd", text: shown });
  const r = sh(command, input);
  for (const line of r.text ? r.text.split("\n") : []) lines.push({ kind: "out", text: line });
  if (exit) lines.push({ kind: "exit", text: `exit ${r.code}` });
  return r;
}
const note = (lines, text) => lines.push({ kind: "note", text: `# ${text}` });

// A repository set up for Claude Code only.
fs.mkdirSync(repo);
write("CLAUDE.md", "Use pnpm, not npm.\n");
write(
  ".claude/skills/review/SKILL.md",
  "---\nname: review\ndescription: Review the change before you stop.\n---\nRead the diff and list problems.\n",
);
write(
  ".claude/settings.json",
  `${JSON.stringify(
    { hooks: { PreToolUse: [{ matcher: "Bash", hooks: [{ type: "command", command: "scripts/guard.sh" }] }] } },
    null,
    2,
  )}\n`,
);
write(
  "scripts/guard.sh",
  '#!/bin/sh\n# Block rm -rf, in every agent.\nif grep -q "rm -rf"; then\n  echo "guard: no rm -rf here" >&2\n  exit 2\nfi\n',
  0o755,
);
write(".mcp.json", `${JSON.stringify({ mcpServers: { docs: { command: "npx", args: ["-y", "docs-mcp"] } } }, null, 2)}\n`);
write(
  ".claude/agents/reviewer.md",
  "---\nname: reviewer\ndescription: Reviews a change.\n---\nReview the diff for bugs.\n",
);
git("init", "-q");
git("add", "-A");
git("commit", "-qm", "Claude Code setup");

const scenes = [];

{
  const lines = [];
  note(lines, "A repository set up for Claude Code");
  run(lines, "agent-harness-adapter install --migrate --harness claude,codex,opencode", {
    shown: "agent-harness-adapter install --migrate --harness claude,codex,opencode",
  });
  scenes.push({ id: "install", title: "install", lines });
}

{
  const lines = [];
  note(lines, ".agents/hooks.json runs scripts/guard.sh before Bash");
  note(lines, "Each agent asks to run `rm -rf build`, in its own words");
  const settings = JSON.parse(fs.readFileSync(path.join(repo, ".claude/settings.json"), "utf8"));
  const codex = JSON.parse(fs.readFileSync(path.join(repo, ".codex/hooks.json"), "utf8"));
  const plugin = fs.readFileSync(path.join(repo, ".opencode/plugins/agents.ts"), "utf8");
  const opencode = /"command":"([^"]*opencode pre-tool)"/.exec(plugin)[1];
  const calls = [
    [
      "Claude Code",
      settings.hooks.PreToolUse[0].hooks[0].command,
      { hook_event_name: "PreToolUse", tool_name: "Bash", tool_input: { command: "rm -rf build" } },
    ],
    [
      "Codex",
      codex.hooks.PreToolUse[0].hooks[0].command,
      {
        hook_event_name: "PreToolUse",
        tool_name: "shell",
        tool_input: { command: ["bash", "-lc", "rm -rf build"] },
      },
    ],
    ["OpenCode", opencode, { tool_name: "bash", tool_input: { command: "rm -rf build" }, cwd: repo }],
  ];
  for (const [name, command, input] of calls) {
    note(lines, `${name}: ${JSON.stringify({ tool_name: input.tool_name, tool_input: input.tool_input }).replaceAll(repo, ".")}`);
    run(lines, command, { input: JSON.stringify(input), exit: true });
  }
  scenes.push({ id: "hooks", title: "one hook, three agents", lines });
}

{
  const lines = [];
  run(lines, "agent-harness-adapter status", { exit: true });
  note(lines, "Someone edits Codex's hooks by hand");
  const p = path.join(repo, ".codex/hooks.json");
  fs.writeFileSync(p, fs.readFileSync(p, "utf8").replace('"timeout": 60', '"timeout": 5'));
  run(lines, "agent-harness-adapter install");
  run(lines, "agent-harness-adapter status", { exit: true });
  scenes.push({ id: "status", title: "status", lines });
}

{
  const lines = [];
  // Back to what install wrote.
  run([], "agent-harness-adapter install --force");
  run(lines, "agent-harness-adapter uninstall");
  note(lines, "It took out only what it wrote: CLAUDE.md held just its @AGENTS.md line");
  note(lines, "Your sources stay");
  run(lines, "ls -A . .agents", { shown: "ls -A . .agents" });
  scenes.push({ id: "uninstall", title: "uninstall", lines });
}

fs.rmSync(tmp, { recursive: true, force: true });

const text = `${JSON.stringify({ scenes }, null, 2)}\n`;
if (flag === "--check") {
  const old = fs.existsSync(out) ? fs.readFileSync(out, "utf8") : "";
  if (old !== text) {
    console.error(`${path.relative(process.cwd(), out)} is out of date: run node docs/scripts/terminal.mjs ${binArg}`);
    process.exit(1);
  }
  console.log("terminal OK");
} else {
  fs.writeFileSync(out, text);
  console.log(`wrote ${path.relative(process.cwd(), out)}`);
}
