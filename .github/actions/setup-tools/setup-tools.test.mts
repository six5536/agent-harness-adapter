// Tests the step of setup-tools that keeps the tools in one folder: a job that names its
// tools gets them as MISE_ENABLE_TOOLS, and mise then installs a cargo: tool of the list
// without the sccache that the mise files pin outside it.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { after, test } from "node:test";

const action = join(import.meta.dirname, "action.yml");

const roots: string[] = [];
after(() => {
  for (const root of roots) {
    rmSync(root, { recursive: true, force: true });
  }
});

function scratch(): string {
  const root = mkdtempSync(join(tmpdir(), "setup-tools-"));
  roots.push(root);
  return root;
}

function indent(line: string): number {
  return line.length - line.trimStart().length;
}

// The script of a step of the action, by the step's name, without its indent.
function stepScript(name: string): string {
  const lines = readFileSync(action, "utf8").split(/\r?\n/u);
  const start = lines.findIndex((line) => line.trim() === `- name: ${name}`);
  assert.notEqual(start, -1, `no step ${name}`);
  const run = lines.findIndex((line, index) => index > start && line.trim() === "run: |");
  const body: string[] = [];
  for (const line of lines.slice(run + 1)) {
    if (line.trim() !== "" && indent(line) <= indent(lines[run] ?? "")) {
      break;
    }
    body.push(line);
  }
  const depth = Math.min(...body.filter((line) => line.trim() !== "").map((line) => indent(line)));
  return body.map((line) => line.slice(depth)).join("\n");
}

// The lines that the step writes to $GITHUB_ENV for the input `tools`.
function stepEnv(tools: string, env: Record<string, string> = {}): string[] {
  const root = scratch();
  const githubEnv = join(root, "github-env");
  writeFileSync(githubEnv, "");
  const run = spawnSync("bash", ["-e", "-c", stepScript("Keep the tools in one folder")], {
    env: { PATH: process.env.PATH, HOME: root, GITHUB_ENV: githubEnv, JOB_TOOLS: tools, ...env },
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  return readFileSync(githubEnv, "utf8").trimEnd().split("\n");
}

// Native Windows runs the step in the bash of Git for Windows, which the PATH of a test
// doesn't pick before the bash of WSL.
const posix = process.platform !== "win32";

test("a job that names its tools uses only those", { skip: !posix }, () => {
  const lines = stepEnv(" node rust  cargo-binstall cargo:mdbook ");
  assert.ok(
    lines.includes("MISE_ENABLE_TOOLS=node,rust,cargo-binstall,cargo:mdbook"),
    lines.join("\n"),
  );
  assert.equal(lines.filter((line) => line.startsWith("MISE_ENABLE_TOOLS=")).length, 1);
});

test(
  "a tool of the list with a version is enabled by its name, and a scope of npm stays",
  { skip: !posix },
  () => {
    const lines = stepEnv("node@22 cargo:mdbook@0.4 npm:@scope/name npm:@scope/other@1.2.3 jq");
    assert.ok(
      lines.includes("MISE_ENABLE_TOOLS=node,cargo:mdbook,npm:@scope/name,npm:@scope/other,jq"),
      lines.join("\n"),
    );
  },
);

test("a list that the job enables already keeps its tools", { skip: !posix }, () => {
  const lines = stepEnv("node", { MISE_ENABLE_TOOLS: "python" });
  assert.ok(lines.includes("MISE_ENABLE_TOOLS=python,node"), lines.join("\n"));
});

test("npm writes its logs outside the cache of the tools", { skip: !posix }, () => {
  const lines = stepEnv("", { RUNNER_TEMP: "/runner/temp" });
  assert.ok(lines.includes("npm_config_logs_dir=/runner/temp/npm-logs"), lines.join("\n"));
});

test(
  "a job of every tool enables no list, and a pattern in the list stays as written",
  { skip: !posix },
  () => {
    const every = stepEnv("");
    assert.ok(!every.some((line) => line.startsWith("MISE_ENABLE_TOOLS=")), every.join("\n"));
    assert.ok(every.includes("MISE_DISABLE_TOOLS=npm:@anthropic-ai/claude-code"));
    assert.ok(stepEnv("node *").includes("MISE_ENABLE_TOOLS=node,*"));
    assert.ok(
      stepEnv("node", { MISE_DISABLE_TOOLS: "python" }).includes(
        "MISE_DISABLE_TOOLS=python,npm:@anthropic-ai/claude-code",
      ),
    );
  },
);

const hasMise = spawnSync("mise", ["--version"]).status === 0;

// Runs `mise install` of a cargo: tool in a project that pins sccache, with a data folder of
// its own and no network, and returns what mise printed.
function installCargoTool(env: Record<string, string>): string {
  const root = scratch();
  const project = join(root, "project");
  mkdirSync(project);
  writeFileSync(
    join(project, "mise.toml"),
    '[tools]\nsccache = "0.19.0"\n"cargo:ci-setup-tools-no-such-crate" = "0.1.0"\n',
  );
  const run = spawnSync("mise", ["install", "cargo:ci-setup-tools-no-such-crate"], {
    cwd: project,
    env: {
      ...process.env,
      MISE_DATA_DIR: join(root, "data"),
      MISE_CACHE_DIR: join(root, "cache"),
      MISE_STATE_DIR: join(root, "state"),
      MISE_CONFIG_DIR: join(root, "config"),
      MISE_GLOBAL_CONFIG_FILE: join(root, "config", "config.toml"),
      MISE_SYSTEM_CONFIG_DIR: join(root, "system"),
      MISE_CEILING_PATHS: root,
      MISE_TRUSTED_CONFIG_PATHS: project,
      MISE_OFFLINE: "1",
      MISE_YES: "1",
      // cargo, where the PATH has one, installs without the network and so fails.
      MISE_CARGO_BINSTALL: "false",
      CARGO_NET_OFFLINE: "true",
      ...env,
    },
    encoding: "utf8",
    timeout: 60_000,
  });
  return `${run.stdout}${run.stderr}`;
}

test(
  "mise installs a cargo: tool of the list without the sccache that the mise files pin",
  { skip: hasMise ? false : "no mise on the PATH" },
  () => {
    // Without the list, mise 2026.11.4 refused the install, as it did in tramlines' docs job,
    // which a later mise can stop doing, so the test checks only the list.
    // The list as the step writes it, where bash runs the step's script here.
    const written = posix
      ? stepEnv("cargo:ci-setup-tools-no-such-crate@0.1.0").find((line) =>
          line.startsWith("MISE_ENABLE_TOOLS="),
        )
      : "MISE_ENABLE_TOOLS=cargo:ci-setup-tools-no-such-crate";
    const list = (written ?? "").replace("MISE_ENABLE_TOOLS=", "");
    assert.equal(list, "cargo:ci-setup-tools-no-such-crate");
    const output = installCargoTool({ MISE_ENABLE_TOOLS: list });
    // mise got past the check of the dependencies to the install of the crate itself, which
    // fails without the network.
    assert.match(output, /cargo:ci-setup-tools-no-such-crate@0\.1\.0/u, output);
    assert.doesNotMatch(output, /requires configured install dependency/u, output);
  },
);

test("no step of the action deletes a tool that a job installed", () => {
  // The archive of the tools keeps every version that any branch installed, as a
  // developer's machine does, and its bound alone cleans it up.
  assert.doesNotMatch(readFileSync(action, "utf8"), /prune/u);
});
