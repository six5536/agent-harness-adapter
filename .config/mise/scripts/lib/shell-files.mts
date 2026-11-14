// Lists the shell scripts of the project for format-shell.mts and lint-shell.mts: the
// files that git tracks or would add, which shfmt finds to be shell scripts. git lists
// the files, so that a file that git ignores, such as a clone in an ignored folder,
// stays out, and `shfmt -f` keeps the shell scripts among them, by their extension or
// their shebang, and leaves out each whose path a section of EditorConfig with
// `ignore = true` matches, such as `[vendor/**]`. A section that names only a folder,
// such as `[vendor]`, matches no path that shfmt gets, as it matches a folder only in
// shfmt's walk.

import { spawnSync } from "node:child_process";
import { lstatSync } from "node:fs";

// The longest list of paths in one run of a tool: half of the 32,767 characters of a
// Windows command line, which leaves room for the path of the tool and the quotes
// around each path that holds a space.
export const MAX_CHARS = 16_000;

export interface Outcome {
  status: number;
  stdout: string;
}

// Runs `command` with `args`, and returns its exit status and, when `capture` is
// true, its standard output.
export type Runner = (command: string, args: string[], capture: boolean) => Outcome;

export interface Listing {
  status: number;
  files: string[];
}

// The lines of `output`, without the empty ones. A line can hold spaces.
export function lines(output: string): string[] {
  return output.split(/\r?\n/u).filter((line) => line !== "");
}

// The fields of git's output with -z, without the empty field after the last NUL.
export function nulFields(output: string): string[] {
  return output.split("\0").filter((field) => field !== "");
}

// `files` split into batches whose paths add up to at most `maxChars` characters, with
// a space between each pair. A path longer than `maxChars` is a batch of its own.
export function batches(files: string[], maxChars: number = MAX_CHARS): string[][] {
  const result: string[][] = [];
  let batch: string[] = [];
  let chars = 0;
  for (const file of files) {
    const added = batch.length === 0 ? file.length : chars + 1 + file.length;
    if (batch.length > 0 && added > maxChars) {
      result.push(batch);
      batch = [file];
      chars = file.length;
    } else {
      batch.push(file);
      chars = added;
    }
  }
  if (batch.length > 0) {
    result.push(batch);
  }
  return result;
}

// Whether `path` is a regular file. A symbolic link isn't, as shfmt's walk skips it,
// and neither is a folder, such as a submodule, a nested repository, or a tracked file
// that the working tree has deleted.
export function isRegularFile(path: string): boolean {
  try {
    return lstatSync(path).isFile();
  } catch {
    return false;
  }
}

// The shell scripts below the current folder that git tracks or would add, with a
// failing status where git or a run of shfmt fails. A run of shfmt that fails, such as
// on a file that it can't read, still gives the scripts that it found, and the other
// batches still run, so that the task reads every other script. `isFile` keeps the
// paths that shfmt reads, because shfmt fails on a path that doesn't exist and walks a
// folder.
export function listShellFiles(
  run: Runner,
  maxChars: number = MAX_CHARS,
  isFile: (path: string) => boolean = isRegularFile,
): Listing {
  const listed = run("git", ["ls-files", "-z", "--cached", "--others", "--exclude-standard"], true);
  if (listed.status !== 0) {
    console.error(
      "The shell tasks list their files with `git ls-files`, which failed, so they run only in a git repository that git reads.",
    );
    return { status: listed.status, files: [] };
  }
  // The index lists a file of a merge conflict once for each of its stages.
  const paths = [...new Set(nulFields(listed.stdout))].filter((path) => {
    // shfmt prints one path on each line, so a name with a line break would come back
    // as other paths, and a name that isn't UTF-8 reads as another name, with U+FFFD.
    if (/[\r\n\uFFFD]/u.test(path)) {
      console.error(
        `The shell tasks leave out ${JSON.stringify(path)}, whose name holds a line break or isn't UTF-8.`,
      );
      return false;
    }
    return isFile(path);
  });
  const files: string[] = [];
  let status = 0;
  for (const batch of batches(paths, maxChars)) {
    const found = run("shfmt", ["-f", "--apply-ignore", "--", ...batch], true);
    if (status === 0) {
      status = found.status;
    }
    files.push(...lines(found.stdout));
  }
  return { status, files };
}

// Lists the shell scripts, and runs `command` with `flags` on each batch of them,
// after `--`, so that it reads every script that the listing found. It returns the
// status of the listing when the listing fails, and otherwise the status of the first
// run of `command` that fails, or 0. It runs `command` only when there is a script.
export function runOnShellFiles(
  run: Runner,
  command: string,
  flags: string[],
  maxChars: number = MAX_CHARS,
  isFile: (path: string) => boolean = isRegularFile,
): number {
  const listed = listShellFiles(run, maxChars, isFile);
  let status = listed.status;
  for (const batch of batches(listed.files, maxChars)) {
    const outcome = run(command, [...flags, "--", ...batch], false);
    if (status === 0) {
      status = outcome.status;
    }
  }
  return status;
}

// Runs a command without a shell, so that no path is split or expanded. A command that
// doesn't start, or that a signal ends, has the status 1.
export function spawnRunner(command: string, args: string[], capture: boolean): Outcome {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    // git lists every file of a large repository.
    maxBuffer: 2 ** 30,
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error !== undefined) {
    console.error(`${command}: ${result.error.message}`);
    return { status: 1, stdout: "" };
  }
  // Node.js gives no output, null, for a stream that the command inherits.
  return { status: result.status ?? 1, stdout: capture ? result.stdout : "" };
}
