// Lists the crates of the Rust workspace from `cargo metadata`.

import { spawnSync } from "node:child_process";
import process from "node:process";

export interface Package {
  name: string;
  version: string;
  manifest: string;
  publish: unknown[] | null;
}

// Returns whether `value` is an object that is neither null nor an array.
function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

// Returns the package that the entry `p` of `cargo metadata` describes, or
// undefined when the entry lacks a field.
function toPackage(p: unknown): Package | undefined {
  if (
    !isRecord(p) ||
    typeof p.name !== "string" ||
    typeof p.version !== "string" ||
    typeof p.manifest_path !== "string"
  ) {
    return undefined;
  }
  const publish = Array.isArray(p.publish) ? p.publish : null;
  return { name: p.name, version: p.version, manifest: p.manifest_path, publish };
}

// Returns each crate of the workspace. `flags` go to `cargo metadata`, such as
// `--locked`. When cargo fails, or lists a crate without a name, a version, or
// a manifest, the process exits with the cause.
export function workspacePackages(...flags: string[]): Package[] {
  const result = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1", ...flags], {
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.status !== 0) {
    console.error(`cargo metadata failed${result.error ? `: ${result.error.message}` : ""}`);
    process.exit(1);
  }
  const metadata: unknown = JSON.parse(result.stdout);
  const packages: unknown[] =
    isRecord(metadata) && Array.isArray(metadata.packages) ? metadata.packages : [];
  const listed = packages.map((p) => toPackage(p));
  if (listed.includes(undefined)) {
    console.error("cargo metadata listed a crate without a name, a version, or a manifest");
    process.exit(1);
  }
  return listed.filter((p) => p !== undefined);
}
