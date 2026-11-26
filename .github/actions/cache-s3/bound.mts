// The bound of the size of ci-cache-s3's archives: the project's setting of it, and the
// size past which a save deletes an archive rather than write it, so that the next job
// builds the cache cold and the archive takes a new baseline.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

// The project's own settings of the pack, from the root of the repository.
export const settingsPath = ".config/ci-cache-s3/settings.project.json";

// The bound of an archive's size: a multiple of its baseline, the size of the archive that
// a save made after a cold start, or a fixed number of bytes.
export type Bound = { multiple: number } | { bytes: number };

export const defaultBound: Bound = { multiple: 2 };

const units = new Map(
  Object.entries({
    B: 1,
    KB: 1e3,
    MB: 1e6,
    GB: 1e9,
    TB: 1e12,
    KiB: 1024,
    MiB: 1024 ** 2,
    GiB: 1024 ** 3,
    TiB: 1024 ** 4,
  }),
);

// Reads the project's settings of the pack, and throws an error that names the file where
// they aren't an object whose only key, `bound`, is a multiple such as "3x" or a size
// such as "4 GiB".
export function parseSettings(text: string, file = settingsPath): { bound: Bound } {
  const fail = (why: string): never => {
    throw new Error(`${file}: ${why}`);
  };
  let settings: unknown;
  try {
    settings = JSON.parse(text);
  } catch (error) {
    return fail(`not JSON: ${error instanceof Error ? error.message : String(error)}`);
  }
  if (typeof settings !== "object" || settings === null || Array.isArray(settings)) {
    return fail("not an object of settings");
  }
  const unknown = Object.keys(settings).filter((key) => key !== "bound");
  if (unknown.length > 0) {
    return fail(
      `has ${unknown.join(", ")}, which the pack doesn't read; its only setting is bound`,
    );
  }
  const value: unknown = "bound" in settings ? settings.bound : undefined;
  if (value === undefined) {
    return { bound: defaultBound };
  }
  const bound = typeof value === "string" ? value.trim() : "";
  const multiple = /^(?<value>\d+(?:\.\d+)?)x$/u.exec(bound)?.groups?.value;
  if (multiple !== undefined && Number(multiple) > 1) {
    return { bound: { multiple: Number(multiple) } };
  }
  const size = /^(?<value>\d+(?:\.\d+)?) ?(?<unit>[KMGT]i?B|B)$/u.exec(bound)?.groups;
  const unit = units.get(size?.unit ?? "");
  if (size?.value !== undefined && unit !== undefined && Number(size.value) > 0) {
    return { bound: { bytes: Math.round(Number(size.value) * unit) } };
  }
  return fail(
    `bound is ${JSON.stringify(value)}, where it must be a multiple of the baseline above 1, such as "3x", or a size, such as "4 GiB" or "800 MB"`,
  );
}

// The project's bound of the archives, or the default where it sets none.
export function readBound(workspace: string): Bound {
  const file = join(workspace, settingsPath);
  return existsSync(file) ? parseSettings(readFileSync(file, "utf8")).bound : defaultBound;
}

// The size past which a save deletes the archive rather than write it.
export function limitOf(bound: Bound, baseline: number): number {
  return "multiple" in bound ? Math.floor(baseline * bound.multiple) : bound.bytes;
}

export function describe(bound: Bound, baseline: number): string {
  return "multiple" in bound
    ? `${String(bound.multiple)} times its baseline of ${String(baseline)} bytes`
    : `the fixed size of ${String(bound.bytes)} bytes`;
}
