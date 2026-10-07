"use strict";

// Maps the host platform to the prebuilt binary package and resolves the binary
// path. Logic is dependency-injected (platform, arch, requireResolve) so it is
// unit-testable without the platform packages actually being installed.

const PACKAGES = {
  "linux x64": "@six5536/agent-harness-kit-linux-x64",
  "linux arm64": "@six5536/agent-harness-kit-linux-arm64",
  "darwin x64": "@six5536/agent-harness-kit-darwin-x64",
  "darwin arm64": "@six5536/agent-harness-kit-darwin-arm64",
  "win32 x64": "@six5536/agent-harness-kit-win32-x64",
};

const SUPPORTED = Object.keys(PACKAGES)
  .map((k) => k.replace(" ", "-"))
  .join(", ");

/** Return the platform package name for a platform/arch, or null if unsupported. */
function selectPackage(platform, arch) {
  return PACKAGES[`${platform} ${arch}`] || null;
}

/** The binary file name inside a platform package. */
function binaryName(platform) {
  return platform === "win32" ? "ahk.exe" : "ahk";
}

/**
 * Resolve the absolute path to the prebuilt binary for the given platform/arch.
 * Throws if the platform has no binary, or if it has one but the package was
 * never installed — each error names the fix. `requireResolve` defaults to the
 * real `require.resolve`.
 */
function resolveBinary(platform, arch, requireResolve = require.resolve) {
  const pkg = selectPackage(platform, arch);
  if (!pkg) {
    throw new Error(
      `No prebuilt ahk binary for ${platform}-${arch}.\n` +
        `Supported platforms: ${SUPPORTED}.\n` +
        `Install from source instead: cargo install agent-harness-kit`,
    );
  }
  try {
    return requireResolve(`${pkg}/bin/${binaryName(platform)}`);
  } catch {
    throw new Error(
      `The ahk platform package "${pkg}" is not installed.\n` +
        `Optional dependencies were most likely skipped during install.\n` +
        `Reinstall @six5536/agent-harness-kit, or build from source: cargo install agent-harness-kit`,
    );
  }
}

/**
 * Map a `spawnSync` result to the exit code this process should use.
 *
 * A child killed by a signal reports `status === null` and `signal === "SIGINT"`
 * etc. Shells encode that as `128 + signum`, so a Ctrl-C'd run exits 130 rather
 * than a misleading 1. `signals` is injected for testability and defaults to
 * Node's own table.
 */
function exitCode(result, signals = require("node:os").constants.signals) {
  if (result.signal) {
    const signum = signals[result.signal];
    if (signum) {
      return 128 + signum;
    }
  }
  return result.status === null ? 1 : result.status;
}

module.exports = {
  selectPackage,
  binaryName,
  resolveBinary,
  exitCode,
  PACKAGES,
  SUPPORTED,
};
