// Finds the tasks whose names begin with a prefix, as steps, and runs a list of
// named steps, failing at the end with the name of each step that failed. The
// checks of the commit hook and of a release run their steps with it, and the lists
// of CI's jobs find their tasks with it.

import { spawnSync } from "node:child_process";
import process from "node:process";

export interface Step {
  name: string;
  command: string[];
  runs: boolean;
}

function isNamed(value: unknown): value is { name: string } {
  return (
    typeof value === "object" && value !== null && "name" in value && typeof value.name === "string"
  );
}

// The step that runs the task `name`.
export function taskStep(name: string): Step {
  return { name, command: ["mise", "run", "-q", name], runs: true };
}

export function taskSteps(prefix: string): Step[] {
  const listing = spawnSync("mise", ["tasks", "ls", "--json", "--hidden"], { encoding: "utf8" });
  if (listing.status !== 0) {
    const cause =
      listing.error?.message ??
      (listing.signal === null
        ? `exit ${String(listing.status)}: ${listing.stderr}`
        : `killed by ${listing.signal}`);
    throw new Error(`mise tasks ls failed: ${cause}`);
  }
  let tasks: unknown;
  try {
    tasks = JSON.parse(listing.stdout);
  } catch {
    throw new Error(`mise tasks ls printed no JSON: ${listing.stdout.slice(0, 200)}`);
  }
  const list: unknown[] = Array.isArray(tasks) ? tasks : [];
  return list
    .filter((task) => isNamed(task))
    .map((task) => task.name)
    .filter((name) => name.startsWith(prefix))
    .map((name) => taskStep(name));
}

// Returns whether `step` runs, given the names of the steps to skip.
export function willRun(step: Step, skipped: Set<string>): boolean {
  return step.runs && !skipped.has(step.name);
}

export function runSteps(steps: Step[], skipped: Set<string> = new Set()): void {
  const failed: string[] = [];
  for (const step of steps) {
    if (willRun(step, skipped)) {
      console.log(`==> ${step.name}`);
      const [command = "", ...args] = step.command;
      const result = spawnSync(command, args, { stdio: "inherit" });
      if (result.status !== 0) {
        failed.push(step.name);
      }
    }
  }
  if (failed.length > 0) {
    console.error(`Failed: ${failed.join(", ")}`);
    process.exit(1);
  }
}
