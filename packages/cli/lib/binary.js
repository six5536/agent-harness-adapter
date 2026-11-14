"use strict";

// Maps the host platform to its prebuilt binary package. The names come from
// this package's own package.json, so this file names no project. Its functions
// take their inputs as parameters, so the tests run without the platform packages.

const manifest = require("../package.json");

const PLATFORM_SUFFIX = /-(linux|darwin|win32)-(x64|arm64)$/u;

/** Maps "<platform> <arch>" to the name of the package that holds that binary. */
function packagesFrom(optionalDependencies) {
  const packages = {};
  for (const name of Object.keys(optionalDependencies ?? {})) {
    const match = PLATFORM_SUFFIX.exec(name);
    if (match) {
      packages[`${match[1]} ${match[2]}`] = name;
    }
  }
  return packages;
}

/** Returns the name of the installed command, the first key of `bin`. */
function commandName(bin = manifest.bin) {
  return Object.keys(bin)[0];
}

function selectPackage(platform, arch, packages = packagesFrom(manifest.optionalDependencies)) {
  return packages[`${platform} ${arch}`] ?? null;
}

function binaryName(platform, command = commandName()) {
  return platform === "win32" ? `${command}.exe` : command;
}

// sokf:begin resolve-binary
/**
 * Returns the path of the binary for a platform. Each error tells the user what
 * to do. `requireResolve` defaults to Node's own `require.resolve`.
 */
function resolveBinary(
  platform,
  arch,
  requireResolve = require.resolve,
  packages = packagesFrom(manifest.optionalDependencies),
  command = commandName(),
) {
  const pkg = selectPackage(platform, arch, packages);
  const fromSource = `cargo install ${command}`;
  if (!pkg) {
    const supported = Object.keys(packages)
      .map((key) => key.replace(" ", "-"))
      .join(", ");
    throw new Error(
      `There is no prebuilt ${command} binary for ${platform}-${arch}.\n` +
        `${command} has prebuilt binaries for ${supported}.\n` +
        `Install from source instead: ${fromSource}`,
    );
  }
  try {
    return requireResolve(`${pkg}/bin/${binaryName(platform, command)}`);
  } catch {
    throw new Error(
      `The ${command} platform package "${pkg}" is not installed.\n` +
        `npm most likely skipped the optional dependencies when it installed ${command}.\n` +
        `Reinstall ${command} without --no-optional, or build from source: ${fromSource}`,
    );
  }
}
// sokf:end resolve-binary

// sokf:begin exit-code
/**
 * Maps a `spawnSync` result to this process's exit code. A process stopped by a
 * signal exits with 128 plus the signal's number, as shells report it.
 */
function exitCode(result, signals = require("node:os").constants.signals) {
  if (result.signal) {
    const signum = signals[result.signal];
    if (signum) {
      return 128 + signum;
    }
  }
  return result.status ?? 1;
}
// sokf:end exit-code

module.exports = { packagesFrom, commandName, selectPackage, binaryName, resolveBinary, exitCode };
