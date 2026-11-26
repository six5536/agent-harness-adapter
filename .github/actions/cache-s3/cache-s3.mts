// The code of the actions cache-restore and cache-save of the pack ci-cache-s3, which
// keep CI's caches in an S3 store of any provider: the tools of ~/.tools, cargo's
// downloads, the caches that .config/ci-cache-s3/caches.project.json lists, and, through
// sccache's S3 backend, the Rust build. The runner's own Node.js runs it, because the
// restore runs before mise installs Node.js. `restore` runs before mise installs the
// tools and, with the phase `build`, after it; `save` runs after the job's task. Each
// archive is a tar of the platform's own tar, compressed by zstd through a pipe. A run
// without the store's settings uses no store, and an error of the store only warns.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { type Bound, describe, limitOf, readBound } from "./bound.mts";
import { awsEnv, copy, headObject, isRecord, savedSince, type Stored, upload } from "./store.mts";
import {
  archivesExact,
  compareManifests,
  firstChangedFile,
  plainEnv,
  present,
  treeManifest,
  type Pipe,
  readArchive,
  runPipe,
  writeArchive,
} from "./archive.mts";

export { plainEnv } from "./archive.mts";

// The project's own caches, from the root of the repository.
export const listPath = ".config/ci-cache-s3/caches.project.json";

// The names that the pack's own caches take, which the project's list can't take.
const reservedNames = /^(?:tools(?:-.*)?|cargo|sccache)$/u;

export type Env = Record<string, string | undefined>;

export interface Settings {
  endpoint: string;
  bucket: string;
  region: string;
  accessKeyId: string;
  secretAccessKey: string;
}

export interface Cache {
  name: string;
  // Each path starts with `~/`, from the home folder, or none does, from the root of the
  // repository.
  paths: string[];
  // The members under the paths whose changes don't count, such as cargo's record of its
  // uses of the downloads, which each build writes.
  unwatched?: string[];
}

// A cache that the restore left, which the save writes where it changed.
export interface Restored {
  name: string;
  key: string;
  paths: string[];
  unwatched: string[];
  // The file of the manifest of the paths that the restore left.
  manifest: string;
  // True where the store holds the key but the restore couldn't use it, so that the save
  // replaces it whether or not the paths changed.
  replace: boolean;
  // The size of the archive that the save after a cold start made, which the restored
  // archive carries, or absent where the restore found no archive to build on.
  baseline?: number;
  // True where the restore couldn't read the store, so that the save writes nothing over
  // an archive that holds what other branches installed.
  unreadable: boolean;
  // Whether the store held the key at the restore, and its ETag and time of writing, which
  // the restore read before its download, so that the save tells another job's archive
  // from the one that it restored.
  found: boolean;
  etag?: string;
  modified?: number;
}

export interface State {
  caches: Restored[];
  // The path of the sccache that the job's builds run through, or "" where they don't.
  sccache: string;
}

export interface Result {
  status: number | null;
  stdout: string;
  stderr: string;
}

export type Run = (
  command: string,
  args: string[],
  // `detached` runs a program that leaves a process behind, such as sccache's server,
  // with no pipes, which the process would otherwise hold open, so that the call returns.
  options: { cwd?: string; env?: Env; detached?: boolean },
) => Result;

export interface Context {
  env: Env;
  home: string;
  workspace: string;
  temp: string;
  // The platform of the runner, as process.platform names it, which picks its tar.
  platform: string;
  run: Run;
  pipe: Pipe;
  log: (line: string) => void;
  // Waits before another try of an upload, for the given milliseconds.
  wait?: (milliseconds: number) => Promise<void>;
}

// Runs a program without a shell, which would split its paths at a blank.
export const runCommand: Run = (command, args, options) => {
  if (options.detached === true) {
    const started = spawnSync(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: "ignore",
    });
    return { status: started.status, stdout: "", stderr: started.error?.message ?? "" };
  }
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env,
    encoding: "utf8",
  });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.error === undefined ? result.stderr : result.error.message,
  };
};

// Returns the store's settings, or the names of the settings that are empty. The region
// defaults to `auto`, which a provider without regions takes.
export function readSettings(env: Env): { settings: Settings } | { missing: string[] } {
  const names = {
    endpoint: "CI_CACHE_S3_ENDPOINT",
    bucket: "CI_CACHE_S3_BUCKET",
    accessKeyId: "CI_CACHE_S3_ACCESS_KEY_ID",
    secretAccessKey: "CI_CACHE_S3_SECRET_ACCESS_KEY",
  };
  const missing = Object.values(names).filter((name) => (env[name] ?? "").trim() === "");
  if (missing.length > 0) {
    return { missing };
  }
  const value = (name: string): string => (env[name] ?? "").trim();
  return {
    settings: {
      endpoint: value(names.endpoint),
      bucket: value(names.bucket),
      region: value("CI_CACHE_S3_REGION") === "" ? "auto" : value("CI_CACHE_S3_REGION"),
      accessKeyId: value(names.accessKeyId),
      secretAccessKey: value(names.secretAccessKey),
    },
  };
}

// The scope of a run: `release` on a release tag, and `dev` on every other ref.
export function scopeOf(ref: string): "release" | "dev" {
  return ref.startsWith("refs/tags/v") ? "release" : "dev";
}

// The start of every key of the run: its scope, the repository, and the platform, such as
// `dev/owner/repo/Linux-X64/`.
export function prefixOf(env: Env): string {
  const repository = env.GITHUB_REPOSITORY ?? "";
  const platform = `${env.RUNNER_OS ?? ""}-${env.RUNNER_ARCH ?? ""}`;
  const parts = repository.split("/");
  const named =
    parts.length === 2 && parts.every((part) => /^[\w.-]+$/u.test(part) && !/^\.\.?$/u.test(part));
  if (!named || !/^\w+-\w+$/u.test(platform)) {
    throw new Error(
      "GITHUB_REPOSITORY, RUNNER_OS, and RUNNER_ARCH must name a repository and a platform",
    );
  }
  return `${scopeOf(env.GITHUB_REF ?? "")}/${repository}/${platform}/`;
}

// The format of the archives, in every key of an archive, so that no archive of an earlier
// format restores: a change to how the save writes an archive, or to what the restore
// expects of one, takes the next version. v2 restores each tree exactly, links that lead
// nowhere and Windows' junctions among them, where v1 left out what tar couldn't read, and
// v3 is the one archive of each cache, which a job restores whatever it holds, where v2
// keyed each archive by a hash of the files that chose its contents.
export const archiveFormat = "v3";

// The key of a cache's archive, a tar that zstd compresses: one for each scope, platform,
// and name, with no hash of any file, so that a job restores what the last save left and
// mise and cargo bring it up to date.
export function archiveKey(prefix: string, name: string): string {
  return `${prefix}${name}/${archiveFormat}/current.tar.zst`;
}

// The name of the tools cache for the tools that setup-tools installs: `tools` for every
// tool, and a name of its own for each smaller set, so that the archive of a set never
// stands in for every tool.
export function toolsName(tools: string): string {
  const words = tools
    .trim()
    .split(/\s+/u)
    .filter((word) => word !== "")
    .toSorted();
  if (words.length === 0) {
    return "tools";
  }
  // The readable part can be the same for two sets, such as cargo:foo and cargo-foo, so a
  // hash of the exact set follows it.
  const readable = words
    .join("-")
    .toLowerCase()
    .replaceAll(/[^a-z0-9.-]+/gu, "-");
  const exact = createHash("sha256").update(words.join(" ")).digest("hex").slice(0, 8);
  return `tools-${readable}-${exact}`;
}

// The pack's own caches: the tools, and cargo's downloads in a job that installs every
// tool, of a project with a Cargo.lock. A job of a smaller set, such as the one that lists
// the jobs, runs no build, so it would save an empty archive under the key of the downloads.
export function builtinCaches(tools: string, hasCargoLock: boolean): Cache[] {
  const caches: Cache[] = [{ name: toolsName(tools), paths: ["~/.tools"] }];
  if (hasCargoLock && tools.trim() === "") {
    // Cargo deletes on its own each download that it hasn't used for months, by the record
    // of its uses in .global-cache, which counts a download that it lacks as used now.
    caches.push({
      name: "cargo",
      paths: ["~/.cargo/registry", "~/.cargo/git", "~/.cargo/.global-cache"],
      unwatched: [".cargo/.global-cache"],
    });
  }
  return caches;
}

function isStringList(value: unknown): value is string[] {
  return (
    Array.isArray(value) &&
    value.length > 0 &&
    value.every((item) => typeof item === "string" && item !== "")
  );
}

// Reads the project's list of caches, and throws an error that names the file where it
// isn't a list of caches with a name and paths.
export function parseCaches(text: string, file = listPath): Cache[] {
  const fail = (why: string): never => {
    throw new Error(`${file}: ${why}`);
  };
  let list: unknown;
  try {
    list = JSON.parse(text);
  } catch (error) {
    return fail(`not JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (!Array.isArray(list)) {
    return fail("not a list of caches");
  }
  const names = new Set<string>();
  return list.map((item: unknown, index): Cache => {
    const where = `cache ${String(index + 1)}`;
    if (!isRecord(item)) {
      return fail(`${where} is not an object with name and paths`);
    }
    // A cache is one archive, which no file keys, so a list that names its key files would
    // promise what the pack doesn't do.
    if ("keyFiles" in item) {
      return fail(
        `${where} has keyFiles, which the pack no longer reads, because each cache is one archive that the save writes again where it changed; remove it`,
      );
    }
    const { name, paths } = item;
    if (typeof name !== "string" || !/^[a-z0-9][a-z0-9.-]*$/u.test(name)) {
      return fail(`${where} needs a name of lowercase letters, digits, dots, and hyphens`);
    }
    if (reservedNames.test(name)) {
      return fail(`${where} takes the name ${name}, which the pack's own caches use`);
    }
    if (names.has(name)) {
      return fail(`${where} takes the name ${name} a second time`);
    }
    names.add(name);
    if (!isStringList(paths)) {
      return fail(`${where}, ${name}, needs paths, a list of paths`);
    }
    for (const path of paths) {
      const relative = path.startsWith("~/") ? path.slice(2) : path;
      const parts = relative.split(/[/\\]/u);
      // Each part is a name, so that the path names a folder or a file below the base, as
      // tar stores it, and never the base itself.
      if (
        /^(?:[/\\]|[A-Za-z]:)/u.test(relative) ||
        parts.some((part) => part === "" || part === "." || part === "..") ||
        relative.startsWith("-")
      ) {
        fail(
          `${where}, ${name}, has the path ${path}, which must name a folder or a file inside the home folder or the repository, without . or .. or an empty part, and not start with -`,
        );
      }
    }
    if (new Set(paths.map((path) => path.startsWith("~/"))).size > 1) {
      fail(`${where}, ${name}, mixes paths from the home folder with paths from the repository`);
    }
    return { name, paths };
  });
}

// The folder that a cache's paths start from, and each path from it.
function located(
  cache: { paths: string[] },
  context: Context,
): { base: string; members: string[] } {
  const fromHome = cache.paths[0]?.startsWith("~/") ?? false;
  return {
    base: fromHome ? context.home : context.workspace,
    members: cache.paths.map((path) => (fromHome ? path.slice(2) : path)),
  };
}

function stateFile(context: Context): string {
  return join(context.temp, "ci-cache-s3", "state.json");
}

function isState(value: unknown): value is State {
  return isRecord(value) && Array.isArray(value.caches) && typeof value.sccache === "string";
}

// The state that the restore left for the later phases, or undefined where it left none.
function readState(context: Context): State | undefined {
  const file = stateFile(context);
  if (!existsSync(file)) {
    return undefined;
  }
  const state: unknown = JSON.parse(readFileSync(file, "utf8"));
  if (!isState(state)) {
    throw new Error(`${file} holds no state of the restore`);
  }
  return state;
}

function writeState(context: Context, state: State): void {
  mkdirSync(join(context.temp, "ci-cache-s3"), { recursive: true });
  writeFileSync(stateFile(context), `${JSON.stringify(state)}\n`);
}

function archiveFile(context: Context, name: string): string {
  return join(context.temp, "ci-cache-s3", `${name}.tar.zst`);
}

// The archive that the restore extracted, which the save keeps until it has compared the
// files whose time of change differs with the files of the archive.
function restoredFile(context: Context, name: string): string {
  // A suffix that no other file of a cache's name ends with, so that no two caches share a
  // file, as `foo.restored` and `foo` would share `foo.restored.tar.zst`.
  return join(context.temp, "ci-cache-s3", `${name}.tar.zst.restored`);
}

// How a restore of one cache went: a hit, a miss, an archive that couldn't be extracted,
// which the save replaces, or a store that couldn't be read, under which the save writes
// nothing, with what the store held of the key before the download.
type Outcome = { outcome: "hit" | "absent" | "unusable" | "unreadable"; stored: Stored };

// Restores one cache. An error of the store or of tar only warns, and an archive that
// couldn't be extracted leaves no part of itself in the tree, so that the job starts cold.
async function restoreOne(
  settings: Settings,
  cache: Cache,
  key: string,
  context: Context,
): Promise<Outcome> {
  // The ETag before the download is that of the archive that the job builds on, or of an
  // older one, which only makes the save leave a newer archive in place.
  const stored = headObject(settings, key, context);
  if (stored.state === "absent") {
    context.log(`ci-cache-s3: ${cache.name} missed ${key}`);
    return { outcome: "absent", stored };
  }
  const archive = restoredFile(context, cache.name);
  const download =
    stored.state === "error"
      ? { status: 1, stderr: stored.why }
      : copy(settings, `s3://${settings.bucket}/${key}`, archive, context);
  // Another job can delete the key between the HEAD and the download, past its bound.
  if (download.status !== 0 && /\(404\)|Not Found|does not exist/iu.test(download.stderr)) {
    context.log(`ci-cache-s3: ${cache.name} missed ${key}`);
    return { outcome: "absent", stored: { state: "absent" } };
  }
  if (download.status !== 0) {
    context.log(
      `::warning::ci-cache-s3: ${cache.name} couldn't read ${key}: ${download.stderr.trim()}`,
    );
    return { outcome: "unreadable", stored };
  }
  const { base, members } = located(cache, context);
  const failed = await readArchive(context, base, archive);
  if (failed !== undefined) {
    rmSync(archive, { force: true });
    context.log(`::warning::ci-cache-s3: ${cache.name} couldn't extract ${key}: ${failed}`);
    // A cache of the home folder holds only what the archives put there, so the restore
    // deletes its part of a tree, and the save replaces the archive. One of the repository
    // can hold files that git checked out, which stay, so its save writes nothing, rather
    // than an archive of part of a tree.
    if (base !== context.home) {
      return { outcome: "unreadable", stored };
    }
    for (const member of members) {
      rmSync(join(base, member), { recursive: true, force: true });
    }
    return { outcome: "unusable", stored };
  }
  context.log(`ci-cache-s3: ${cache.name} restored ${key}`);
  return { outcome: "hit", stored };
}

// The settings of sccache's S3 backend, with incremental builds off, because sccache can't
// cache them, and a server that never stops, because the server alone holds the store's
// keys.
export function sccacheSettings(settings: Settings, prefix: string): Record<string, string> {
  return {
    CARGO_INCREMENTAL: "0",
    SCCACHE_BUCKET: settings.bucket,
    SCCACHE_ENDPOINT: settings.endpoint,
    SCCACHE_REGION: settings.region,
    SCCACHE_S3_KEY_PREFIX: `${prefix}sccache/`,
    SCCACHE_IGNORE_SERVER_IO_ERROR: "1",
    SCCACHE_IDLE_TIMEOUT: "0",
    // A server that a build starts again, without the keys, then finds none at once,
    // rather than wait for the metadata of a cloud machine.
    AWS_EC2_METADATA_DISABLED: "true",
  };
}

// Restores every cache before mise installs the tools, whatever its archive holds, and
// records the paths as it leaves them for the save.
export async function restore(tools: string, context: Context): Promise<void> {
  const list = join(context.workspace, listPath);
  const listed = existsSync(list) ? parseCaches(readFileSync(list, "utf8")) : [];
  // A malformed setting fails the job, with or without the store.
  readBound(context.workspace);
  // Only a job that installs every tool runs the project's tasks, which fill its caches.
  const own = tools.trim() === "" ? listed : [];
  const read = readSettings(context.env);
  if ("missing" in read) {
    const verb = read.missing.length === 1 ? "is" : "are";
    context.log(
      `::notice::ci-cache-s3 uses no store, because ${read.missing.join(", ")} ${verb} empty`,
    );
    return;
  }
  const prefix = prefixOf(context.env);
  // A HEAD of a missing object answers 404 whether the key or the bucket is missing, so a
  // check of the bucket first tells a wrong setting from a miss.
  const bucket = context.run(
    "aws",
    [
      "s3api",
      "head-bucket",
      "--bucket",
      read.settings.bucket,
      "--endpoint-url",
      read.settings.endpoint,
    ],
    { cwd: context.temp, env: awsEnv(read.settings, context) },
  );
  if (bucket.status !== 0) {
    context.log(
      `::warning::ci-cache-s3 uses no store, because the bucket ${read.settings.bucket} can't be read: ${bucket.stderr.trim()}`,
    );
    return;
  }
  const hasCargoLock = existsSync(join(context.workspace, "Cargo.lock"));
  const caches: Restored[] = [];
  for (const cache of [...builtinCaches(tools, hasCargoLock), ...own]) {
    const key = archiveKey(prefix, cache.name);
    mkdirSync(join(context.temp, "ci-cache-s3"), { recursive: true });
    const unwatched = cache.unwatched ?? [];
    const manifest = join(context.temp, "ci-cache-s3", `${cache.name}.manifest`);
    let restored: Outcome;
    try {
      restored = await restoreOne(read.settings, cache, key, context);
      // The paths as the restore left them, or as the job found them on a miss, which the
      // save compares with the paths at its own time.
      const { base, members } = located(cache, context);
      writeFileSync(manifest, treeManifest(base, present(base, members), unwatched));
    } catch (error) {
      context.log(
        `::warning::ci-cache-s3: ${cache.name} couldn't restore ${key}: ${error instanceof Error ? error.message : String(error)}`,
      );
      restored = { outcome: "unreadable", stored: { state: "error", why: "" } };
    }
    const { outcome, stored } = restored;
    const found = stored.state === "found" ? stored : undefined;
    const baseline = outcome === "hit" ? found?.baseline : undefined;
    caches.push({
      name: cache.name,
      key,
      paths: cache.paths,
      unwatched,
      manifest,
      replace: outcome === "unusable",
      unreadable: outcome === "unreadable",
      found: stored.state !== "absent",
      ...(found?.etag === undefined ? {} : { etag: found.etag }),
      ...(found?.modified === undefined ? {} : { modified: found.modified }),
      ...(baseline === undefined ? {} : { baseline }),
    });
  }
  writeState(context, { caches, sccache: "" });
}

// After mise installs the tools, turns sccache on for a Rust project's job that installed
// every tool, where mise has a sccache. It starts sccache's server with the store's keys,
// and writes to $GITHUB_ENV only the settings without them, so that no later step of the
// job gets the keys. RUSTC_WRAPPER names the program itself, because cargo runs no batch
// file, such as a shim of mise on Windows can be.
export function turnOnBuild(tools: string, context: Context): void {
  const state = readState(context);
  const read = readSettings(context.env);
  if (state === undefined || "missing" in read) {
    return;
  }
  const envFile = context.env.GITHUB_ENV ?? "";
  if (tools.trim() !== "" || envFile === "" || !existsSync(join(context.workspace, "Cargo.lock"))) {
    return;
  }
  const which = context.run("mise", ["which", "sccache"], {
    cwd: context.workspace,
    env: plainEnv(context.env),
  });
  const program = which.stdout.trim();
  if (which.status !== 0 || program === "") {
    context.log(
      "::notice::ci-cache-s3 builds without sccache, because mise has none; pin it in the mise files",
    );
    return;
  }
  const prefix = prefixOf(context.env);
  const settings = sccacheSettings(read.settings, prefix);
  const lines = [
    `RUSTC_WRAPPER=${program}`,
    ...Object.entries(settings).map(([key, value]) => `${key}=${value}`),
  ];
  if (lines.some((line) => /[\r\n]/u.test(line))) {
    context.log(
      "::warning::ci-cache-s3 builds without sccache, because a setting holds a line break",
    );
    return;
  }
  const start = context.run(program, ["--start-server"], {
    cwd: context.workspace,
    env: { ...awsEnv(read.settings, context), ...settings },
    detached: true,
  });
  if (start.status !== 0) {
    context.log(
      `::warning::ci-cache-s3 builds without sccache, because its server didn't start (exit ${String(start.status)})`,
    );
    return;
  }
  appendFileSync(envFile, `${lines.join("\n")}\n`);
  writeState(context, { ...state, sccache: program });
  context.log(`ci-cache-s3: sccache caches the build under ${prefix}sccache/`);
}

// Saves each cache whose paths changed since the restore, or whose archive the restore
// couldn't use, each on its own, so that an error of one leaves the others to save, and
// stops sccache in any case.
export async function save(context: Context): Promise<void> {
  const state = readState(context);
  const read = readSettings(context.env);
  if (state === undefined || "missing" in read) {
    return;
  }
  try {
    const bound = readBound(context.workspace);
    for (const cache of state.caches) {
      try {
        await saveOne(read.settings, bound, cache, context);
      } catch (error) {
        context.log(
          `::warning::ci-cache-s3: ${cache.name} saves no archive: ${error instanceof Error ? error.message : String(error)}`,
        );
      } finally {
        rmSync(restoredFile(context, cache.name), { force: true });
      }
    }
  } finally {
    stopSccache(state, context);
  }
}

// Saves one cache where its paths changed since the restore, or where the restore couldn't
// use its archive, unless another job saved it meanwhile, or its bound says to delete it.
async function saveOne(
  settings: Settings,
  bound: Bound,
  cache: Restored,
  context: Context,
): Promise<void> {
  if (cache.unreadable) {
    context.log(
      `ci-cache-s3: ${cache.name} saves nothing, because the restore couldn't read ${cache.key}, which can hold what other branches installed`,
    );
    return;
  }
  if (!archivesExact(context.platform)) {
    context.log(
      `::warning::ci-cache-s3: ${cache.name} saves no archive on ${context.platform}, because no full run of CI has shown that an archive there restores junctions and links that lead nowhere exactly, so the next job installs it cold`,
    );
    return;
  }
  const { base, members } = located(cache, context);
  const paths = present(base, members);
  if (paths.length === 0 && cache.replace) {
    if (savedSince(settings, cache, context)) {
      return;
    }
    removeKey(
      settings,
      cache,
      "the restore couldn't extract it, and the job left none of its paths",
      context,
    );
    return;
  }
  if (paths.length === 0) {
    context.log(`ci-cache-s3: ${cache.name} has nothing to save`);
    return;
  }
  const before = existsSync(cache.manifest) ? readFileSync(cache.manifest, "utf8") : "";
  const compared = compareManifests(before, treeManifest(base, paths, cache.unwatched));
  // A file of the same size whose time of change differs, such as one that a tool wrote
  // again with the same bytes, changed only where its bytes differ from the archive's.
  const restored = restoredFile(context, cache.name);
  let change = compared.change;
  if (change === undefined && compared.unsure.length > 0) {
    change = existsSync(restored)
      ? await firstChangedFile(context, base, restored, compared.unsure)
      : compared.unsure[0];
  }
  rmSync(restored, { force: true });
  if (change === undefined && !cache.replace) {
    context.log(`ci-cache-s3: ${cache.name} unchanged, so ${cache.key} stays`);
    return;
  }
  if (change !== undefined) {
    context.log(`ci-cache-s3: ${cache.name} changed since the restore, first at ${change}`);
  }
  if (savedSince(settings, cache, context)) {
    return;
  }
  const archive = archiveFile(context, cache.name);
  let failed: string | undefined;
  try {
    failed = await writeArchive(context, base, paths, archive);
  } catch (error) {
    failed = error instanceof Error ? error.message : String(error);
  }
  if (failed !== undefined) {
    rmSync(archive, { force: true });
    context.log(
      `::warning::ci-cache-s3: ${cache.name} saves no archive, because it couldn't archive its paths exactly: ${failed}`,
    );
    return;
  }
  // Another job can have saved while this one wrote its archive.
  if (savedSince(settings, cache, context)) {
    rmSync(archive, { force: true });
    return;
  }
  // A save after a cold start sets the baseline, and a later one keeps it, unless the
  // archive would grow past its bound, when the save deletes it, so that the next job
  // builds the cache cold from the config.
  const size = statSync(archive).size;
  const baseline = cache.baseline ?? size;
  const limit = limitOf(bound, baseline);
  if (size > limit) {
    rmSync(archive, { force: true });
    const why = `the archive would be ${String(size)} bytes, past its bound of ${String(limit)} bytes, ${describe(bound, baseline)}`;
    // A cold archive past a fixed bound isn't written, and a stored one, even a broken one
    // that the restore couldn't use, is deleted, so that no job restores it again.
    if (!cache.found) {
      context.log(`::notice::ci-cache-s3: ${cache.name} saves no archive, because ${why}`);
      return;
    }
    removeKey(
      settings,
      cache,
      `${why}, so the next job builds it cold and sets a new baseline`,
      context,
    );
    return;
  }
  await upload(settings, cache, archive, { size, baseline }, context);
  rmSync(archive, { force: true });
}

// Deletes a cache's archive and its baseline from the store, so that the next job builds the
// cache cold, and logs why.
function removeKey(settings: Settings, cache: Restored, why: string, context: Context): void {
  const remove = context.run(
    "aws",
    [
      "s3",
      "rm",
      `s3://${settings.bucket}/${cache.key}`,
      "--endpoint-url",
      settings.endpoint,
      "--only-show-errors",
    ],
    { cwd: context.temp, env: awsEnv(settings, context) },
  );
  context.log(
    remove.status === 0
      ? `::notice::ci-cache-s3: ${cache.name} deleted ${cache.key} and its baseline, because ${why}`
      : `::warning::ci-cache-s3: ${cache.name} couldn't delete ${cache.key}: ${remove.stderr.trim()}`,
  );
}

// Stops sccache, which prints its statistics, so that each run records its hit rate.
function stopSccache(state: State, context: Context): void {
  if (state.sccache !== "") {
    const stop = context.run(state.sccache, ["--stop-server"], {
      cwd: context.workspace,
      env: plainEnv(context.env),
    });
    context.log(
      stop.status === 0
        ? stop.stdout.trim()
        : `ci-cache-s3: sccache --stop-server: ${stop.stderr.trim()}`,
    );
  }
}

// The context of a run on a runner of GitHub Actions. The store's keys come from the
// action's inputs, which reach no other step, or else from the environment.
export function runnerContext(env: Env, home: string): Context {
  const input = (name: string, fallback: string): string | undefined =>
    (env[`INPUT_${name}`] ?? "") === "" ? env[fallback] : env[`INPUT_${name}`];
  return {
    env: {
      ...env,
      CI_CACHE_S3_ACCESS_KEY_ID: input("ACCESS-KEY-ID", "CI_CACHE_S3_ACCESS_KEY_ID"),
      CI_CACHE_S3_SECRET_ACCESS_KEY: input("SECRET-ACCESS-KEY", "CI_CACHE_S3_SECRET_ACCESS_KEY"),
    },
    home,
    workspace: env.GITHUB_WORKSPACE ?? process.cwd(),
    temp: env.RUNNER_TEMP ?? process.cwd(),
    platform: process.platform,
    run: runCommand,
    pipe: runPipe,
    log: (line) => {
      console.log(line);
    },
  };
}
