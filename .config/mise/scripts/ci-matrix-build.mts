// Prints as JSON the jobs that build.yml runs: one for each task named
// build:release:<runner>:<name>, on the runner <runner>-latest, with an artifact
// named after the task.
//
// Usage: ci-matrix-build.mts

import { taskSteps } from "./lib/check-steps.mts";

export interface BuildJob {
  task: string;
  runner: string;
  artifact: string;
}

// The job of the task `task`, whose name begins with build:release:.
export function buildJob(task: string): BuildJob {
  const runner = task.split(":")[2] ?? "";
  return { task, runner: `${runner}-latest`, artifact: task.replaceAll(":", "-") };
}

if (import.meta.main) {
  console.log(JSON.stringify(taskSteps("build:release:").map((step) => buildJob(step.name))));
}
