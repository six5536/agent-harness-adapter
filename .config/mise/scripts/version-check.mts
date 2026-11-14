// Fails if the versions that the version:list:<name> tasks report differ from one
// another, or from the version given, or if a version is given and no task reports
// one.
//
// Usage: version-check.mts [version]

import { spawnSync } from "node:child_process";
import process from "node:process";

function isNamed(value: unknown): value is { name: string } {
  return (
    typeof value === "object" && value !== null && "name" in value && typeof value.name === "string"
  );
}

function versionListTasks(): string[] {
  const listing = spawnSync("mise", ["tasks", "ls", "--json", "--hidden"], { encoding: "utf8" });
  const tasks: unknown = JSON.parse(listing.stdout);
  const list: unknown[] = Array.isArray(tasks) ? tasks : [];
  return list
    .filter((task) => isNamed(task))
    .map((task) => task.name)
    .filter((name) => name.startsWith("version:list:"));
}

function reportedVersions(): Map<string, string> {
  const versions = new Map<string, string>();
  for (const task of versionListTasks()) {
    const result = spawnSync("mise", ["run", "-q", task], { encoding: "utf8" });
    if (result.status !== 0) {
      console.error(`version:check: ${task} failed: ${result.stderr}`);
      process.exit(1);
    }
    for (const line of result.stdout.split("\n").filter((text) => text.includes(" "))) {
      const separator = line.lastIndexOf(" ");
      versions.set(line.slice(0, separator), line.slice(separator + 1));
    }
  }
  return versions;
}

// Returns what is wrong with the reported versions, or undefined when they agree.
export function versionProblem(
  versions: Map<string, string>,
  expected?: string,
): string | undefined {
  // A release must not pass because nothing states its version.
  if (expected !== undefined && versions.size === 0) {
    return `no task reports a version to compare with ${expected}. A task named version:list:<name> prints a file and its version on each line.`;
  }
  const distinct = new Set(versions.values());
  if (expected !== undefined) {
    distinct.add(expected);
  }
  if (distinct.size <= 1) {
    return undefined;
  }
  const heading =
    expected === undefined
      ? "the versions differ:"
      : `the versions differ, or differ from ${expected}:`;
  return [heading, ...[...versions].map(([file, version]) => `  ${file} ${version}`)].join("\n");
}

if (import.meta.main) {
  const problem = versionProblem(reportedVersions(), process.argv.at(2));
  if (problem !== undefined) {
    console.error(`version:check: ${problem}`);
    process.exit(1);
  }
}
