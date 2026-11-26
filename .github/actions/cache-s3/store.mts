// The transfers of ci-cache-s3 to and from its S3 store through the AWS CLI: the CLI's
// environment and settings, a copy, what the store holds of a key, and the upload of an
// archive, which leaves a key that another job saved since the restore.

import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Context, Env, Restored, Result, Settings } from "./cache-s3.mts";

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// The settings of the AWS CLI's transfers of the store: parts of 64 MB, so that a large
// archive takes few parts of a multipart upload, which R2 can refuse to complete.
const awsConfig = `[default]
s3 =
  multipart_threshold = 64MB
  multipart_chunksize = 64MB
`;

// The file of those settings, which awsEnv names, in the job's temporary folder.
function awsConfigFile(context: Context): string {
  const file = join(context.temp, "ci-cache-s3", "aws-config");
  if (!existsSync(file)) {
    mkdirSync(join(context.temp, "ci-cache-s3"), { recursive: true });
    writeFileSync(file, awsConfig);
  }
  return file;
}

// The environment of the AWS CLI, with the store's keys, and checksums only where a
// request needs one, because not every provider takes the CLI's newer checksums.
export function awsEnv(settings: Settings, context: Context): Env {
  return {
    ...context.env,
    AWS_CONFIG_FILE: awsConfigFile(context),
    AWS_ACCESS_KEY_ID: settings.accessKeyId,
    AWS_SECRET_ACCESS_KEY: settings.secretAccessKey,
    AWS_DEFAULT_REGION: settings.region,
    AWS_REQUEST_CHECKSUM_CALCULATION: "when_required",
    AWS_RESPONSE_CHECKSUM_VALIDATION: "when_required",
    AWS_EC2_METADATA_DISABLED: "true",
  };
}

export function copy(
  settings: Settings,
  from: string,
  to: string,
  context: Context,
  extra: string[] = [],
): Result {
  return context.run(
    "aws",
    [
      "s3",
      "cp",
      from,
      to,
      "--endpoint-url",
      settings.endpoint,
      "--only-show-errors",
      "--no-progress",
      ...extra,
    ],
    { cwd: context.temp, env: awsEnv(settings, context) },
  );
}

// What the store holds of a key: its ETag, which each upload changes, the time it was
// written, in milliseconds, and the baseline in its metadata, or that it holds no such key,
// or that it couldn't tell.
export type Stored =
  | { state: "found"; etag?: string; modified?: number; baseline?: number }
  | { state: "absent" }
  | { state: "error"; why: string };

export function headObject(settings: Settings, key: string, context: Context): Stored {
  const head = context.run(
    "aws",
    [
      "s3api",
      "head-object",
      "--bucket",
      settings.bucket,
      "--key",
      key,
      "--endpoint-url",
      settings.endpoint,
    ],
    { cwd: context.temp, env: awsEnv(settings, context) },
  );
  if (head.status !== 0) {
    return /\(404\)|Not Found|does not exist/iu.test(head.stderr)
      ? { state: "absent" }
      : { state: "error", why: head.stderr.trim() };
  }
  try {
    const object: unknown = JSON.parse(head.stdout);
    const fields = isRecord(object) ? object : {};
    const metadata = isRecord(fields.Metadata) ? fields.Metadata : {};
    const baseline = Number(metadata.baseline);
    const modified = Date.parse(String(fields.LastModified));
    return {
      state: "found",
      ...(typeof fields.ETag === "string" ? { etag: fields.ETag } : {}),
      ...(Number.isSafeInteger(baseline) && baseline > 0 ? { baseline } : {}),
      ...(Number.isNaN(modified) ? {} : { modified }),
    };
  } catch {
    return { state: "found" };
  }
}

// Whether two values that two HEADs gave differ, where a value that one couldn't read tells
// nothing.
function differs(before: unknown, after: unknown): boolean {
  return before !== undefined && after !== undefined && before !== after;
}

// Whether another job wrote the cache's key after this job's restore, which then saves
// nothing, so that no two jobs upload one key at once, which R2 can refuse: a key where the
// restore found none, or one whose ETag, or else whose time of writing, differs from what
// the restore found, by the store's own values, so that no clock of a runner counts.
export function savedSince(settings: Settings, cache: Restored, context: Context): boolean {
  const now = headObject(settings, cache.key, context);
  const saved =
    now.state === "found" &&
    (!cache.found ||
      (cache.etag !== undefined && now.etag !== undefined
        ? differs(cache.etag, now.etag)
        : differs(cache.modified, now.modified)));
  if (!saved) {
    return false;
  }
  context.log(
    `ci-cache-s3: ${cache.name} finds ${cache.key} saved by another job since the restore, so it stays`,
  );
  return true;
}

// The waits before each further try of an upload: R2 now and then refuses to complete a
// multipart upload with InvalidPart, which a new upload of the same file passes.
const uploadWaits = [5000, 15_000];

// Uploads a cache's archive with its baseline, and tries again after a failure, unless
// another job saved the key meanwhile.
export async function upload(
  settings: Settings,
  cache: Restored,
  archive: string,
  sizes: { size: number; baseline: number },
  context: Context,
): Promise<void> {
  const wait =
    context.wait ??
    ((milliseconds: number) =>
      new Promise<void>((resolve) => {
        setTimeout(resolve, milliseconds);
      }));
  for (let attempt = 0; ; attempt += 1) {
    const sent = copy(settings, archive, `s3://${settings.bucket}/${cache.key}`, context, [
      "--metadata",
      `baseline=${String(sizes.baseline)}`,
    ]);
    if (sent.status === 0) {
      context.log(
        `ci-cache-s3: ${cache.name} saved ${cache.key}, ${String(sizes.size)} bytes, with the baseline ${String(sizes.baseline)}`,
      );
      return;
    }
    if (attempt >= uploadWaits.length) {
      context.log(
        `::warning::ci-cache-s3: ${cache.name} couldn't write ${cache.key}: ${sent.stderr.trim()}`,
      );
      return;
    }
    const pause = uploadWaits[attempt] ?? 0;
    context.log(
      `ci-cache-s3: ${cache.name} tries again in ${String(pause / 1000)} s to write ${cache.key}: ${sent.stderr.trim()}`,
    );
    await wait(pause);
    if (savedSince(settings, cache, context)) {
      return;
    }
  }
}
