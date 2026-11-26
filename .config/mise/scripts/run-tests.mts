// Runs the fast tests, the long tests when given `long`, the tests that only CI can
// run when given `ci`, or the tests that only a developer's machine runs when given
// `local`, and warns if the fast, the long, or the local tests take longer than their
// limit. Outside CI, it skips the CI tier and says so, because a test of that tier can
// change the settings of the machine that it runs on. In CI, it skips the local tier
// and says so, because a test of that tier, such as a measure of scale, would cost its
// time on every platform of CI. In CI, it runs the tests with nextest's profile `ci`,
// which the Rust pack's `.config/nextest.toml` sets to report every failure rather
// than stop at the first.
//
// Usage: run-tests.mts [fast|long|ci|local]

import { spawnSync } from "node:child_process";
import process from "node:process";

export type TierRun =
  | { task: string; limit?: { variable: string; seconds: number } }
  | { skipped: string };

type Env = Record<string, string | undefined>;

// An empty limit counts as unset, and any other value that isn't a decimal number of
// seconds is refused, as Number would read a blank one as 0, which warns on every run,
// a word as NaN, which no time exceeds, and 0x10 or 1e9 as numbers.
function limited(task: string, variable: string, seconds: number, env: Env): TierRun {
  const text = env[variable] ?? "";
  if (text !== "" && !/^\s*\d+(?:\.\d+)?\s*$/u.test(text)) {
    throw new RangeError(`${variable} must be a number of seconds, not ${JSON.stringify(text)}`);
  }
  return { task, limit: { variable, seconds: text === "" ? seconds : Number(text) } };
}

// A CI system, such as GitHub Actions, sets CI, and a developer's shell doesn't. An
// empty value, false, or 0 counts as unset, as a developer sets CI=false to turn off a
// tool's CI mode.
function inCi(env: Env): boolean {
  return !["", "false", "0"].includes((env.CI ?? "").trim().toLowerCase());
}

// The environment of the tests that `env` starts: in CI, nextest's profile `ci`, unless
// the variable NEXTEST_PROFILE names another, because a CI run costs a run of every
// platform, and a failure that the first one hides costs another.
export function childEnv(env: Env): Env {
  if (!inCi(env) || env.NEXTEST_PROFILE !== undefined) {
    return { ...env };
  }
  return { ...env, NEXTEST_PROFILE: "ci" };
}

// What the tier `tier` runs in the environment `env`: the pattern of the tier's
// members, which mise runs together, or the message of a skipped tier.
export function tierRun(tier: string | undefined, env: Env): TierRun {
  switch (tier ?? "fast") {
    case "fast":
      return limited("test:fast:*", "TEST_FAST_TOTAL_SECONDS", 15, env);
    case "long":
      return limited("test:long:*", "TEST_LONG_TOTAL_SECONDS", 60, env);
    case "ci":
      if (!inCi(env)) {
        return {
          skipped:
            "test:ci: skipped the CI tier, because the variable CI is unset, empty, false, or 0; CI runs it.",
        };
      }
      return { task: "test:ci:*" };
    case "local": {
      // The limit is read first, so that a bad one fails in CI too.
      const run = limited("test:local:*", "TEST_LOCAL_TOTAL_SECONDS", 60, env);
      if (inCi(env)) {
        return {
          skipped:
            "test:local: skipped the local tier, because the variable CI is set; a developer's machine runs it.",
        };
      }
      return run;
    }
    default:
      throw new RangeError(`the tier must be fast, long, ci, or local, not ${tier}`);
  }
}

function main(): number {
  let run: TierRun;
  try {
    run = tierRun(process.argv[2], process.env);
  } catch (error) {
    console.error(`run-tests: ${error instanceof Error ? error.message : String(error)}`);
    return 2;
  }
  if ("skipped" in run) {
    console.log(run.skipped);
    return 0;
  }

  const start = performance.now();
  const result = spawnSync("mise", ["run", "-q", run.task], {
    env: childEnv(process.env),
    stdio: "inherit",
  });
  const seconds = (performance.now() - start) / 1000;
  if (result.error !== undefined) {
    console.error(`run-tests: mise run ${run.task} didn't start: ${result.error.message}`);
    return 1;
  }

  if (run.limit !== undefined && seconds > run.limit.seconds) {
    console.warn(
      `Warning: ${run.task} took ${seconds.toFixed(1)}s, over the limit of ${run.limit.seconds}s in ${run.limit.variable}.`,
    );
  }
  return result.status ?? 1;
}

// exitCode, not exit, so that a message to a pipe is written in full first.
if (import.meta.main) {
  process.exitCode = main();
}
