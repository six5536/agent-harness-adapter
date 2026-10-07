"use strict";

// Finds the prebuilt addon for the host: the platform package npm installed
// as an optional dependency, or AHA_NODE_ADDON (a local build). Logic is
// dependency-injected so it is testable without the platform packages.

// @zen-impl: BND-4_AC-2
const PACKAGES = {
  "linux x64": "@six5536/agent-harness-adapter-node-linux-x64-gnu",
  "linux arm64": "@six5536/agent-harness-adapter-node-linux-arm64-gnu",
  "darwin x64": "@six5536/agent-harness-adapter-node-darwin-x64",
  "darwin arm64": "@six5536/agent-harness-adapter-node-darwin-arm64",
  "win32 x64": "@six5536/agent-harness-adapter-node-win32-x64",
};

const SUPPORTED = "linux-x64 (glibc), linux-arm64 (glibc), darwin-x64, darwin-arm64, win32-x64";

/** Whether this Linux runs on glibc (the addon is built against it). */
function isGlibc(report = () => process.report.getReport()) {
  try {
    return Boolean(report().header.glibcVersionRuntime);
  } catch {
    return false;
  }
}

/** The platform package for a host, or null if there is none. */
function selectPackage(platform, arch, glibc = true) {
  if (platform === "linux" && !glibc) {
    return null;
  }
  return PACKAGES[`${platform} ${arch}`] || null;
}

/** Load the addon; each error names the fix. */
function loadAddon({
  platform = process.platform,
  arch = process.arch,
  env = process.env,
  glibc = platform === "linux" ? isGlibc() : true,
  load = require,
} = {}) {
  if (env.AHA_NODE_ADDON) {
    return load(env.AHA_NODE_ADDON);
  }
  const pkg = selectPackage(platform, arch, glibc);
  if (!pkg) {
    throw new Error(
      `No prebuilt agent-harness-adapter addon for ${platform}-${arch}${glibc ? "" : " (musl)"}.\n` +
        `Supported platforms: ${SUPPORTED}.\n` +
        "The agent-harness-adapter command works everywhere: npm install -g @six5536/agent-harness-adapter",
    );
  }
  try {
    return load(`${pkg}/index.node`);
  } catch (err) {
    throw new Error(
      `The agent-harness-adapter addon package "${pkg}" could not be loaded (${err.message}).\n` +
        "Optional dependencies were most likely skipped during install; reinstall @six5536/agent-harness-adapter-node.",
    );
  }
}

module.exports = { PACKAGES, SUPPORTED, isGlibc, selectPackage, loadAddon };
