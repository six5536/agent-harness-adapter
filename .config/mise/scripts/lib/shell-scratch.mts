// A scratch git repository in a temporary folder for the tests of the shell scripts,
// format-shell.mts and lint-shell.mts, which run the real git, shfmt, and shellcheck.

import { spawnSync } from "node:child_process";
import type { SpawnSyncReturns } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import process from "node:process";

// The environment of a scratch repository below `base`: without the variables that a
// commit hook exports, such as the GIT_DIR of a linked worktree, which would point
// git at the repository of the hook, and without the machine's git settings, such as
// a global `core.excludesFile` that ignores `bin/`, so that no setting of the machine
// changes what git lists. Git looks for a repository no higher than `base`, so that a
// scratch folder without git is outside every repository.
function scratchEnv(base: string): NodeJS.ProcessEnv {
  const config = join(base, "gitconfig");
  writeFileSync(config, "");
  return {
    ...Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("GIT_"))),
    GIT_CONFIG_GLOBAL: config,
    GIT_CONFIG_NOSYSTEM: "1",
    XDG_CONFIG_HOME: base,
    GIT_CEILING_DIRECTORIES: base,
  };
}

// Runs `body` in a new folder that holds `files`, each path relative to the folder,
// an empty .shellcheckrc, which shellcheck reads in place of any in a folder above or
// in the home folder, and an .editorconfig with `root = true`, which keeps shfmt from
// reading one in a folder above, so that no setting of the machine changes a finding
// or a format. A test's own .editorconfig starts with `root = true` too. The
// folder is a git repository with no commit, unless `git` is false. `body` gets the
// folder and the environment to run git and the scripts in.
export function inScratch<T>(
  files: Record<string, string>,
  body: (dir: string, env: NodeJS.ProcessEnv) => T,
  git: boolean = true,
): T {
  const base = realpathSync(mkdtempSync(join(tmpdir(), "shell-scratch-")));
  try {
    const env = scratchEnv(base);
    const dir = join(base, "repo");
    mkdirSync(dir);
    if (git) {
      const init = spawnSync("git", ["init", "-q"], { cwd: dir, env, encoding: "utf8" });
      if (init.status !== 0) {
        throw new Error(`git init failed: ${init.stderr}`);
      }
    }
    for (const [name, text] of Object.entries({
      ".shellcheckrc": "",
      ".editorconfig": "root = true\n",
      ...files,
    })) {
      mkdirSync(dirname(join(dir, name)), { recursive: true });
      writeFileSync(join(dir, name), text);
    }
    return body(dir, env);
  } finally {
    rmSync(base, { recursive: true, force: true });
  }
}

// Runs the script `script` with Node.js in `dir`, with `args` and `env`.
export function runScript(
  script: string,
  args: string[],
  dir: string,
  env: NodeJS.ProcessEnv,
): SpawnSyncReturns<string> {
  return spawnSync(process.execPath, [script, ...args], { cwd: dir, env, encoding: "utf8" });
}
