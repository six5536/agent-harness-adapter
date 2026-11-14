// Publishes each crate of the workspace that crates.io doesn't have yet. With
// `needed`, prints whether any crate is still to publish.
//
// Usage: rust-publish.mts [needed]

import { spawnSync } from "node:child_process";
import process from "node:process";
import { type Package, workspacePackages } from "./lib/cargo-metadata.mts";

async function isPublished(crate: Package): Promise<boolean> {
  const url = `https://crates.io/api/v1/crates/${crate.name}/${crate.version}`;
  const response = await fetch(url, { headers: { "User-Agent": "pack-publish" } });
  return response.status === 200;
}

// A crate is publishable unless Cargo reports an empty list for `publish = false`.
const publishable = workspacePackages("--locked").filter((crate) => crate.publish?.length !== 0);

if (process.argv[2] === "needed") {
  console.log(publishable.length > 0 ? "true" : "false");
} else if ((process.env.PUBLISH_DISABLED ?? "") !== "") {
  console.error(`publish:rust: publishing is disabled here: ${process.env.PUBLISH_DISABLED ?? ""}`);
  process.exit(1);
} else if (publishable.length > 0) {
  const done = await Promise.all(publishable.map((crate) => isPublished(crate)));
  const excluded = publishable.filter((_, index) => done[index]).map((crate) => crate.name);
  if (excluded.length < publishable.length) {
    const args = [
      "publish",
      "--workspace",
      "--locked",
      ...excluded.flatMap((name) => ["--exclude", name]),
    ];
    if (process.env.PUBLISH_DRY_RUN === "true") {
      args.push("--dry-run");
    }
    const result = spawnSync("cargo", args, { stdio: "inherit" });
    if (result.status !== 0) {
      process.exit(1);
    }
  }
}
