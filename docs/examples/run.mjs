// Runs the site's examples (PLAN-016 N2): each form installs mytool into a
// temporary project, then the hooks it installed for Claude Code and Codex
// must refuse `rm -rf` and allow `ls`.
//
//   node docs/examples/run.mjs <form>...     forms: command, rust, node, python
//
// command: AHA, the agent-harness-adapter binary.
// rust:    RUST_EXAMPLE, the built `mytool` example of agent-harness-adapter-core.
// node:    AHA_NODE_ADDON, a built addon.
// python:  PYTHON, a Python with the binding installed (default python3).

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const nodePackage = path.resolve(here, "../../packages/agent-harness-adapter-node");
const need = (name) => {
  const v = process.env[name];
  if (!v) throw new Error(`${name} is not set`);
  return path.resolve(v);
};

// Per form: copy what it needs into `proj`, install, and say what `mytool`
// on PATH runs (`mytool hook ...`).
const FORMS = {
  command(proj, env) {
    fs.copyFileSync(path.join(here, "mytool.harness.toml"), path.join(proj, "mytool.harness.toml"));
    run(proj, need("AHA"), ["install", "--manifest", "mytool.harness.toml", "--harness", "claude,codex,gemini"], { env });
    return `exec "${path.join(here, "mytool-hook.sh")}"`;
  },
  rust(proj, env) {
    const bin = need("RUST_EXAMPLE");
    run(proj, bin, ["install", proj], { env });
    return `exec "${bin}" "$@"`;
  },
  node(proj, env) {
    need("AHA_NODE_ADDON");
    const scope = path.join(proj, "node_modules/@six5536");
    fs.mkdirSync(scope, { recursive: true });
    fs.symlinkSync(nodePackage, path.join(scope, "agent-harness-adapter-node"), "junction");
    for (const f of ["install.mjs", "hook.mjs"]) fs.copyFileSync(path.join(here, f), path.join(proj, f));
    run(proj, process.execPath, ["install.mjs"], { env });
    return `shift; exec "${process.execPath}" "${path.join(proj, "hook.mjs")}" "$@"`;
  },
  python(proj, env) {
    const py = process.env.PYTHON ?? "python3";
    for (const f of ["install.py", "hook.py"]) fs.copyFileSync(path.join(here, f), path.join(proj, f));
    run(proj, py, ["install.py"], { env });
    return `shift; exec "${py}" "${path.join(proj, "hook.py")}" "$@"`;
  },
};

// A shell call, in Claude Code's and Codex's words.
const CALLS = {
  claude: (cmd) => ({ hook_event_name: "PreToolUse", tool_name: "Bash", tool_input: { command: cmd } }),
  codex: (cmd) => ({ hook_event_name: "PreToolUse", tool_name: "shell", tool_input: { command: ["bash", "-lc", cmd] } }),
};
const FILES = { claude: ".claude/settings.json", codex: ".codex/hooks.json" };

function run(cwd, cmd, args, opts = {}) {
  const r = spawnSync(cmd, args, { cwd, encoding: "utf8", ...opts });
  if (r.status !== 0 && !opts.any) {
    throw new Error(`${cmd} ${args.join(" ")}: exit ${r.status}\n${r.stdout}${r.stderr}${r.error ?? ""}`);
  }
  return r;
}

function check(form) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `aha-example-${form}-`));
  try {
    const proj = path.join(dir, "proj");
    const bin = path.join(dir, "bin");
    fs.mkdirSync(proj);
    fs.mkdirSync(bin);
    const env = { ...process.env, HOME: dir, USERPROFILE: dir, PATH: `${bin}${path.delimiter}${process.env.AHA ? path.dirname(path.resolve(process.env.AHA)) : ""}${path.delimiter}${process.env.PATH}` };
    if (env.AHA_NODE_ADDON) env.AHA_NODE_ADDON = path.resolve(env.AHA_NODE_ADDON);
    const body = FORMS[form](proj, env);
    fs.writeFileSync(path.join(bin, "mytool"), `#!/bin/sh\n${body}\n`, { mode: 0o755 });
    for (const [harness, file] of Object.entries(FILES)) {
      const settings = JSON.parse(fs.readFileSync(path.join(proj, file), "utf8"));
      const [command] = settings.hooks.PreToolUse.flatMap((g) => g.hooks.map((h) => h.command));
      for (const [cmd, denied] of [["rm -rf build", true], ["ls", false]]) {
        const r = run(proj, "sh", ["-c", command], { env, input: JSON.stringify(CALLS[harness](cmd)), any: true });
        const said = `${r.stdout}${r.stderr}`;
        const ok = denied ? said.includes("mytool: no rm -rf here") : r.status === 0 && !said.includes("rm -rf");
        if (!ok) throw new Error(`${form} ${harness} \`${cmd}\`: exit ${r.status}\n${said}`);
      }
    }
    console.log(`${form}: ok`);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

const forms = process.argv.slice(2);
if (!forms.length || forms.some((f) => !FORMS[f])) {
  console.error(`usage: run.mjs <${Object.keys(FORMS).join("|")}>...`);
  process.exit(2);
}
for (const form of forms) check(form);
