// Formats the shell scripts of the project with shfmt: `write` rewrites them, as
// `format:write:shell` does, and `check` prints the diff of each that shfmt would
// change and fails, as `format:check:shell` does. It reads only the scripts that git
// tracks or would add, as lib/shell-files.mts lists them, so that a file that git
// ignores stays as it is. It runs shfmt only when there is a script, because shfmt
// without a file reads its standard input, and passes the paths as arguments after
// `--`, so that a path that starts with `-` is no option, in batches that fit on any
// platform's command line.
//
// Usage: format-shell.mts write|check

import process from "node:process";
import { MAX_CHARS, isRegularFile, runOnShellFiles, spawnRunner } from "./lib/shell-files.mts";
import type { Runner } from "./lib/shell-files.mts";

export type Mode = "write" | "check";

const FLAGS: Record<Mode, string> = { write: "-w", check: "-d" };

// Lists the shell scripts, and runs shfmt on each batch of them, so that it formats
// or reports every script. It returns the status of the listing when the listing
// fails, and otherwise the status of the first run of shfmt that fails, or 0.
export function formatShell(
  run: Runner,
  mode: Mode,
  maxChars: number = MAX_CHARS,
  isFile: (path: string) => boolean = isRegularFile,
): number {
  return runOnShellFiles(run, "shfmt", [FLAGS[mode]], maxChars, isFile);
}

if (import.meta.main) {
  const mode = process.argv[2];
  if (process.argv.length !== 3 || (mode !== "write" && mode !== "check")) {
    console.error("Usage: format-shell.mts write|check");
    process.exit(2);
  }
  process.exitCode = formatShell(spawnRunner, mode);
}
