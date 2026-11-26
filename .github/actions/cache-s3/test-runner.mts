// The runner of the tests of cache-s3.mts and bound.mts: a scratch job whose commands a
// function answers, and doubles of the store.

import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after } from "node:test";
import type { Context, Env, Result } from "./cache-s3.mts";

export const settingsEnv: Env = {
  CI_CACHE_S3_ENDPOINT: "https://s3.example.com",
  CI_CACHE_S3_BUCKET: "ci-cache",
  CI_CACHE_S3_REGION: "",
  CI_CACHE_S3_ACCESS_KEY_ID: "key-id",
  CI_CACHE_S3_SECRET_ACCESS_KEY: "secret",
  GITHUB_REPOSITORY: "owner/repo",
  GITHUB_REF: "refs/heads/feature/x",
  RUNNER_OS: "Linux",
  RUNNER_ARCH: "X64",
};

// The size of each archive that the double of zstd writes, which a test can change.
export const sizes = { archive: 1000 };

export interface Call {
  command: string;
  args: string[];
}

const roots: string[] = [];

// A new folder of its own, which the tests delete when they end.
export function scratchFolder(prefix: string): string {
  const folder = mkdtempSync(join(tmpdir(), prefix));
  roots.push(folder);
  return folder;
}
after(() => {
  for (const root of roots) {
    rmSync(root, { recursive: true, force: true });
  }
});

// A scratch runner with a home, a workspace, and a temporary folder, whose commands a
// function answers, and which records each call and each line of the log.
export function scratch(
  answer: (call: Call) => Partial<Result> = () => ({}),
  env: Env = settingsEnv,
) {
  const root = mkdtempSync(join(tmpdir(), "cache-s3-"));
  roots.push(root);
  const dirs = {
    home: join(root, "home"),
    workspace: join(root, "workspace"),
    temp: join(root, "temp"),
  };
  for (const dir of Object.values(dirs)) {
    mkdirSync(dir, { recursive: true });
  }
  const calls: Call[] = [];
  const lines: string[] = [];
  const envFile = join(root, "github-env");
  writeFileSync(envFile, "");
  const context: Context = {
    env: { ...env, GITHUB_ENV: envFile },
    ...dirs,
    platform: "linux",
    run: (command, args) => {
      const call = { command, args };
      calls.push(call);
      return { status: 0, stdout: "", stderr: "", ...answer(call) };
    },
    // The double runs each program of a pipe as a call of its own, and a zstd that writes an
    // archive writes one of `archiveSize` bytes.
    pipe: (from, to) => {
      const results: [Result, Result] = [
        context.run(from.command, from.args, from),
        context.run(to.command, to.args, to),
      ];
      const output = to.args.indexOf("-o");
      if (to.command === "zstd" && output !== -1 && results[1].status === 0) {
        writeFileSync(to.args[output + 1] ?? "", Buffer.alloc(sizes.archive));
      }
      return Promise.resolve(results);
    },
    log: (line) => {
      lines.push(line);
    },
    // A try again of an upload waits for nothing.
    wait: () => Promise.resolve(),
  };
  return { context, calls, lines, envFile, dirs };
}

export const missing: Partial<Result> = {
  status: 1,
  stderr:
    'fatal error: An error occurred (404) when calling the HeadObject operation: Key "x" does not exist',
};

// The double of a store that holds the keys that `held` matches.
export function store(held: (key: string) => boolean): (call: Call) => Partial<Result> {
  return (call) => {
    if (call.command !== "aws") {
      return {};
    }
    const [, operation] = call.args;
    if (operation === "cp" && call.args[2].startsWith("s3://")) {
      return held(call.args[2]) ? {} : missing;
    }
    if (operation === "head-object") {
      return held(call.args[5] ?? "") ? {} : { status: 254, stderr: "Not Found" };
    }
    return {};
  };
}

// The key of an S3 URL of the bucket of the tests.
function key(url: string): string {
  return url.replace("s3://ci-cache/", "");
}

// A double of the store that keeps the baseline of each key that a save wrote, which a
// test can fill, and its ETag, which each upload changes, as a test can, and that answers a
// HEAD with both.
export function memoryStore(
  objects: Map<string, string>,
  etags = new Map<string, string>(),
): (call: Call) => Partial<Result> {
  let uploads = 0;
  return (call) => {
    if (call.command !== "aws") {
      return {};
    }
    const [, operation, from = "", to = ""] = call.args;
    if (operation === "cp" && from.startsWith("s3://")) {
      return objects.has(key(from)) ? {} : missing;
    }
    if (operation === "cp") {
      const metadata = call.args[call.args.indexOf("--metadata") + 1] ?? "";
      objects.set(key(to), metadata.replace("baseline=", ""));
      uploads += 1;
      etags.set(key(to), `"upload-${String(uploads)}"`);
      return {};
    }
    if (operation === "head-object") {
      const at = call.args[5] ?? "";
      const baseline = objects.get(at);
      const ETag = etags.get(at) ?? '"first"';
      return baseline === undefined
        ? { status: 254, stderr: "Not Found" }
        : {
            stdout: JSON.stringify({
              ContentLength: 1,
              ETag,
              LastModified: "2026-01-01T00:00:00+00:00",
              Metadata: { baseline },
            }),
          };
    }
    if (operation === "rm") {
      objects.delete(key(from));
    }
    return {};
  };
}
