// Runs the Rust tests once with coverage, then fails when any crate of the
// workspace covers less than COVERAGE_MIN_LINES percent of its own lines, 90
// by default. A crate without lines to measure, such as a crate for Windows
// alone on Linux, passes with a note. A crate that another language tests sets
// `[package.metadata.coverage] skip = true` in its Cargo.toml, and the run
// leaves it out, so that it isn't built with every feature. The default filter
// of .config/nextest.toml keeps the CI and the local tiers out of the run.
// With `report`, the script instead runs the tests once with coverage, without
// the skipped crates, and writes the HTML report.
//
// Usage: rust-coverage-check.mts [report]

import { spawnSync } from "node:child_process";
import process from "node:process";
import { isRecord, type Package, workspacePackages } from "./lib/cargo-metadata.mts";

// The coverage builds every feature, so the code behind a feature counts. The
// build and each report fail on a stale Cargo.lock, and the clean leaves it as
// it is. A report takes no features, which only the build reads.
const lockedFlags = ["--locked"];
const buildFlags = ["--all-features", ...lockedFlags];

export interface Lines {
  count: number;
  covered: number;
  percent: number;
}

// Runs cargo with `args`, and returns its exit status and, where `capture` is
// true, its standard output, which otherwise goes to the terminal.
export type Cargo = (args: string[], capture: boolean) => { status: number | null; stdout: string };

// Returns whether the crate `p` sets `package.metadata.coverage.skip` to true.
function isSkipped(p: Package): boolean {
  return isRecord(p.metadata) && isRecord(p.metadata.coverage) && p.metadata.coverage.skip === true;
}

// Returns the names of the crates to measure: each crate that isn't skipped.
export function measuredCrates(packages: Package[]): string[] {
  return packages.filter((p) => !isSkipped(p)).map((p) => p.name);
}

// Returns the names of the crates that set `package.metadata.coverage.skip`
// to true, which the run of the tests leaves out.
export function skippedCrates(packages: Package[]): string[] {
  return packages.filter((p) => isSkipped(p)).map((p) => p.name);
}

// The arguments of cargo that leave each crate of `skipped` out of the run.
function excludeArgs(skipped: string[]): string[] {
  return skipped.flatMap((name) => ["--exclude", name]);
}

// Returns the minimum of line coverage that the variable `value` sets, 90 when
// it is unset or empty, as for ${COVERAGE_MIN_LINES:-90} in a shell and in the
// Vitest base of the typescript pack. A value that isn't a percentage throws,
// a blank one too, which Number would read as 0.
export function minimumLines(value?: string): number {
  if (value === undefined || value === "") {
    return 90;
  }
  const minimum = value.trim() === "" ? Number.NaN : Number(value);
  if (!Number.isFinite(minimum) || minimum < 0 || minimum > 100) {
    throw new Error(
      `COVERAGE_MIN_LINES is ${JSON.stringify(value)}, not a percentage from 0 to 100`,
    );
  }
  return minimum;
}

// The toolchain argument of cargo, such as `+nightly-2026-11-05`, or none.
function toolchainArgs(toolchain: string | undefined): string[] {
  return toolchain === undefined || toolchain === "" ? [] : [`+${toolchain}`];
}

// The arguments of cargo that remove the coverage of an earlier run. Without
// `--locked`, the clean would rewrite a stale Cargo.lock before the build. With
// it, cargo-llvm-cov only warns and exits 0, and the build then fails.
export function cleanArgs(toolchain: string | undefined): string[] {
  return [...toolchainArgs(toolchain), "llvm-cov", "clean", "--workspace", ...lockedFlags];
}

// The arguments of cargo that run the tests once with coverage, without the
// crates of `skipped`, and then pass `output`, the choice of the report.
function runArgs(toolchain: string | undefined, skipped: string[], output: string): string[] {
  return [
    ...toolchainArgs(toolchain),
    "llvm-cov",
    "nextest",
    "--workspace",
    ...excludeArgs(skipped),
    ...buildFlags,
    "--run-ignored",
    "all",
    output,
  ];
}

// The arguments of cargo that run the tests once with coverage, without the
// crates of `skipped`, and write no report.
export function testArgs(toolchain: string | undefined, skipped: string[]): string[] {
  return runArgs(toolchain, skipped, "--no-report");
}

// The arguments of cargo that run the tests once with coverage, without the
// crates of `skipped`, and write the HTML report, which `--exclude` keeps to
// the other crates.
export function htmlArgs(toolchain: string | undefined, skipped: string[]): string[] {
  return runArgs(toolchain, skipped, "--html");
}

// The arguments of cargo that print the summary of the crate `name` as JSON.
// `--package` keeps only the crate's own files, the lines that the tests of
// every crate ran, and no file of a crate nested in its folder.
export function reportArgs(toolchain: string | undefined, name: string): string[] {
  return [
    ...toolchainArgs(toolchain),
    "llvm-cov",
    "report",
    "--json",
    "--summary-only",
    "--package",
    name,
    ...lockedFlags,
  ];
}

// Returns the totals of the lines from the JSON summary `text` of a report.
export function lineCoverage(text: string): Lines {
  let report: unknown;
  try {
    report = JSON.parse(text);
  } catch {
    report = undefined;
  }
  const data =
    isRecord(report) && Array.isArray(report.data) ? (report.data[0] as unknown) : undefined;
  const lines = isRecord(data) && isRecord(data.totals) ? data.totals.lines : undefined;
  if (
    !isRecord(lines) ||
    typeof lines.count !== "number" ||
    typeof lines.covered !== "number" ||
    typeof lines.percent !== "number"
  ) {
    throw new Error("the report holds no line coverage");
  }
  return { count: lines.count, covered: lines.covered, percent: lines.percent };
}

// Returns `percent` with two decimals, rounded down, so that a crate below the
// minimum never shows it. The product is rounded to six decimals first, so
// that an error of floating point, such as 64.35 * 100 = 6434.999…, doesn't
// take a hundredth off.
function percentText(percent: number): string {
  return (Math.floor(Math.round(percent * 100 * 1e6) / 1e6) / 100).toFixed(2);
}

// Returns the message for the crate `name`, which has lines to measure, when
// its `lines` fall below `minimum`, or undefined when they reach it.
export function shortfall(name: string, lines: Lines, minimum: number): string | undefined {
  if (lines.percent >= minimum) {
    return undefined;
  }
  return `${name} covers ${percentText(lines.percent)}% of its ${String(lines.count)} lines, below the minimum of ${String(minimum)}%`;
}

// The line of a crate without lines to measure, which passes. The note names
// the causes, because one of them, code that no test binary holds, is a fault
// of the crate's configuration.
function noLinesNote(name: string): string {
  return `${name}: no lines to measure: it holds no code that the coverage can count, such as a crate of types alone or a crate for another platform, or no test binary holds its code, which is a fault where its targets set test = false and no other crate's tests use it`;
}

// Splits `packages` into the names of the crates to measure and of those to skip.
function split(packages: Package[]): { measured: string[]; skipped: string[] } {
  return { measured: measuredCrates(packages), skipped: skippedCrates(packages) };
}

// Logs a line for each crate of `skipped`.
function logSkipped(skipped: string[], log: (line: string) => void): void {
  for (const name of skipped) {
    log(`${name}: skipped, as its Cargo.toml sets skip = true in [package.metadata.coverage]`);
  }
}

// Returns whether `measured` is empty, after it logs the crates of `skipped`
// and that no test runs, because cargo-llvm-cov refuses a run that leaves out
// every crate.
function nothingToMeasure(
  { measured, skipped }: { measured: string[]; skipped: string[] },
  log: (line: string) => void,
): boolean {
  if (measured.length > 0) {
    return false;
  }
  logSkipped(skipped, log);
  log("no crate is left to measure, so no Rust test runs with coverage");
  return true;
}

export interface Check {
  packages: Package[];
  cargo: Cargo;
  minimum: number;
  toolchain: string | undefined;
  log: (line: string) => void;
  error: (line: string) => void;
}

// Returns the line of the crate `name` from the `result` of its report: a
// failure when the crate falls below `minimum` or the report fails, and
// otherwise its coverage or the note of a crate without lines.
function judge(
  name: string,
  result: ReturnType<Cargo>,
  minimum: number,
): { failed: boolean; line: string } {
  if (result.status !== 0) {
    return { failed: true, line: `${name}: cargo llvm-cov report failed` };
  }
  let lines: Lines;
  try {
    lines = lineCoverage(result.stdout);
  } catch (cause) {
    const reason = cause instanceof Error ? cause.message : String(cause);
    return { failed: true, line: `${name}: ${reason}` };
  }
  if (lines.count === 0) {
    return { failed: false, line: noLinesNote(name) };
  }
  const message = shortfall(name, lines, minimum);
  return message === undefined
    ? {
        failed: false,
        line: `${name}: ${percentText(lines.percent)}% of ${String(lines.count)} lines`,
      }
    : { failed: true, line: message };
}

// Runs the tests once without the skipped crates, then checks each measured
// crate against the minimum, and returns the exit code: 1 when the tests fail
// or any crate fails. The skipped crates are named after cargo's output.
export function checkCoverage(check: Check): number {
  const { cargo, error, log, minimum, toolchain } = check;
  const crates = split(check.packages);
  if (nothingToMeasure(crates, log)) {
    return 0;
  }
  const { measured, skipped } = crates;
  const cleaned = cargo(cleanArgs(toolchain), false).status === 0;
  const tested = cleaned && cargo(testArgs(toolchain, skipped), false).status === 0;
  logSkipped(skipped, log);
  if (!cleaned) {
    error("cargo llvm-cov clean failed");
    return 1;
  }
  if (!tested) {
    error("the build or the tests failed, so no crate's coverage was checked");
    return 1;
  }
  let failed = false;
  for (const name of measured) {
    const verdict = judge(name, cargo(reportArgs(toolchain, name), true), minimum);
    if (verdict.failed) {
      error(verdict.line);
      failed = true;
    } else {
      log(verdict.line);
    }
  }
  return failed ? 1 : 0;
}

export type Html = Omit<Check, "minimum">;

// Runs the tests once without the skipped crates and writes the HTML report,
// and returns the exit code: 1 when the run fails. The skipped crates are
// named after cargo's output.
export function writeHtml(html: Html): number {
  const crates = split(html.packages);
  if (nothingToMeasure(crates, html.log)) {
    return 0;
  }
  const { skipped } = crates;
  const status = html.cargo(htmlArgs(html.toolchain, skipped), false).status;
  logSkipped(skipped, html.log);
  if (status !== 0) {
    html.error("the build or the tests failed, so the HTML report wasn't written");
    return 1;
  }
  return 0;
}

// Runs cargo in this process's terminal, and captures its standard output
// where `capture` is true.
function spawnCargo(args: string[], capture: boolean): ReturnType<Cargo> {
  const result = spawnSync("cargo", args, {
    encoding: "utf8",
    maxBuffer: 256 * 1024 * 1024,
    stdio: ["ignore", capture ? "pipe" : "inherit", "inherit"],
  });
  if (result.error !== undefined) {
    console.error(`cargo failed to start: ${result.error.message}`);
  }
  return { status: result.status, stdout: capture ? result.stdout : "" };
}

// Runs the mode that the arguments `args` name, and returns the exit code.
function main(args: string[]): number {
  const mode = args.at(0);
  if (args.length > 1 || (mode !== undefined && mode !== "report")) {
    console.error("usage: rust-coverage-check.mts [report]");
    return 2;
  }
  const toolchain = process.env.RUST_NIGHTLY;
  const output = { log: console.log, error: console.error };
  if (mode === "report") {
    return writeHtml({
      packages: workspacePackages("--locked"),
      cargo: spawnCargo,
      toolchain,
      ...output,
    });
  }
  let minimum: number;
  try {
    minimum = minimumLines(process.env.COVERAGE_MIN_LINES);
  } catch (cause) {
    console.error(cause instanceof Error ? cause.message : String(cause));
    return 1;
  }
  const packages = workspacePackages("--locked");
  return checkCoverage({ packages, cargo: spawnCargo, minimum, toolchain, ...output });
}

if (import.meta.main) {
  process.exitCode = main(process.argv.slice(2));
}
