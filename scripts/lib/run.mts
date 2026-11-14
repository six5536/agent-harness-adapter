// The helpers that the project's scripts share: running a program and stopping
// at its failure, the paths of the debug build, and a Python environment of the
// project's own, in which a script installs what it builds.

import { spawnSync } from "node:child_process";
import type { SpawnSyncOptions } from "node:child_process";
import { rmSync } from "node:fs";
import { join, resolve } from "node:path";
import process from "node:process";

export const windows = process.platform === "win32";

// Cargo's target folder, which CARGO_TARGET_DIR moves.
export const targetDir = process.env.CARGO_TARGET_DIR ?? "target";

// The command-line program's name, which is also its crate's name.
export const command = "agent-harness-adapter";

// Stops the script with `message`, naming the script `name`.
export function fail(name: string, message: string): never {
  console.error(`${name}: ${message}`);
  process.exit(1);
}

// Runs `program` with `args`, its output on this process's own, and stops the
// script when the program fails to start or exits with a code other than 0.
export function run(
  name: string,
  program: string,
  args: string[],
  options: SpawnSyncOptions = {},
): void {
  const result = spawnSync(program, args, { stdio: "inherit", ...options });
  if (result.error !== undefined) {
    fail(name, `${program} didn't start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    const how =
      result.signal === null ? `exited with ${String(result.status)}` : `ended by ${result.signal}`;
    fail(name, `${program} ${args.join(" ")} ${how}`);
  }
}

// The absolute path of the debug build of the command, which `buildCommand`
// builds.
export function commandPath(): string {
  return resolve(targetDir, "debug", windows ? `${command}.exe` : command);
}

// Builds the debug build of the command, and returns its absolute path.
export function buildCommand(name: string): string {
  run(name, "cargo", ["build", "--locked", "-p", command]);
  return commandPath();
}

// Returns the Python of a virtual environment at `directory`, which it creates
// with the Python on the path when it has none that runs, such as after an
// upgrade of mise's Python, so that what a script installs stays out of every
// other environment.
export function venvPython(name: string, directory: string): string {
  const python = resolve(
    windows ? join(directory, "Scripts", "python.exe") : join(directory, "bin", "python"),
  );
  if (spawnSync(python, ["--version"], { stdio: "ignore" }).status !== 0) {
    run(name, "python", ["-m", "venv", "--clear", directory]);
  }
  return python;
}

// Builds the Python binding's wheel for the host, and installs it, and only
// it, in the virtual environment of `python`. The wheel goes to a folder of
// the script `name`'s own, so that two scripts can run at once.
export function installPythonBinding(name: string, python: string): void {
  const wheels = join(targetDir, `${name}-wheels`);
  rmSync(wheels, { recursive: true, force: true });
  run(name, "maturin", [
    "build",
    "--locked",
    "--manifest-path",
    "crates/bind/agent-harness-adapter-py/Cargo.toml",
    "--out",
    wheels,
    "--interpreter",
    python,
  ]);
  run(name, python, [
    "-m",
    "pip",
    "install",
    "--quiet",
    "--force-reinstall",
    "--no-index",
    "--find-links",
    wheels,
    "six5536-agent-harness-adapter",
  ]);
}
