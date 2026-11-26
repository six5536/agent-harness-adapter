// The archives of the pack ci-cache-s3, which restore a tree exactly as it was saved: the
// platform's own tar, compressed by zstd through a pipe that the runner's own Node.js
// runs, and on Windows the links, which the save records and the restore makes again.

import { spawn } from "node:child_process";
import {
  closeSync,
  existsSync,
  lstatSync,
  mkdirSync,
  openSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  readSync,
  rmSync,
  statSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { basename, join, win32 } from "node:path";
import type { Context, Env, Result } from "./cache-s3.mts";

// The environment of a program other than the AWS CLI and sccache's server, without the
// store's keys, because none of them needs the keys, and mise can run the project's code.
export function plainEnv(env: Env): Env {
  const keys = new Set([
    "CI_CACHE_S3_ACCESS_KEY_ID",
    "CI_CACHE_S3_SECRET_ACCESS_KEY",
    "INPUT_ACCESS-KEY-ID",
    "INPUT_SECRET-ACCESS-KEY",
  ]);
  return Object.fromEntries(Object.entries(env).filter(([name]) => !keys.has(name)));
}

// One program of a pipe.
export interface Step {
  command: string;
  args: string[];
  cwd?: string;
  env?: Env;
}

// Runs two programs with the output of the first as the input of the second, and returns
// the result of each, without its output.
export type Pipe = (from: Step, to: Step) => Promise<[Result, Result]>;

// Settles with the result of a program once it ends, or once it fails to start.
function ended(child: ReturnType<typeof spawn>, stderr: { text: string }): Promise<Result> {
  return new Promise((resolve) => {
    child.on("error", (error) => {
      resolve({ status: null, stdout: "", stderr: error.message });
    });
    child.on("close", (status) => {
      resolve({ status, stdout: "", stderr: stderr.text });
    });
  });
}

function started(step: Step, stdio: ["ignore" | "pipe", "ignore" | "pipe", "pipe"]) {
  const child = spawn(step.command, step.args, { cwd: step.cwd, env: step.env, stdio });
  const stderr = { text: "" };
  child.stderr?.setEncoding("utf8");
  child.stderr?.on("data", (chunk: string) => {
    stderr.text += chunk;
  });
  return { child, result: ended(child, stderr) };
}

// Pipes one program into another without a shell, through the runner's own Node.js, so
// that the pipe works alike on every platform.
export const runPipe: Pipe = (from, to) => {
  const second = started(to, ["pipe", "ignore", "pipe"]);
  const first = started(from, ["ignore", "pipe", "pipe"]);
  // The second program can end first, such as on an error, and then takes no more input.
  second.child.stdin?.on("error", () => {
    // The second program's own result tells why it took no more input.
  });
  if (first.child.stdout === null || second.child.stdin === null) {
    second.child.stdin?.end();
  } else {
    first.child.stdout.pipe(second.child.stdin);
  }
  // Once the second program exits, the first one would wait for ever on a full pipe that
  // nobody reads, so the pipe ends at the exit, not at the close of the second's streams,
  // which waits for the first. The pipe comes apart before the first's output resumes,
  // because Node.js pauses a stream that loses its last pipe, as it did on macOS in run
  // 45106982356. A second program that fails ends the first, and one that succeeds before it
  // reads all, as tar can stop at the end of an archive before its padding, lets the rest
  // drain, so that the first one ends as it would.
  second.child.on("exit", (status) => {
    const output = first.child.stdout;
    if (output !== null && second.child.stdin !== null) {
      output.unpipe(second.child.stdin);
    }
    second.child.stdin?.destroy();
    if (status === 0) {
      output?.resume();
    } else {
      output?.destroy();
    }
  });
  return Promise.all([first.result, second.result]);
};

// The platform's own tar, by its path, so that no other tar on the PATH, such as the GNU
// tar of Git for Windows, stands in for it: GNU tar on Linux, and bsdtar on macOS and on
// Windows.
export function tarProgram(platform: string, env: Env): string {
  return platform === "win32"
    ? win32.join(env.SystemRoot ?? env.windir ?? "C:\\Windows", "System32", "tar.exe")
    : "/usr/bin/tar";
}

// Whether the archives of a platform are known to keep every entry exactly, junctions and
// links that lead nowhere among them, as the round trip of archive.test.mts shows in a full
// run of CI on the platform. On Windows, the save records the links itself and the restore
// makes them again. A platform that no run has shown saves no archive, and a later job
// installs the cache's contents cold.
export function archivesExact(platform: string): boolean {
  // Run 45090238477 passed the round trip and the save that fails midway on Windows, with
  // its bsdtar 3.8.9.
  return ["linux", "darwin", "win32"].includes(platform);
}

// The file at the root of an archive of Windows that holds the archive's links.
export const linksFile = ".ci-cache-s3-links.json";

// A link of a tree on Windows: a junction, or a symbolic link to a folder or to a file,
// which Windows tells apart, and which a link that leads nowhere keeps too.
export type LinkKind = "junction" | "dir" | "file";

export interface Link {
  path: string;
  target: string;
  kind: LinkKind;
}

// The entries under the paths, from `base`, without following any link, each as its path
// from `base`, in the order of a walk that sorts each folder: the links, and every other
// entry, each folder before what it holds.
export function walkTree(base: string, paths: string[]): { entries: string[]; links: string[] } {
  const entries: string[] = [];
  const links: string[] = [];
  const visit = (path: string): void => {
    const stat = lstatSync(join(base, path));
    if (stat.isSymbolicLink()) {
      links.push(path);
      return;
    }
    entries.push(path);
    if (!stat.isDirectory()) {
      return;
    }
    const names = readdirSync(join(base, path)).toSorted((a, b) => (a < b ? -1 : Number(a > b)));
    for (const name of names) {
      visit(`${path}/${name}`);
    }
  };
  for (const path of paths) {
    visit(path);
  }
  return { entries, links };
}

// The members of a cache that exist, from `base`, a link that leads nowhere among them.
export function present(base: string, members: string[]): string[] {
  return members.filter((member) => {
    try {
      lstatSync(join(base, member));
      return true;
    } catch {
      return false;
    }
  });
}

// An entry of a manifest: its path and type, the mode of a file or a folder, a file's size
// and time of change, and a link's target.
interface Listed {
  path: string;
  type: "file" | "folder" | "link";
  mode?: number;
  size?: number;
  mtime?: number;
  target?: string;
}

// A manifest of the entries under the paths, from `base`, without following a link, one
// line of JSON for each in the order of the walk, so that any name fits, without each
// member that `unwatched` names, so that two manifests differ only where the tree changed.
// A folder's own time of change isn't in it, because a file made and deleted inside, such
// as a lock, changes it.
export function treeManifest(base: string, paths: string[], unwatched: string[] = []): string {
  const lines: string[] = [];
  const visit = (path: string): void => {
    if (unwatched.includes(path)) {
      return;
    }
    const full = join(base, path);
    const stat = lstatSync(full);
    let entry: Listed;
    if (stat.isSymbolicLink()) {
      entry = { path, type: "link", target: readlinkSync(full) };
    } else if (stat.isDirectory()) {
      entry = { path, type: "folder", mode: stat.mode };
    } else {
      entry = {
        path,
        type: "file",
        mode: stat.mode,
        size: stat.size,
        mtime: Math.trunc(stat.mtimeMs),
      };
    }
    lines.push(JSON.stringify(entry));
    if (entry.type === "folder") {
      const names = readdirSync(full).toSorted((a, b) => (a < b ? -1 : Number(a > b)));
      for (const name of names) {
        visit(`${path}/${name}`);
      }
    }
  };
  for (const path of paths) {
    visit(path);
  }
  return lines.map((line) => `${line}\n`).join("");
}

function listed(manifest: string): Map<string, Listed> {
  const map = new Map<string, Listed>();
  for (const line of manifest.split("\n")) {
    if (line === "") {
      continue;
    }
    const value: unknown = JSON.parse(line);
    if (typeof value !== "object" || value === null || !("path" in value)) {
      continue;
    }
    const field = (name: string): unknown => (name in value ? Reflect.get(value, name) : undefined);
    const number = (name: string): number | undefined => {
      const found = field(name);
      return typeof found === "number" ? found : undefined;
    };
    const type = field("type");
    const target = field("target");
    map.set(String(value.path), {
      path: String(value.path),
      type: type === "file" || type === "link" ? type : "folder",
      mode: number("mode"),
      size: number("size"),
      mtime: number("mtime"),
      target: typeof target === "string" ? target : undefined,
    });
  }
  return map;
}

// How a tree changed between two manifests, as make and rsync tell it: the first entry that
// changed for certain, in the order of the walk, where an entry is new or gone, or its
// type, mode, size, or link target differs, and else each file of the same size whose time of
// change differs, whose bytes alone can tell.
export function compareManifests(
  before: string,
  after: string,
): { change?: string; unsure: string[] } {
  const old = listed(before);
  const now = listed(after);
  const unsure: string[] = [];
  for (const [path, entry] of now) {
    const was = old.get(path);
    if (
      was?.type !== entry.type ||
      was.mode !== entry.mode ||
      was.size !== entry.size ||
      was.target !== entry.target
    ) {
      return { change: path, unsure };
    }
    if (was.mtime !== entry.mtime) {
      unsure.push(path);
    }
  }
  const gone = [...old.keys()].find((path) => !now.has(path));
  return gone === undefined ? { unsure } : { change: gone, unsure };
}

// Whether two files hold the same bytes, read a part at a time, so that a file of any size
// fits.
function sameBytes(first: string, second: string): boolean {
  if (statSync(first).size !== statSync(second).size) {
    return false;
  }
  const one = openSync(first, "r");
  const two = openSync(second, "r");
  try {
    const left = Buffer.alloc(1 << 20);
    const right = Buffer.alloc(1 << 20);
    for (;;) {
      const read = readSync(one, left, 0, left.length, null);
      const other = readSync(two, right, 0, right.length, null);
      if (read !== other || !left.subarray(0, read).equals(right.subarray(0, other))) {
        return false;
      }
      if (read === 0) {
        return true;
      }
    }
  } finally {
    closeSync(one);
    closeSync(two);
  }
}

// The first of the files that differs in its bytes from the same file of the archive that
// the restore extracted, or undefined where each is the same, which extracts only those
// files of the archive into a folder of its own, in one pass of the archive.
export async function firstChangedFile(
  context: Context,
  base: string,
  archive: string,
  files: string[],
): Promise<string | undefined> {
  if (files.length === 0) {
    return undefined;
  }
  const cwd = join(context.temp, "ci-cache-s3");
  const folder = join(cwd, `${basename(archive)}.compare`);
  rmSync(folder, { recursive: true, force: true });
  mkdirSync(folder, { recursive: true });
  const list = join(cwd, `${basename(archive)}.compare.list`);
  writeFileSync(list, files.map((file) => `${file}\0`).join(""));
  const env = plainEnv(context.env);
  const steps: [Step, Step] = [
    { command: "zstd", args: ["-d", "-q", "-c", archive], cwd, env },
    {
      command: tarProgram(context.platform, context.env),
      args: ["-xf", "-", "-C", folder, "--null", "-T", list],
      cwd,
      env,
    },
  ];
  const failed = failure(steps, await context.pipe(...steps));
  try {
    if (failed !== undefined) {
      return files[0];
    }
    return files.find((file) => {
      const old = join(folder, file);
      return !existsSync(old) || !sameBytes(old, join(base, file));
    });
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
}

// The PowerShell of Windows, which tells a junction from a symbolic link, and a link to a
// folder from one to a file, where Node.js reports each as a symbolic link. It reads each
// link as a FileInfo of .NET, because Get-Item finds no item at a link that leads nowhere.
const kindScript = `param([string]$List)
$ErrorActionPreference = 'Stop'
$paths = Get-Content -LiteralPath $List -Raw -Encoding UTF8 | ConvertFrom-Json
$kinds = foreach ($path in $paths) {
  $item = [System.IO.FileInfo]::new($path)
  if ($item.LinkType -eq 'Junction') { 'junction' }
  elseif ($item.LinkType -eq 'SymbolicLink') {
    if ($item.Attributes -band [IO.FileAttributes]::Directory) { 'dir' } else { 'file' }
  }
  else { 'other ' + $item.LinkType }
}
ConvertTo-Json -InputObject @($kinds) -Compress
`;

// The kind of each link of a tree on Windows, by its full path, which throws where
// PowerShell fails or names a kind of link that the restore can't make again.
export function linkKinds(fullPaths: string[], context: Context): LinkKind[] {
  if (fullPaths.length === 0) {
    return [];
  }
  const folder = join(context.temp, "ci-cache-s3");
  mkdirSync(folder, { recursive: true });
  const script = join(folder, "link-kinds.ps1");
  const list = join(folder, "link-kinds.json");
  writeFileSync(script, kindScript);
  writeFileSync(list, JSON.stringify(fullPaths));
  const powershell = win32.join(
    context.env.SystemRoot ?? context.env.windir ?? "C:\\Windows",
    "System32",
    "WindowsPowerShell",
    "v1.0",
    "powershell.exe",
  );
  const run = context.run(
    powershell,
    ["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File", script, list],
    { cwd: folder, env: plainEnv(context.env) },
  );
  if (run.status !== 0) {
    throw new Error(`PowerShell couldn't tell the kinds of the links: ${run.stderr.trim()}`);
  }
  const kinds: unknown = JSON.parse(run.stdout);
  if (!Array.isArray(kinds) || kinds.length !== fullPaths.length) {
    throw new Error(`PowerShell told the kinds of the links as ${run.stdout.trim()}`);
  }
  return kinds.map((kind: unknown, index) => {
    if (kind === "junction" || kind === "dir" || kind === "file") {
      return kind;
    }
    throw new Error(`${fullPaths[index] ?? ""} is a link of the kind ${String(kind)}`);
  });
}

// Makes each link again, and returns the error of each that it couldn't make.
function makeLinks(base: string, links: Link[]): string[] {
  return links.flatMap((link) => {
    try {
      // A link already at the path, such as from an earlier restore of the job, gives way,
      // as tar replaces its own entries.
      const path = join(base, link.path);
      if (present(base, [link.path]).length > 0 && lstatSync(path).isSymbolicLink()) {
        unlinkSync(path);
      }
      symlinkSync(link.target, path, link.kind);
      return [];
    } catch (error) {
      return [`${link.path}: ${error instanceof Error ? error.message : String(error)}`];
    }
  });
}

function isLink(value: unknown): value is Link {
  return (
    typeof value === "object" &&
    value !== null &&
    "path" in value &&
    typeof value.path === "string" &&
    "target" in value &&
    typeof value.target === "string" &&
    "kind" in value &&
    (value.kind === "junction" || value.kind === "dir" || value.kind === "file")
  );
}

// Writes an archive of the paths, from `base`, that restores them exactly, and returns the
// error that kept it from doing so, or undefined. It never changes the tree. The platform's
// tar archives each file, folder, and link as it is on Linux and macOS. On Windows, its
// bsdtar follows a junction as a folder and fails at one that leads nowhere, so the save
// hands it a list of every entry but the links, with recursion off, so that it neither
// walks into a junction nor reads a link, and records the links in a file of the archive.
export async function writeArchive(
  context: Context,
  base: string,
  paths: string[],
  archive: string,
): Promise<string | undefined> {
  const cwd = join(context.temp, "ci-cache-s3");
  mkdirSync(cwd, { recursive: true });
  const env = plainEnv(context.env);
  const tar = tarProgram(context.platform, context.env);
  const zstd: Step = { command: "zstd", args: [...zstdCompressArgs, archive], cwd, env };
  if (context.platform !== "win32") {
    const steps: [Step, Step] = [
      { command: tar, args: ["-cf", "-", "-C", base, ...paths], cwd, env },
      zstd,
    ];
    return failure(steps, await context.pipe(...steps));
  }
  const { entries, links: found } = walkTree(base, paths);
  // bsdtar reads its list in the code page of the system, not in UTF-8.
  const unlisted = [...entries, ...found].find((path) => !/^[ -~]*$/u.test(path));
  if (unlisted !== undefined) {
    return `${unlisted} has a name outside printable ASCII, which bsdtar can't read from its list`;
  }
  const kinds = linkKinds(
    found.map((path) => join(base, path)),
    context,
  );
  const links = found.map((path, index): Link => ({
    path,
    target: readlinkSync(join(base, path)),
    kind: kinds[index] ?? "file",
  }));
  const record = join(cwd, `${basename(archive)}.links`);
  rmSync(record, { recursive: true, force: true });
  mkdirSync(record);
  writeFileSync(join(record, linksFile), `${JSON.stringify(links)}\n`);
  // A line -C of the list changes the folder of the lines after it, which bsdtar reads only
  // from a list of lines, not from one that --null separates, and no name on Windows holds
  // a line break.
  const list = join(cwd, `${basename(archive)}.list`);
  writeFileSync(list, ["-C", base, ...entries, "-C", record, linksFile, ""].join("\n"));
  const steps: [Step, Step] = [
    { command: tar, args: ["-c", "-n", "-f", "-", "-T", list], cwd, env },
    zstd,
  ];
  return failure(steps, await context.pipe(...steps));
}

// Extracts an archive into `base` with each mode as it was saved, makes again each link
// that an archive of Windows recorded, and returns the error that kept it from doing so,
// or undefined.
export async function readArchive(
  context: Context,
  base: string,
  archive: string,
): Promise<string | undefined> {
  const cwd = join(context.temp, "ci-cache-s3");
  const env = plainEnv(context.env);
  const steps: [Step, Step] = [
    { command: "zstd", args: ["-d", "-q", "-c", archive], cwd, env },
    {
      command: tarProgram(context.platform, context.env),
      args: ["-xpf", "-", "-C", base],
      cwd,
      env,
    },
  ];
  const failed = failure(steps, await context.pipe(...steps));
  const record = join(base, linksFile);
  if (failed !== undefined) {
    // The record of the links comes last in the archive, so a late failure leaves it.
    rmSync(record, { force: true });
    return failed;
  }
  if (!existsSync(record)) {
    return undefined;
  }
  let links: unknown;
  try {
    links = JSON.parse(readFileSync(record, "utf8"));
  } catch (error) {
    rmSync(record, { force: true });
    return `${linksFile} isn't JSON: ${error instanceof Error ? error.message : String(error)}`;
  }
  rmSync(record);
  if (!Array.isArray(links) || !links.every((link) => isLink(link))) {
    return `${linksFile} holds no list of links`;
  }
  const broken = makeLinks(base, links);
  return broken.length === 0 ? undefined : `the links couldn't be made: ${broken.join("; ")}`;
}

// zstd compresses on every core at a low level, which favours the time of a job over the
// size of the archive.
export const zstdCompressArgs = ["-T0", "-3", "-q", "-f", "-o"];

// The errors of the programs of a pipe that failed, each after its name, or undefined where
// both succeeded. Both count, because the first program fails too where the second ends
// early, such as a zstd that the runner lacks.
export function failure(steps: [Step, Step], results: [Result, Result]): string | undefined {
  const errors = results.flatMap((result, index) =>
    result.status === 0
      ? []
      : [
          `${steps[index]?.command ?? ""}: ${result.stderr.trim() || `exit ${String(result.status)}`}`,
        ],
  );
  return errors.length === 0 ? undefined : errors.join("; ");
}
