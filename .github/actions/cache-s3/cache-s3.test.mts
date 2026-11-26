// Tests how cache-s3.mts reads its settings and the project's list, names its keys, and
// restores and saves the caches, with a double of the commands that it runs.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  rmSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { test } from "node:test";
import { runPipe } from "./archive.mts";
import {
  archiveKey,
  builtinCaches,
  type Env,
  parseCaches,
  prefixOf,
  readSettings,
  restore,
  type Result,
  save,
  scopeOf,
  toolsName,
  plainEnv,
  runCommand,
  runnerContext,
  turnOnBuild,
} from "./cache-s3.mts";
import { type Call, scratch, scratchFolder, settingsEnv, store } from "./test-runner.mts";
import { snapshot, toolsTree } from "./test-tree.mts";

test("the settings are read, with the region auto by default", () => {
  assert.deepEqual(readSettings(settingsEnv), {
    settings: {
      endpoint: "https://s3.example.com",
      bucket: "ci-cache",
      region: "auto",
      accessKeyId: "key-id",
      secretAccessKey: "secret",
    },
  });
  const read = readSettings({ ...settingsEnv, CI_CACHE_S3_REGION: "eu-west-1" });
  assert.ok("settings" in read && read.settings.region === "eu-west-1");
});

test("an empty or blank setting is named as missing", () => {
  assert.deepEqual(
    readSettings({
      ...settingsEnv,
      CI_CACHE_S3_BUCKET: " ",
      CI_CACHE_S3_SECRET_ACCESS_KEY: undefined,
    }),
    {
      missing: ["CI_CACHE_S3_BUCKET", "CI_CACHE_S3_SECRET_ACCESS_KEY"],
    },
  );
});

test("only a release tag takes the release scope", () => {
  assert.equal(scopeOf("refs/tags/v1.2.3"), "release");
  assert.equal(scopeOf("refs/tags/v0.1.0-rc.1"), "release");
  assert.equal(scopeOf("refs/heads/main"), "dev");
  assert.equal(scopeOf("refs/pull/7/merge"), "dev");
  assert.equal(scopeOf("refs/tags/nightly"), "dev");
  assert.equal(scopeOf(""), "dev");
});

test("a key starts with the scope, the repository, and the platform", () => {
  assert.equal(prefixOf(settingsEnv), "dev/owner/repo/Linux-X64/");
  assert.equal(
    prefixOf({
      ...settingsEnv,
      GITHUB_REF: "refs/tags/v2.0.0",
      RUNNER_OS: "macOS",
      RUNNER_ARCH: "ARM64",
    }),
    "release/owner/repo/macOS-ARM64/",
  );
  assert.equal(
    archiveKey("dev/owner/repo/Linux-X64/", "cargo"),
    "dev/owner/repo/Linux-X64/cargo/v3/current.tar.zst",
  );
  assert.throws(
    () => prefixOf({ ...settingsEnv, GITHUB_REPOSITORY: "../x" }),
    /GITHUB_REPOSITORY/u,
  );
});

test("each set of tools has a cache of its own", () => {
  assert.equal(toolsName(""), "tools");
  assert.equal(toolsName("  "), "tools");
  assert.match(toolsName("node"), /^tools-node-[0-9a-f]{8}$/u);
  assert.equal(toolsName("rust cargo:cargo-deny"), toolsName("cargo:cargo-deny  rust"));
  assert.match(toolsName("rust cargo:cargo-deny"), /^tools-cargo-cargo-deny-rust-[0-9a-f]{8}$/u);
  assert.notEqual(toolsName("cargo:foo"), toolsName("cargo-foo"));
});

test("cargo's downloads are cached only beside a Cargo.lock, in a job of every tool", () => {
  assert.deepEqual(
    builtinCaches("", false).map((cache) => cache.name),
    ["tools"],
  );
  assert.deepEqual(
    builtinCaches("", true).map((cache) => cache.name),
    ["tools", "cargo"],
  );
  assert.equal(builtinCaches("node", true).length, 1);
});

test("the project's list gives each cache its name and paths", () => {
  const text = JSON.stringify([
    { name: "playwright", paths: ["~/.cache/ms-playwright"] },
    { name: "web.build", paths: ["web/.cache", "web/dist"] },
  ]);
  assert.deepEqual(parseCaches(text), [
    { name: "playwright", paths: ["~/.cache/ms-playwright"] },
    { name: "web.build", paths: ["web/.cache", "web/dist"] },
  ]);
  assert.deepEqual(parseCaches("[]"), []);
});

test("a malformed list fails with a message that names the file", () => {
  const cases: [string, RegExp][] = [
    ["{", /not JSON/u],
    ["{}", /not a list of caches/u],
    ["[1]", /cache 1 is not an object/u],
    ['[{"name": "Big", "paths": ["a"] }]', /needs a name/u],
    ['[{"name": "tools-x", "paths": ["a"] }]', /the pack's own caches/u],
    ['[{"name": "cargo", "paths": ["a"] }]', /the pack's own caches/u],
    [
      '[{"name": "a", "paths": ["a"] }, {"name": "a", "paths": ["b"] }]',
      /cache 2 takes the name a a second time/u,
    ],
    ['[{"name": "a", "paths": [] }]', /needs paths/u],
    ['[{"name": "a", "paths": ["/etc"] }]', /must name a folder or a file inside/u],
    ['[{"name": "a", "paths": ["~/../x"] }]', /must name a folder or a file inside/u],
    ['[{"name": "a", "paths": ["C:/x"] }]', /must name a folder or a file inside/u],
    ['[{"name": "a", "paths": ["-build"] }]', /not start with -/u],
    ['[{"name": "a", "paths": ["."] }]', /without \. or \.\./u],
    ['[{"name": "a", "paths": ["~/."] }]', /without \. or \.\./u],
    ['[{"name": "a", "paths": ["build/"] }]', /an empty part/u],
    ['[{"name": "a", "paths": ["./build"] }]', /without \. or \.\./u],
    ['[{"name": "a", "paths": ["~/--exclude=x"] }]', /not start with -/u],
    ['[{"name": "a", "paths": ["~/a", "b"] }]', /mixes paths/u],
  ];
  for (const [text, message] of cases) {
    assert.throws(
      () => parseCaches(text),
      (error: Error) =>
        error.message.startsWith(".config/ci-cache-s3/caches.project.json: ") &&
        message.test(error.message),
      text,
    );
  }
});

test("a list that names keyFiles fails with a message that names the file and says the key is gone", () => {
  assert.throws(
    () =>
      parseCaches(JSON.stringify([{ name: "web", paths: ["web/.cache"], keyFiles: ["a.json"] }])),
    (error: Error) =>
      error.message.startsWith(".config/ci-cache-s3/caches.project.json: cache 1 has keyFiles") &&
      error.message.includes("no longer reads"),
  );
});

test("without the store's settings, the restore names them, runs nothing, and the save does nothing", async () => {
  const run = scratch(() => ({}), {
    ...settingsEnv,
    CI_CACHE_S3_ENDPOINT: "",
    CI_CACHE_S3_BUCKET: "",
  });
  await restore("", run.context);
  assert.deepEqual(run.calls, []);
  assert.deepEqual(run.lines, [
    "::notice::ci-cache-s3 uses no store, because CI_CACHE_S3_ENDPOINT, CI_CACHE_S3_BUCKET are empty",
  ]);
  turnOnBuild("", run.context);
  await save(run.context);
  assert.deepEqual(run.calls, []);
  assert.equal(readFileSync(run.envFile, "utf8"), "");
});

test("a malformed list fails the restore even without the store", async () => {
  const run = scratch(() => ({}), { ...settingsEnv, CI_CACHE_S3_BUCKET: "" });
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(join(run.dirs.workspace, ".config/ci-cache-s3/caches.project.json"), "{");
  await assert.rejects(restore("", run.context), /caches\.project\.json: not JSON/u);
});

test("a hit extracts its archive, and a miss is saved after the task", async () => {
  const run = scratch(store((key) => !key.includes("/cargo/")));
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  await restore("", run.context);
  assert.deepEqual(run.calls[0]?.args.slice(0, 4), [
    "s3api",
    "head-bucket",
    "--bucket",
    "ci-cache",
  ]);
  // The restore reads each key with head-object before it downloads one that exists.
  const downloads = run.calls
    .filter((call) => call.command === "aws" && call.args[1] === "head-object")
    .map((call) => `s3://ci-cache/${call.args[5] ?? ""}`);
  assert.equal(downloads.length, 2);
  // The ETag that the save compares comes from before the download, so that an archive that
  // another job uploads meanwhile is never taken for the one that the job restored.
  const order = run.calls.map((call) => call.args[1]);
  assert.ok(order.indexOf("head-object") < order.indexOf("cp"), order.join(" "));
  assert.match(
    downloads[0] ?? "",
    /^s3:\/\/ci-cache\/dev\/owner\/repo\/Linux-X64\/tools\/v3\/current\.tar\.zst$/u,
  );
  assert.match(
    downloads[1] ?? "",
    /^s3:\/\/ci-cache\/dev\/owner\/repo\/Linux-X64\/cargo\/v3\/current\.tar\.zst$/u,
  );
  const archives = join(run.dirs.temp, "ci-cache-s3");
  assert.deepEqual(
    run.calls
      .filter((call) => call.command !== "aws")
      .map((call) => [call.command].concat(call.args)),
    [
      ["zstd", "-d", "-q", "-c", join(archives, "tools.tar.zst.restored")],
      ["/usr/bin/tar", "-xpf", "-", "-C", run.dirs.home],
    ],
  );
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: tools restored")));
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: cargo missed")));

  run.calls.length = 0;
  mkdirSync(join(run.dirs.home, ".cargo/registry"), { recursive: true });
  await save(run.context);
  assert.deepEqual(
    run.calls.map((call) =>
      [call.command].concat(call.command === "aws" ? call.args.slice(0, 4) : call.args),
    ),
    [
      // The save looks for another job's archive before it archives and before it uploads.
      ["aws", "s3api", "head-object", "--bucket", "ci-cache"],
      // ~/.cargo/git doesn't exist, so the archive holds only the registry.
      ["/usr/bin/tar", "-cf", "-", "-C", run.dirs.home, ".cargo/registry"],
      ["zstd", "-T0", "-3", "-q", "-f", "-o", join(archives, "cargo.tar.zst")],
      ["aws", "s3api", "head-object", "--bucket", "ci-cache"],
      ["aws", "s3", "cp", join(archives, "cargo.tar.zst"), downloads[1]],
    ],
  );
});

test("a cache whose paths are as the restore left them isn't saved, and one that changed is", async () => {
  const run = scratch(store(() => true));
  const tools = join(run.dirs.home, ".tools");
  mkdirSync(join(tools, "mise/installs/node/26.13.1"), { recursive: true });
  writeFileSync(join(tools, "a.txt"), "a\n");
  // The double extracts nothing, so the tools as they are stand for what the archive held.
  await restore("", run.context);
  run.calls.length = 0;
  await save(run.context);
  assert.deepEqual(run.calls, []);
  assert.ok(
    run.lines.includes(
      "ci-cache-s3: tools unchanged, so dev/owner/repo/Linux-X64/tools/v3/current.tar.zst stays",
    ),
    run.lines.join("\n"),
  );
  // mise installs a new version, and the save uploads the archive again.
  mkdirSync(join(tools, "mise/installs/node/26.14.0"), { recursive: true });
  run.lines.length = 0;
  await save(run.context);
  assert.ok(
    run.lines.includes(
      "ci-cache-s3: tools changed since the restore, first at .tools/mise/installs/node/26.14.0",
    ),
    run.lines.join("\n"),
  );
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: tools saved")));
});

test("a file of the same size whose time of change differs counts as a change", async () => {
  const run = scratch(store(() => true));
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  writeFileSync(join(run.dirs.home, ".tools/a.txt"), "a\n");
  await restore("", run.context);
  utimesSync(join(run.dirs.home, ".tools/a.txt"), new Date(0), new Date(0));
  await save(run.context);
  assert.ok(
    run.lines.includes("ci-cache-s3: tools changed since the restore, first at .tools/a.txt"),
    run.lines.join("\n"),
  );
});

test("cargo's record of its uses of the downloads counts for no change, and goes in the archive", async () => {
  const run = scratch(store(() => true));
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  mkdirSync(join(run.dirs.home, ".cargo/registry/cache"), { recursive: true });
  writeFileSync(join(run.dirs.home, ".cargo/.global-cache"), "uses\n");
  await restore("", run.context);
  writeFileSync(join(run.dirs.home, ".cargo/.global-cache"), "newer uses\n");
  run.calls.length = 0;
  await save(run.context);
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: cargo unchanged")));
  writeFileSync(join(run.dirs.home, ".cargo/registry/cache/x.crate"), "crate\n");
  run.calls.length = 0;
  await save(run.context);
  const tar = run.calls.find((call) => call.command === "/usr/bin/tar");
  assert.deepEqual(tar?.args.slice(4), [".cargo/registry", ".cargo/.global-cache"]);
});

test("a job restores the one archive of a cache whatever it holds", async () => {
  // The store holds the archive of the tools under its one key, whatever the mise files say.
  const run = scratch(store((key) => key.endsWith("/tools/v3/current.tar.zst")));
  writeFileSync(join(run.dirs.workspace, "mise.toml"), '[tools]\nnode = "26.13.1"\n');
  await restore("", run.context);
  writeFileSync(join(run.dirs.workspace, "mise.toml"), '[tools]\nnode = "26.14.0"\n');
  const other = scratch(store((key) => key.endsWith("/tools/v3/current.tar.zst")));
  await restore("", other.context);
  for (const job of [run, other]) {
    assert.ok(
      job.lines.includes(
        "ci-cache-s3: tools restored dev/owner/repo/Linux-X64/tools/v3/current.tar.zst",
      ),
      job.lines.join("\n"),
    );
  }
});

test("a job of a smaller set of tools caches neither cargo's downloads nor the project's caches", async () => {
  const run = scratch(store(() => false));
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/caches.project.json"),
    JSON.stringify([{ name: "web", paths: ["web/.cache"] }]),
  );
  await restore("node", run.context);
  const names = run.calls
    .filter((call) => call.args[1] === "head-object")
    .map((call) => `s3://ci-cache/${call.args[5] ?? ""}`)
    .map((url) => url.split("/").at(-3));
  assert.equal(names.length, 1);
  assert.match(names[0] ?? "", /^tools-node-[0-9a-f]{8}$/u);
});

test("an archive that can't be extracted is replaced", async () => {
  const run = scratch((call) =>
    call.command === "/usr/bin/tar" && call.args[0] === "-xpf" ? { status: 2, stderr: "bad" } : {},
  );
  await restore("", run.context);
  assert.ok(
    run.lines.some(
      (line) => line.includes("tools couldn't extract") && line.endsWith(": /usr/bin/tar: bad"),
    ),
  );
  // The job installs the tools cold.
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  run.calls.length = 0;
  await save(run.context);
  assert.deepEqual(
    run.calls.map((call) => `${call.command} ${call.args[0] ?? ""} ${call.args[1] ?? ""}`),
    [
      "aws s3api head-object",
      "/usr/bin/tar -cf -",
      "zstd -T0 -3",
      "aws s3api head-object",
      "aws s3 cp",
    ],
  );
});

test("a bucket that can't be read warns and leaves the store out", async () => {
  const run = scratch((call) =>
    call.args[1] === "head-bucket" ? { status: 254, stderr: "Not Found" } : {},
  );
  await restore("", run.context);
  assert.equal(run.calls.length, 1);
  assert.deepEqual(run.lines, [
    "::warning::ci-cache-s3 uses no store, because the bucket ci-cache can't be read: Not Found",
  ]);
  await save(run.context);
  assert.equal(run.calls.length, 1);
});

test("an error of the store warns, and the save writes nothing over the archive that it couldn't read", async () => {
  const run = scratch((call) =>
    call.command === "aws" && call.args[1] !== "head-bucket"
      ? { status: 1, stderr: "Could not connect to the endpoint URL" }
      : {},
  );
  await restore("", run.context);
  const key = archiveKey("dev/owner/repo/Linux-X64/", "tools");
  assert.deepEqual(run.lines, [
    `::warning::ci-cache-s3: tools couldn't read ${key}: Could not connect to the endpoint URL`,
  ]);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  run.lines.length = 0;
  run.calls.length = 0;
  await save(run.context);
  assert.ok(
    run.lines.includes(
      `ci-cache-s3: tools saves nothing, because the restore couldn't read ${key}, which can hold what other branches installed`,
    ),
    run.lines.join("\n"),
  );
  assert.ok(!run.calls.some((call) => call.args[1] === "cp"));
});

test("an archive that can't be extracted leaves none of its paths, so that the job starts cold", async () => {
  const run = scratch((call) =>
    call.command === "/usr/bin/tar" && call.args[0] === "-xpf"
      ? { status: 2, stderr: "corrupt" }
      : store(() => true)(call),
  );
  // What the extraction left before it failed.
  mkdirSync(join(run.dirs.home, ".tools/mise/installs/python"), { recursive: true });
  await restore("", run.context);
  assert.equal(existsSync(join(run.dirs.home, ".tools")), false);
});

test("a project's cache from the repository is restored into the repository", async () => {
  const run = scratch(store((key) => key.includes("/web/")));
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/caches.project.json"),
    JSON.stringify([{ name: "web", paths: ["web/.cache"] }]),
  );
  await restore("", run.context);
  assert.deepEqual(
    run.calls.filter((call) => call.command === "/usr/bin/tar").map((call) => call.args),
    [["-xpf", "-", "-C", run.dirs.workspace]],
  );
});

const sccache = "/tools/mise/installs/sccache/0.19.0/sccache";

test("sccache turns on after the install in a Rust job of every tool, with its keys only in its server", async () => {
  const envs = new Map<string, Env>();
  const run = scratch((call) => (call.command === "mise" ? { stdout: `${sccache}\n` } : {}));
  const answer = run.context.run;
  run.context.run = (command, args, options) => {
    envs.set(`${command} ${args.join(" ")}`, options.env ?? {});
    return answer(command, args, options);
  };
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  await restore("", run.context);
  turnOnBuild("", run.context);
  const written = readFileSync(run.envFile, "utf8");
  assert.deepEqual(written.trimEnd().split("\n"), [
    `RUSTC_WRAPPER=${sccache}`,
    "CARGO_INCREMENTAL=0",
    "SCCACHE_BUCKET=ci-cache",
    "SCCACHE_ENDPOINT=https://s3.example.com",
    "SCCACHE_REGION=auto",
    "SCCACHE_S3_KEY_PREFIX=dev/owner/repo/Linux-X64/sccache/",
    "SCCACHE_IGNORE_SERVER_IO_ERROR=1",
    "SCCACHE_IDLE_TIMEOUT=0",
    "AWS_EC2_METADATA_DISABLED=true",
  ]);
  assert.ok(!written.includes("secret"));
  const server = envs.get(`${sccache} --start-server`) ?? {};
  assert.equal(server.AWS_SECRET_ACCESS_KEY, "secret");
  assert.equal(server.SCCACHE_BUCKET, "ci-cache");
  run.calls.length = 0;
  await save(run.context);
  assert.deepEqual(
    run.calls
      .filter((call) => call.command !== "aws")
      .map((call) => [call.command].concat(call.args)),
    [[sccache, "--stop-server"]],
  );
});

function withSccache(call: Call): Partial<Result> {
  return call.command === "mise" ? { stdout: `${sccache}\n` } : {};
}

test("sccache stays off for a set of tools, without a Cargo.lock, where mise has none, and where its server fails", async () => {
  {
    const run = scratch(withSccache);
    writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
    await restore("node", run.context);
    turnOnBuild("node", run.context);
    assert.equal(readFileSync(run.envFile, "utf8"), "");
  }
  {
    const run = scratch(withSccache);
    await restore("", run.context);
    turnOnBuild("", run.context);
    assert.equal(readFileSync(run.envFile, "utf8"), "");
  }
  {
    const run = scratch((call) => (call.command === "mise" ? { status: 1 } : {}));
    writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
    await restore("", run.context);
    turnOnBuild("", run.context);
    assert.equal(readFileSync(run.envFile, "utf8"), "");
    assert.ok(run.lines.at(-1)?.includes("mise has none") === true);
  }
  {
    const run = scratch((call) =>
      call.command === sccache ? { status: 2, stderr: "no server" } : withSccache(call),
    );
    writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
    await restore("", run.context);
    turnOnBuild("", run.context);
    assert.equal(readFileSync(run.envFile, "utf8"), "");
    assert.ok(run.lines.at(-1)?.includes("its server didn't start (exit 2)") === true);
  }
});

test("the store's keys come from the action's inputs, or else from the environment", () => {
  const fromInputs = runnerContext(
    {
      "INPUT_ACCESS-KEY-ID": "in-id",
      "INPUT_SECRET-ACCESS-KEY": "in-secret",
      CI_CACHE_S3_ACCESS_KEY_ID: "env-id",
    },
    "/home",
  );
  assert.equal(fromInputs.env.CI_CACHE_S3_ACCESS_KEY_ID, "in-id");
  assert.equal(fromInputs.env.CI_CACHE_S3_SECRET_ACCESS_KEY, "in-secret");
  const fromEnv = runnerContext(
    { "INPUT_ACCESS-KEY-ID": "", CI_CACHE_S3_ACCESS_KEY_ID: "env-id" },
    "/home",
  );
  assert.equal(fromEnv.env.CI_CACHE_S3_ACCESS_KEY_ID, "env-id");
  assert.equal(fromEnv.env.CI_CACHE_S3_SECRET_ACCESS_KEY, undefined);
});

test("only the AWS CLI and sccache's server get the store's keys", async () => {
  assert.deepEqual(
    plainEnv({
      PATH: "/bin",
      CI_CACHE_S3_BUCKET: "ci-cache",
      CI_CACHE_S3_ACCESS_KEY_ID: "id",
      CI_CACHE_S3_SECRET_ACCESS_KEY: "secret",
      "INPUT_ACCESS-KEY-ID": "id",
      "INPUT_SECRET-ACCESS-KEY": "secret",
    }),
    { PATH: "/bin", CI_CACHE_S3_BUCKET: "ci-cache" },
  );
  const envs = new Map<string, Env>();
  const run = scratch((call) => (call.command === "mise" ? { stdout: `${sccache}\n` } : {}));
  const answer = run.context.run;
  run.context.run = (command, args, options) => {
    envs.set(command, options.env ?? {});
    return answer(command, args, options);
  };
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  await restore("", run.context);
  turnOnBuild("", run.context);
  for (const command of ["zstd", "/usr/bin/tar", "mise"]) {
    assert.ok(envs.has(command), command);
    assert.equal(envs.get(command)?.CI_CACHE_S3_SECRET_ACCESS_KEY, undefined, command);
  }
  assert.equal(envs.get("aws")?.AWS_SECRET_ACCESS_KEY, "secret");
});

test("on a platform that no run has shown exact, the save saves no archive, and says why", async () => {
  const run = scratch(store(() => false));
  run.context.platform = "freebsd";
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  await restore("", run.context);
  run.calls.length = 0;
  await save(run.context);
  assert.deepEqual(run.calls, []);
  assert.ok(
    run.lines.some(
      (line) =>
        line.startsWith("::warning::ci-cache-s3: tools saves no archive on freebsd, because") &&
        line.includes("junctions and links that lead nowhere") &&
        line.endsWith("installs it cold"),
    ),
    run.lines.join("\n"),
  );
});

test("a restore takes no archive of an earlier format", async () => {
  // The store holds only archives of earlier formats, as v1 and v2 wrote them.
  const run = scratch(store((key) => !key.includes("/v3/")));
  await restore("", run.context);
  const downloads = run.calls
    .filter((call) => call.args[1] === "head-object")
    .map((call) => call.args[5] ?? "");
  assert.ok(downloads.length > 0);
  assert.ok(
    downloads.every((key) => key.includes("/tools/v3/")),
    downloads.join("\n"),
  );
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: tools missed")));
  assert.ok(!run.calls.some((call) => call.command === "zstd"));
});

test("on Windows, the save lists every entry but the links for bsdtar, and records the links", async () => {
  const run = scratch((call) => {
    if (call.command.endsWith("powershell.exe")) {
      return { stdout: '["junction","file"]\r\n' };
    }
    return store(() => false)(call);
  });
  run.context.platform = "win32";
  const tools = join(run.dirs.home, ".tools");
  await restore("", run.context);
  // The job installs the tools after the restore missed.
  mkdirSync(join(tools, "pkg/real"), { recursive: true });
  writeFileSync(join(tools, "pkg/real/index.js"), "export {};\n");
  symlinkSync(join(tools, "pkg/real"), join(tools, "pkg/latest"));
  symlinkSync("../none.txt", join(tools, "pkg/nowhere"));
  run.calls.length = 0;
  await save(run.context);
  const folder = join(run.dirs.temp, "ci-cache-s3");
  const list = join(folder, "tools.tar.zst.list");
  const record = join(folder, "tools.tar.zst.links");
  const tar = run.calls.find((call) => call.command.endsWith("tar.exe"));
  assert.deepEqual(tar?.args, ["-c", "-n", "-f", "-", "-T", list]);
  assert.equal(
    readFileSync(list, "utf8"),
    [
      "-C",
      run.dirs.home,
      ".tools",
      ".tools/pkg",
      ".tools/pkg/real",
      ".tools/pkg/real/index.js",
      "-C",
      record,
      ".ci-cache-s3-links.json",
      "",
    ].join("\n"),
  );
  assert.deepEqual(JSON.parse(readFileSync(join(record, ".ci-cache-s3-links.json"), "utf8")), [
    { path: ".tools/pkg/latest", target: join(tools, "pkg/real"), kind: "junction" },
    // Windows writes the target of a symbolic link with its own separator.
    { path: ".tools/pkg/nowhere", target: readlinkSync(join(tools, "pkg/nowhere")), kind: "file" },
  ]);
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: tools saved")));
});

test("a failure of tar or of zstd saves nothing, and names each program that failed", async () => {
  const run = scratch((call) => {
    if (call.command === "zstd") {
      return { status: null, stderr: "spawn zstd ENOENT" };
    }
    if (call.command === "/usr/bin/tar") {
      return { status: 2, stderr: "tar: write error" };
    }
    return store(() => false)(call);
  });
  await restore("", run.context);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  run.calls.length = 0;
  await save(run.context);
  assert.ok(
    run.lines.includes(
      "::warning::ci-cache-s3: tools saves no archive, because it couldn't archive its paths exactly: /usr/bin/tar: tar: write error; zstd: spawn zstd ENOENT",
    ),
    run.lines.join("\n"),
  );
  assert.ok(!run.calls.some((call) => call.args[1] === "cp"));
});

const hasZstd = spawnSync("zstd", ["--version"]).status === 0;

// The file of a folder that stands for a key of the store, with a name that every
// platform takes: Windows reads a colon, as in `s3:`, as the start of a stream of the file.
function stored(bucket: string, key: string): string {
  return join(bucket, key.replaceAll(/[^A-Za-z0-9._-]/gu, "_"));
}

test(
  "a saved archive restores the tools exactly through the store, and an unchanged job saves none",
  { skip: hasZstd ? false : "no zstd on the PATH" },
  async () => {
    const bucket = scratchFolder("cache-s3-bucket-");
    const run = scratch();
    run.context.platform = process.platform;
    run.context.pipe = runPipe;
    // tar and zstd come from the PATH.
    run.context.env = { ...process.env, ...run.context.env };
    // The store's double copies the files of a folder, and the other commands run.
    run.context.run = (command, args, options) => {
      if (command !== "aws") {
        return runCommand(command, args, options);
      }
      const [, operation, from = "", to = ""] = args;
      if (operation === "cp" && from.startsWith("s3://")) {
        if (!existsSync(stored(bucket, from))) {
          return { status: 1, stdout: "", stderr: "An error occurred (404)" };
        }
        copyFileSync(stored(bucket, from), to);
      } else if (operation === "cp") {
        copyFileSync(from, stored(bucket, to));
      } else if (operation === "head-object") {
        return existsSync(stored(bucket, `s3://ci-cache/${args[5] ?? ""}`))
          ? { status: 0, stdout: "{}", stderr: "" }
          : { status: 254, stdout: "", stderr: "Not Found" };
      }
      return { status: 0, stdout: "", stderr: "" };
    };
    await restore("", run.context);
    // The job installs the tools after the restore missed.
    toolsTree(run.dirs.home);
    const tools = join(run.dirs.home, ".tools");
    const before = snapshot(tools, run.context);
    await save(run.context);
    assert.ok(
      run.lines.some((line) => line.startsWith("ci-cache-s3: tools saved")),
      run.lines.join("\n"),
    );
    rmSync(tools, { recursive: true });
    await restore("", run.context);
    assert.ok(
      run.lines.some((line) => line.startsWith("ci-cache-s3: tools restored")),
      run.lines.join("\n"),
    );
    assert.deepEqual(snapshot(tools, run.context), before);
    // A job that changes nothing after the restore saves nothing, nor one that writes a file
    // again with the same bytes, as mise does with .mise-installs.toml.
    writeFileSync(join(tools, "a.txt"), "a\n");
    const later = new Date(Date.now() + 5000);
    utimesSync(join(tools, "a.txt"), later, later);
    run.lines.length = 0;
    await save(run.context);
    assert.ok(
      run.lines.some((line) => line.startsWith("ci-cache-s3: tools unchanged")),
      run.lines.join("\n"),
    );
  },
);

test("an error of one cache's save leaves the others to save, and stops sccache", async () => {
  const run = scratch((call) =>
    call.command === "mise" ? { stdout: `${sccache}\n` } : store(() => false)(call),
  );
  writeFileSync(join(run.dirs.workspace, "Cargo.lock"), "lock\n");
  mkdirSync(join(run.dirs.workspace, ".config/ci-cache-s3"), { recursive: true });
  writeFileSync(
    join(run.dirs.workspace, ".config/ci-cache-s3/caches.project.json"),
    JSON.stringify([{ name: "web", paths: ["web/.cache"] }]),
  );
  await restore("", run.context);
  turnOnBuild("", run.context);
  // The manifest of the tools can't be read, as a file that a job removes or locks.
  const manifest = join(run.dirs.temp, "ci-cache-s3", "tools.manifest");
  rmSync(manifest);
  mkdirSync(manifest);
  mkdirSync(join(run.dirs.home, ".tools"), { recursive: true });
  mkdirSync(join(run.dirs.workspace, "web/.cache"), { recursive: true });
  run.calls.length = 0;
  await save(run.context);
  assert.ok(
    run.lines.some((line) => line.startsWith("::warning::ci-cache-s3: tools saves no archive: ")),
    run.lines.join("\n"),
  );
  assert.ok(run.lines.some((line) => line.startsWith("ci-cache-s3: web saved")));
  assert.ok(run.calls.some((call) => call.command === sccache && call.args[0] === "--stop-server"));
});
