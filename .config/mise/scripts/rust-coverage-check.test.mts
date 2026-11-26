// Tests how rust-coverage-check.mts chooses the crates, builds cargo's
// arguments, reads each crate's report, and judges it, without running cargo.

import assert from "node:assert/strict";
import { type SpawnSyncReturns, spawnSync } from "node:child_process";
import process from "node:process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import type { Package } from "./lib/cargo-metadata.mts";
import {
  type Cargo,
  checkCoverage,
  cleanArgs,
  htmlArgs,
  lineCoverage,
  measuredCrates,
  minimumLines,
  reportArgs,
  shortfall,
  skippedCrates,
  testArgs,
  writeHtml,
} from "./rust-coverage-check.mts";

function crate(name: string, metadata: unknown = null): Package {
  return {
    name,
    version: "0.1.0",
    manifest: `/w/crates/${name}/Cargo.toml`,
    publish: null,
    metadata,
  };
}

// The JSON that `cargo llvm-cov report --json --summary-only` prints for a
// crate with `count` lines, of which the tests ran `covered`.
function summary(count: number, covered: number): string {
  const percent = count === 0 ? 0 : (covered / count) * 100;
  return JSON.stringify({
    data: [{ files: [], totals: { lines: { count, covered, percent } } }],
    type: "llvm.coverage.json.export",
  });
}

test("a crate whose manifest sets coverage.skip to true is left out", () => {
  const packages = [
    crate("lib"),
    crate("py", { coverage: { skip: true } }),
    crate("app", { coverage: { skip: false } }),
    crate("node", { coverage: { skip: "true" } }),
    crate("docs", { other: 1 }),
  ];
  assert.deepEqual(measuredCrates(packages), ["lib", "app", "node", "docs"]);
  assert.deepEqual(skippedCrates(packages), ["py"]);
});

test("the minimum is 90 where COVERAGE_MIN_LINES is unset or empty", () => {
  assert.equal(minimumLines(), 90);
  assert.equal(minimumLines(""), 90);
  assert.equal(minimumLines("75.5"), 75.5);
  assert.equal(minimumLines("0"), 0);
});

test("a minimum that isn't a percentage names the variable", () => {
  for (const value of ["ninety", "-1", "101", "Infinity", " "]) {
    assert.throws(() => minimumLines(value), /COVERAGE_MIN_LINES/u, value);
  }
});

test("the clean and the tests keep the locked dependencies, and the tests run once with every feature, without a report", () => {
  assert.deepEqual(testArgs("nightly-2026-11-05", []), [
    "+nightly-2026-11-05",
    "llvm-cov",
    "nextest",
    "--workspace",
    "--all-features",
    "--locked",
    "--run-ignored",
    "all",
    "--no-report",
  ]);
  assert.deepEqual(cleanArgs("nightly-2026-11-05"), [
    "+nightly-2026-11-05",
    "llvm-cov",
    "clean",
    "--workspace",
    "--locked",
  ]);
});

test("the tests leave out each skipped crate", () => {
  assert.deepEqual(testArgs("nightly-2026-11-05", ["py", "node"]).slice(3, 8), [
    "--workspace",
    "--exclude",
    "py",
    "--exclude",
    "node",
  ]);
});

test("the HTML report runs the tests once with every feature and the locked dependencies, without the skipped crates", () => {
  assert.deepEqual(htmlArgs("nightly-2026-11-05", ["py"]), [
    "+nightly-2026-11-05",
    "llvm-cov",
    "nextest",
    "--workspace",
    "--exclude",
    "py",
    "--all-features",
    "--locked",
    "--run-ignored",
    "all",
    "--html",
  ]);
});

test("each report covers the files of one crate, and refuses a stale Cargo.lock", () => {
  assert.deepEqual(reportArgs("nightly-2026-11-05", "my-core"), [
    "+nightly-2026-11-05",
    "llvm-cov",
    "report",
    "--json",
    "--summary-only",
    "--package",
    "my-core",
    "--locked",
  ]);
});

test("without a nightly toolchain, cargo runs its own", () => {
  for (const toolchain of [undefined, ""]) {
    assert.equal(cleanArgs(toolchain)[0], "llvm-cov");
    assert.equal(testArgs(toolchain, [])[0], "llvm-cov");
    assert.equal(htmlArgs(toolchain, [])[0], "llvm-cov");
    assert.equal(reportArgs(toolchain, "my-core")[0], "llvm-cov");
  }
});

test("a minimum that isn't a percentage stops the script before cargo runs", () => {
  const result = script();
  assert.equal(result.status, 1);
  assert.match(result.stderr, /COVERAGE_MIN_LINES is "ninety"/u);
});

// Runs the script with `args` where no cargo can start, and a minimum that
// isn't a percentage.
function script(...args: string[]): SpawnSyncReturns<string> {
  const path = fileURLToPath(new URL("rust-coverage-check.mts", import.meta.url));
  return spawnSync(process.execPath, [path, ...args], {
    encoding: "utf8",
    env: { ...process.env, COVERAGE_MIN_LINES: "ninety", PATH: "" },
  });
}

test("with report, the script reads the crates and ignores the minimum", () => {
  const result = script("report");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /cargo metadata failed/u);
  assert.doesNotMatch(result.stderr, /COVERAGE_MIN_LINES/u);
});

test("another argument stops the script with its usage before cargo runs", () => {
  for (const args of [["reprot"], ["report", "extra"]]) {
    const result = script(...args);
    assert.equal(result.status, 2, args.join(" "));
    assert.match(result.stderr, /^usage: rust-coverage-check\.mts \[report\]/u);
  }
});

test("the line coverage comes from the totals of the report", () => {
  assert.deepEqual(lineCoverage(summary(200, 150)), { count: 200, covered: 150, percent: 75 });
});

test("a report without the totals of its lines is refused", () => {
  for (const text of ["", "{}", '{"data":[]}', '{"data":[{"totals":{}}]}', "not json"]) {
    assert.throws(() => lineCoverage(text), /no line coverage/u, text);
  }
});

test("a crate below the minimum is named with its coverage and the minimum", () => {
  assert.equal(
    shortfall("my-app", { count: 3, covered: 2, percent: 66.666_666 }, 90),
    "my-app covers 66.66% of its 3 lines, below the minimum of 90%",
  );
  assert.equal(shortfall("my-app", { count: 10, covered: 9, percent: 90 }, 90), undefined);
});

test("a coverage of two decimals shows as it is", () => {
  assert.deepEqual(
    run([crate("lib")], {
      "report lib": {
        status: 0,
        stdout: JSON.stringify({
          data: [{ totals: { lines: { count: 2000, covered: 1287, percent: 64.35 } } }],
        }),
      },
    }).err,
    ["lib covers 64.35% of its 2000 lines, below the minimum of 90%"],
  );
});

test("a crate whose coverage only rounds up to the minimum is below it", () => {
  assert.match(
    shortfall("my-app", { count: 100_000, covered: 89_999, percent: 89.999 }, 90) ?? "",
    /89\.99% .* 90%/u,
  );
});

interface Run {
  calls: string[][];
  out: string[];
  err: string[];
  code: number;
}

// Runs the check with a cargo that answers each call from `answers`, keyed by
// the subcommand of llvm-cov and, for a report, the crate.
function run(
  packages: Package[],
  answers: Record<string, { status: number; stdout?: string }>,
  minimum = 90,
): Run {
  const result: Run = { calls: [], out: [], err: [], code: 0 };
  const cargo: Cargo = (args) => {
    result.calls.push(args);
    const sub = args[args.indexOf("llvm-cov") + 1] ?? "";
    const key = sub === "report" ? `report ${args[args.indexOf("--package") + 1] ?? ""}` : sub;
    const answer = answers[key] ?? { status: 0 };
    return { status: answer.status, stdout: answer.stdout ?? "" };
  };
  result.code = checkCoverage({
    packages,
    cargo,
    minimum,
    toolchain: "nightly",
    log: (line) => {
      result.out.push(line);
    },
    error: (line) => {
      result.err.push(line);
    },
  });
  return result;
}

test("the tests run once, and each measured crate gets a report of its own", () => {
  const result = run([crate("lib"), crate("py", { coverage: { skip: true } }), crate("app")], {
    "report lib": { status: 0, stdout: summary(10, 10) },
    "report app": { status: 0, stdout: summary(10, 9) },
  });
  assert.equal(result.code, 0);
  assert.deepEqual(result.out, [
    "py: skipped, as its Cargo.toml sets skip = true in [package.metadata.coverage]",
    "lib: 100.00% of 10 lines",
    "app: 90.00% of 10 lines",
  ]);
  assert.deepEqual(
    result.calls.map((args) => args.slice(1, 3).join(" ")),
    ["llvm-cov clean", "llvm-cov nextest", "llvm-cov report", "llvm-cov report"],
  );
  assert.deepEqual(
    result.calls.slice(2).map((args) => args[args.indexOf("--package") + 1]),
    ["lib", "app"],
  );
  assert.deepEqual(result.calls[1]?.slice(3, 6), ["--workspace", "--exclude", "py"]);
  assert.deepEqual(result.err, []);
});

test("a crate without lines to measure passes with a note that names it", () => {
  const result = run([crate("my-win"), crate("lib")], {
    "report my-win": { status: 0, stdout: summary(0, 0) },
    "report lib": { status: 0, stdout: summary(10, 10) },
  });
  assert.equal(result.code, 0);
  assert.deepEqual(result.out, [
    "my-win: no lines to measure: it holds no code that the coverage can count, such as a crate of types alone or a crate for another platform, or no test binary holds its code, which is a fault where its targets set test = false and no other crate's tests use it",
    "lib: 100.00% of 10 lines",
  ]);
  assert.deepEqual(result.err, []);
});

test("one crate below the minimum fails the check after every report", () => {
  const result = run([crate("lib"), crate("app"), crate("cli")], {
    "report lib": { status: 0, stdout: summary(10, 5) },
    "report app": { status: 0, stdout: summary(10, 10) },
    "report cli": { status: 0, stdout: summary(10, 8) },
  });
  assert.equal(result.code, 1);
  assert.equal(result.calls.length, 5);
  assert.deepEqual(result.err, [
    "lib covers 50.00% of its 10 lines, below the minimum of 90%",
    "cli covers 80.00% of its 10 lines, below the minimum of 90%",
  ]);
});

test("no test runs where every crate is skipped, and the check passes and says so", () => {
  const result = run([crate("py", { coverage: { skip: true } })], {});
  assert.equal(result.code, 0);
  assert.deepEqual(result.calls, []);
  assert.deepEqual(result.out, [
    "py: skipped, as its Cargo.toml sets skip = true in [package.metadata.coverage]",
    "no crate is left to measure, so no Rust test runs with coverage",
  ]);
});

test("a failed test run fails the check without a report, and names the skipped crates", () => {
  const result = run([crate("lib"), crate("py", { coverage: { skip: true } })], {
    nextest: { status: 100 },
  });
  assert.equal(result.code, 1);
  assert.equal(result.calls.length, 2);
  assert.match(result.err.join("\n"), /tests failed/u);
  assert.match(result.out.join("\n"), /^py: skipped/u);
});

test("no test runs where the workspace lists no crate", () => {
  const result = run([], {});
  assert.equal(result.code, 0);
  assert.deepEqual(result.calls, []);
  assert.deepEqual(result.out, ["no crate is left to measure, so no Rust test runs with coverage"]);
});

test("a failed clean fails the check before the tests, and names the skipped crates", () => {
  const result = run([crate("lib"), crate("py", { coverage: { skip: true } })], {
    clean: { status: 1 },
  });
  assert.equal(result.code, 1);
  assert.equal(result.calls.length, 1);
  assert.deepEqual(result.err, ["cargo llvm-cov clean failed"]);
  assert.match(result.out.join("\n"), /^py: skipped/u);
});

test("a crate without lines to measure passes at any minimum", () => {
  for (const minimum of [0, 100]) {
    const result = run(
      [crate("my-win")],
      { "report my-win": { status: 0, stdout: summary(0, 0) } },
      minimum,
    );
    assert.equal(result.code, 0, String(minimum));
    assert.match(result.out.join("\n"), /^my-win: no lines to measure/u);
  }
});

test("a report that fails or prints no summary fails the check and names the crate", () => {
  const result = run([crate("lib"), crate("app")], {
    "report lib": { status: 1 },
    "report app": { status: 0, stdout: "{}" },
  });
  assert.equal(result.code, 1);
  assert.equal(result.err.length, 2);
  assert.match(result.err[0] ?? "", /^lib: /u);
  assert.match(result.err[1] ?? "", /^app: /u);
});

// Writes the HTML report with a cargo that answers every call with `status`.
function html(packages: Package[], status: number): Run {
  const result: Run = { calls: [], out: [], err: [], code: 0 };
  result.code = writeHtml({
    packages,
    cargo: (args) => {
      result.calls.push(args);
      return { status, stdout: "" };
    },
    toolchain: "nightly",
    log: (line) => {
      result.out.push(line);
    },
    error: (line) => {
      result.err.push(line);
    },
  });
  return result;
}

test("the HTML report runs the tests once without the skipped crates, and names them", () => {
  const result = html([crate("lib"), crate("py", { coverage: { skip: true } })], 0);
  assert.equal(result.code, 0);
  assert.deepEqual(result.calls, [htmlArgs("nightly", ["py"])]);
  assert.deepEqual(result.out, [
    "py: skipped, as its Cargo.toml sets skip = true in [package.metadata.coverage]",
  ]);
});

test("a failed run of the HTML report fails the task with a message", () => {
  const result = html([crate("lib"), crate("py", { coverage: { skip: true } })], 101);
  assert.equal(result.code, 1);
  assert.deepEqual(result.err, [
    "the build or the tests failed, so the HTML report wasn't written",
  ]);
  assert.match(result.out.join("\n"), /^py: skipped/u);
});

test("no HTML report is written where every crate is skipped", () => {
  const result = html([crate("py", { coverage: { skip: true } })], 0);
  assert.equal(result.code, 0);
  assert.deepEqual(result.calls, []);
  assert.equal(
    result.out.at(-1),
    "no crate is left to measure, so no Rust test runs with coverage",
  );
});
