// Fails when a crate of the Rust workspace doesn't inherit the workspace lints,
// because a lint table applies only to a crate that opts in to it.

import { readFileSync } from "node:fs";
import { relative } from "node:path";
import process from "node:process";
// The shared TypeScript tools hold the parser, outside the folders that Node
// searches for this script's packages.
import { parse } from "../../typescript/node_modules/smol-toml/dist/index.js";
import { workspacePackages } from "./lib/cargo-metadata.mts";

// Returns whether the manifest `text` opts in to the workspace lints with
// `workspace = true` in its `lints` table.
export function inherits(text: string): boolean {
  const lints = parse(text).lints;
  return typeof lints === "object" && "workspace" in lints && lints.workspace === true;
}

if (import.meta.main) {
  const missing = workspacePackages().filter((c) => !inherits(readFileSync(c.manifest, "utf8")));
  for (const c of missing) {
    console.error(
      `${c.name} (${relative(process.cwd(), c.manifest)}) doesn't inherit the workspace lints: add \`[lints]\` with \`workspace = true\`, and move the crate's own lint tables into \`[workspace.lints]\``,
    );
  }
  process.exitCode = missing.length > 0 ? 1 : 0;
}
