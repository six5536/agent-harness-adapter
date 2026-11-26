// Tests how archive.mts archives a tree and restores it exactly, through the platform's own
// tar and zstd, and how it pipes one program into another.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import process from "node:process";
import { after, test } from "node:test";
import {
  compareManifests,
  firstChangedFile,
  present,
  readArchive,
  runPipe,
  tarProgram,
  treeManifest,
  walkTree,
  writeArchive,
} from "./archive.mts";
import { type Context, runCommand } from "./cache-s3.mts";
import { snapshot, toolsTree } from "./test-tree.mts";

const roots: string[] = [];
const lines: string[] = [];
after(() => {
  for (const root of roots) {
    rmSync(root, { recursive: true, force: true });
  }
});

// A runner of the real commands, with a home and a temporary folder of its own.
function scratch(): Context {
  const root = mkdtempSync(join(tmpdir(), "cache-s3-archive-"));
  roots.push(root);
  const context: Context = {
    env: { ...process.env },
    home: join(root, "home"),
    workspace: join(root, "workspace"),
    temp: join(root, "temp"),
    platform: process.platform,
    run: runCommand,
    pipe: runPipe,
    log: (line) => {
      lines.push(line);
    },
  };
  for (const dir of [context.home, context.workspace, context.temp]) {
    mkdirSync(dir, { recursive: true });
  }
  return context;
}

test("each platform's own tar runs by its path", () => {
  assert.equal(tarProgram("linux", {}), "/usr/bin/tar");
  assert.equal(tarProgram("darwin", {}), "/usr/bin/tar");
  assert.equal(tarProgram("win32", { SystemRoot: "D:\\Win" }), "D:\\Win\\System32\\tar.exe");
  assert.equal(tarProgram("win32", {}), "C:\\Windows\\System32\\tar.exe");
});

test("the walk of a tree finds its links without following any, and lists every other entry", () => {
  const context = scratch();
  toolsTree(context.home);
  const { entries, links } = walkTree(context.home, [".tools"]);
  const npm = ".tools/mise/installs/npm-typescript";
  assert.deepEqual(links, [
    ".tools/links/file-to-nowhere",
    ".tools/links/folder-to-nowhere",
    ".tools/links/to-file",
    ".tools/links/to-folder",
    `${npm}/7.0.2/node_modules/.mise/typescript@7.0.2/node_modules/@typescript/typescript-aix-ppc64`,
    `${npm}/7.0.2/node_modules/.mise/typescript@7.0.2/node_modules/@typescript/typescript-win32-x64`,
    `${npm}/latest`,
  ]);
  // A folder that holds links is listed, without them.
  assert.ok(entries.includes(".tools/links"));
  assert.ok(
    entries.includes(`${npm}/7.0.2/node_modules/.mise/typescript@7.0.2/node_modules/@typescript`),
  );
  assert.ok(entries.includes(".tools/a.txt"));
  assert.ok(!entries.some((entry) => links.includes(entry)));
  assert.equal(entries[0], ".tools");
});

test("a save whose tar fails midway leaves the tree as it was, and names the failure", async () => {
  const context = scratch();
  toolsTree(context.home);
  const tools = join(context.home, ".tools");
  const before = snapshot(tools, context);
  // The tar writes part of an archive, and fails.
  context.pipe = (from, to) =>
    runPipe(
      {
        ...from,
        command: process.execPath,
        args: ["-e", "process.stdout.write('x'.repeat(4096)); process.exit(2)"],
      },
      { ...to, command: process.execPath, args: ["-e", "process.stdin.resume()"] },
    );
  const failed = await writeArchive(
    context,
    context.home,
    [".tools"],
    join(context.temp, "t.tar.zst"),
  );
  assert.match(failed ?? "", /exit 2/u);
  assert.deepEqual(snapshot(tools, context), before);
});

const hasZstd = spawnSync("zstd", ["--version"]).status === 0;

// The round trip that decides whether the archives of a platform are exact: on Windows,
// archivesExact turns the save on there only after this test passes in a full run
// of CI, because no developer's machine runs it on Windows.
test(
  "an archive restores a tree with junctions, links, links that lead nowhere, nested node_modules, a long path, and modes exactly",
  { skip: hasZstd ? false : "no zstd on the PATH" },
  async (t) => {
    const context = scratch();
    const tar = spawnSync(tarProgram(process.platform, process.env), ["--version"], {
      encoding: "utf8",
    });
    t.diagnostic(`tar: ${tar.stdout.split(/\r?\n/u)[0] ?? tar.stderr}`);
    toolsTree(context.home);
    const tools = join(context.home, ".tools");
    const before = snapshot(tools, context);
    const archive = join(context.temp, "tools.tar.zst");
    assert.equal(await writeArchive(context, context.home, [".tools"], archive), undefined);
    // The save leaves the tree as it found it.
    assert.deepEqual(snapshot(tools, context), before);
    rmSync(tools, { recursive: true, force: true });
    assert.equal(await readArchive(context, context.home, archive), undefined);
    assert.deepEqual(snapshot(tools, context), before);
  },
);

// A program of the runner's own Node.js.
function node(script: string) {
  return { command: process.execPath, args: ["-e", script] };
}

// A pipe that waits for ever fails its test instead of the job's task, as it hung the task of
// macOS's tests in run 45106982356.
const noHang = { timeout: 20_000 };

test(
  "a pipe hands the output of the first program to the second, and returns both results",
  noHang,
  async () => {
    const [first, second] = await runPipe(
      node("process.stdout.write('x'.repeat(100000))"),
      node(
        "let n = 0; process.stdin.on('data', (c) => { n += c.length; }); process.stdin.on('end', () => { process.exitCode = n === 100000 ? 0 : 1; });",
      ),
    );
    assert.equal(first.status, 0, first.stderr);
    assert.equal(second.status, 0, second.stderr);
  },
);

test("a pipe whose second program ends early ends the first, and fails it", noHang, async () => {
  const [first, second] = await runPipe(
    // A write that waits for the pipe, so that the program holds no more than the pipe.
    node("const b = Buffer.alloc(65536); for (;;) require('node:fs').writeSync(1, b);"),
    node("process.exit(3)"),
  );
  assert.equal(second.status, 3);
  assert.notEqual(first.status, 0);
});

test(
  "a pipe whose second program succeeds before it reads all lets the first end as it would",
  noHang,
  async () => {
    const [first, second] = await runPipe(
      node(
        "const b = Buffer.alloc(65536); for (let i = 0; i < 64; i++) require('node:fs').writeSync(1, b);",
      ),
      node("process.stdin.once('data', () => process.exit(0));"),
    );
    assert.equal(second.status, 0);
    assert.equal(first.status, 0, first.stderr);
  },
);

test(
  "a pipe whose second program succeeds without reading lets far more than a pipe holds drain",
  noHang,
  async () => {
    const [first, second] = await runPipe(
      node(
        "const b = Buffer.alloc(65536); for (let i = 0; i < 1024; i++) require('node:fs').writeSync(1, b);",
      ),
      node("process.exit(0)"),
    );
    assert.equal(second.status, 0);
    assert.equal(first.status, 0, first.stderr);
  },
);

test("a link that leads nowhere at the top of a cache is present, and an absent path isn't", () => {
  const context = scratch();
  symlinkSync(join(context.home, "gone"), join(context.home, "current"));
  assert.deepEqual(present(context.home, ["current", "missing"]), ["current"]);
});

test("the restore makes a link again where an earlier restore of the job left it", async () => {
  const context = scratch();
  context.pipe = () =>
    Promise.resolve([
      { status: 0, stdout: "", stderr: "" },
      { status: 0, stdout: "", stderr: "" },
    ]);
  mkdirSync(join(context.home, "real"));
  symlinkSync(join(context.home, "real"), join(context.home, "latest"), "junction");
  writeFileSync(
    join(context.home, ".ci-cache-s3-links.json"),
    JSON.stringify([{ path: "latest", target: join(context.home, "real"), kind: "junction" }]),
  );
  assert.equal(
    await readArchive(context, context.home, join(context.temp, "a.tar.zst")),
    undefined,
  );
  assert.equal(
    readlinkSync(join(context.home, "latest")).replace(/[\\/]$/u, ""),
    join(context.home, "real"),
  );
});

test("a program that doesn't start fails its side of the pipe with its error", async () => {
  const [first, second] = await runPipe(
    { command: "no-such-program-of-ci-cache-s3", args: [] },
    node("process.stdin.resume()"),
  );
  assert.equal(first.status, null);
  assert.match(first.stderr, /ENOENT/u);
  assert.equal(second.status, 0);
});

// A manifest of the entries, as treeManifest writes one.
const manifest = (...entries: object[]): string =>
  entries.map((entry) => `${JSON.stringify(entry)}\n`).join("");

test("a file of the same size, mode, and time is unchanged, and one of another size or mode is changed, without a read", () => {
  const folder = { path: ".tools", type: "folder", mode: 0o40755 };
  const a = { path: ".tools/a", type: "file", mode: 0o100644, size: 2, mtime: 1000 };
  const b = { path: ".tools/b", type: "file", mode: 0o100644, size: 3, mtime: 1000 };
  const before = manifest(folder, a, b);
  assert.deepEqual(compareManifests(before, before), { unsure: [] });
  assert.deepEqual(compareManifests(before, manifest(folder, a, { ...b, size: 4 })), {
    change: ".tools/b",
    unsure: [],
  });
  // chmod changes no time of change.
  assert.deepEqual(compareManifests(before, manifest(folder, a, { ...b, mode: 0o100755 })), {
    change: ".tools/b",
    unsure: [],
  });
  // A file of the same size and another time of change only its bytes can tell.
  assert.deepEqual(compareManifests(before, manifest(folder, { ...a, mtime: 2000 }, b)), {
    unsure: [".tools/a"],
  });
  assert.deepEqual(compareManifests(before, manifest(folder, a)), {
    change: ".tools/b",
    unsure: [],
  });
  // A name with a tab or a line break fits a line of the manifest.
  const odd = { path: ".tools/a\tb\nc", type: "file", mode: 0o100644, size: 1, mtime: 1 };
  assert.deepEqual(compareManifests(manifest(odd), manifest(odd)), { unsure: [] });
});

test(
  "a file of the same size and another time of change is changed only where its bytes differ from the archive's",
  { skip: hasZstd ? false : "no zstd on the PATH" },
  async () => {
    const context = scratch();
    const tools = join(context.home, ".tools");
    mkdirSync(tools, { recursive: true });
    writeFileSync(join(tools, "same.toml"), "a = 1\n");
    writeFileSync(join(tools, "other.toml"), "b = 1\n");
    const archive = join(context.temp, "tools.restored.tar.zst");
    assert.equal(await writeArchive(context, context.home, [".tools"], archive), undefined);
    const before = treeManifest(context.home, [".tools"]);
    // A tool writes one file again with the same bytes, and one with others of its size.
    writeFileSync(join(tools, "same.toml"), "a = 1\n");
    writeFileSync(join(tools, "other.toml"), "b = 2\n");
    const later = new Date(Date.now() + 5000);
    utimesSync(join(tools, "same.toml"), later, later);
    utimesSync(join(tools, "other.toml"), later, later);
    const { change, unsure } = compareManifests(before, treeManifest(context.home, [".tools"]));
    assert.equal(change, undefined);
    assert.deepEqual(unsure, [".tools/other.toml", ".tools/same.toml"]);
    assert.equal(
      await firstChangedFile(context, context.home, archive, [".tools/same.toml"]),
      undefined,
    );
    assert.equal(
      await firstChangedFile(context, context.home, archive, unsure),
      ".tools/other.toml",
    );
  },
);

test("a record of the links that isn't JSON fails the restore of the archive with a message, and goes", async () => {
  const context = scratch();
  context.pipe = () =>
    Promise.resolve([
      { status: 0, stdout: "", stderr: "" },
      { status: 0, stdout: "", stderr: "" },
    ]);
  writeFileSync(join(context.home, ".ci-cache-s3-links.json"), "{ broken");
  const failed = await readArchive(context, context.home, join(context.temp, "a.tar.zst"));
  assert.match(failed ?? "", /\.ci-cache-s3-links\.json isn't JSON/u);
  assert.equal(existsSync(join(context.home, ".ci-cache-s3-links.json")), false);
});

test("an extraction that fails late leaves no record of the links behind", async () => {
  const context = scratch();
  context.pipe = () =>
    Promise.resolve([
      { status: 1, stdout: "", stderr: "corrupt frame" },
      { status: 0, stdout: "", stderr: "" },
    ]);
  writeFileSync(join(context.home, ".ci-cache-s3-links.json"), "[]");
  const failed = await readArchive(context, context.home, join(context.temp, "a.tar.zst"));
  assert.match(failed ?? "", /corrupt frame/u);
  assert.equal(existsSync(join(context.home, ".ci-cache-s3-links.json")), false);
});
