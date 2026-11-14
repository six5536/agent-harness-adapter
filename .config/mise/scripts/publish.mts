// Runs each task named publish:<name>, first all as dry runs and then for real,
// with publish:github last.

import { spawnSync } from "node:child_process";
import process from "node:process";

function isNamed(value: unknown): value is { name: string } {
  return (
    typeof value === "object" && value !== null && "name" in value && typeof value.name === "string"
  );
}

function publishTasks(): string[] {
  const listing = spawnSync("mise", ["tasks", "ls", "--json", "--hidden"], { encoding: "utf8" });
  const tasks: unknown = JSON.parse(listing.stdout);
  const list: unknown[] = Array.isArray(tasks) ? tasks : [];
  const names = list
    .filter((task) => isNamed(task))
    .map((task) => task.name)
    .filter((name) => /^publish:[^:]+$/u.test(name));
  const last = "publish:github";
  return [
    ...names.filter((name) => name !== last).toSorted(),
    ...names.filter((name) => name === last),
  ];
}

function runAll(tasks: string[], dryRun: boolean): void {
  for (const task of tasks) {
    const env = dryRun ? { ...process.env, PUBLISH_DRY_RUN: "true" } : process.env;
    const result = spawnSync("mise", ["run", "-q", task], { env, stdio: "inherit" });
    if (result.status !== 0) {
      console.error(`publish: ${task} failed`);
      process.exit(1);
    }
  }
}

// A pack or a project sets PUBLISH_DISABLED where nothing may be published, and
// gives the reason as its value.
const disabled = process.env.PUBLISH_DISABLED ?? "";
if (disabled !== "") {
  console.error(`publish: publishing is disabled here: ${disabled}`);
  process.exit(1);
}

// Every dry run comes first, because a publish can't be undone.
const tasks = publishTasks();
runAll(tasks, true);
if (process.env.PUBLISH_DRY_RUN !== "true") {
  runAll(tasks, false);
}
