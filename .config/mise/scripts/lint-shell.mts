// Lints the shell scripts of the project with shellcheck. It reads only the scripts
// that git tracks or would add, as lib/shell-files.mts lists them, and fails when the
// listing fails. It runs shellcheck only when there is a script, because shellcheck
// without a file fails, and passes the paths as arguments after `--`, so that a path
// that starts with `-` is no option, in batches that fit on any platform's command
// line.
//
// Usage: lint-shell.mts

import process from "node:process";
import { MAX_CHARS, isRegularFile, runOnShellFiles, spawnRunner } from "./lib/shell-files.mts";
import type { Runner } from "./lib/shell-files.mts";

// Lists the shell scripts, and runs shellcheck on each batch of them, so that it
// reports every finding. It returns the status of the listing when the listing fails,
// and otherwise the status of the first run of shellcheck that fails, or 0.
export function lintShell(
  run: Runner,
  maxChars: number = MAX_CHARS,
  isFile: (path: string) => boolean = isRegularFile,
): number {
  return runOnShellFiles(run, "shellcheck", [], maxChars, isFile);
}

if (import.meta.main) {
  process.exitCode = lintShell(spawnRunner);
}
