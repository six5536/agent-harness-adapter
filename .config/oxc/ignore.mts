// The project's own patterns of files that oxlint and oxfmt leave out, from
// .config/oxc/ignore.project.json, which no pack supplies, so a project lists
// them without editing a pack's file. The file holds a JSON list of glob
// patterns, relative to the repository root, such as ["archive/**"].

import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const file = fileURLToPath(new URL("ignore.project.json", import.meta.url));

function isPatternList(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === "string");
}

export function projectIgnorePatterns(): string[] {
  if (!existsSync(file)) {
    return [];
  }
  const value: unknown = JSON.parse(readFileSync(file, "utf8"));
  if (!isPatternList(value)) {
    throw new Error(`${file} must hold a JSON list of glob patterns`);
  }
  return value;
}
