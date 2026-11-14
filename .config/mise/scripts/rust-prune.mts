// Deletes target/ when a different toolchain built it, and any build folder over
// TARGET_MAX_GB. It measures the folders at most once a day.

import {
  existsSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import process from "node:process";

const targetDir = "target";
const toolchainMarker = join(targetDir, ".pack-toolchain");
const sizeStamp = join(targetDir, ".pack-size-checked");
const day = 24 * 60 * 60 * 1000;
const gigabyte = 1024 ** 3;

function currentToolchain(): string {
  const channel = /^channel\s*=\s*"(?<channel>[^"]+)"/mu.exec(
    readFileSync("rust-toolchain.toml", "utf8"),
  );
  return `${channel?.groups?.channel ?? ""} ${process.env.RUST_NIGHTLY ?? ""}\n`;
}

function folderSize(folder: string): number {
  let total = 0;
  for (const entry of readdirSync(folder, { recursive: true, withFileTypes: true })) {
    if (entry.isFile()) {
      total += statSync(join(entry.parentPath, entry.name)).size;
    }
  }
  return total;
}

function sizeCheckIsDue(): boolean {
  return !existsSync(sizeStamp) || Date.now() - statSync(sizeStamp).mtimeMs > day;
}

function pruneOversizedFolders(limit: number): void {
  for (const entry of readdirSync(targetDir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      const folder = join(targetDir, entry.name);
      const size = folderSize(folder);
      if (size > limit * gigabyte) {
        const shown = (size / gigabyte).toFixed(1);
        console.warn(
          `prune: deleted ${folder} (${shown} GB). If a full build needs this much, raise TARGET_MAX_GB.`,
        );
        rmSync(folder, { force: true, recursive: true });
      }
    }
  }
}

function markSizeChecked(): void {
  writeFileSync(sizeStamp, "");
  const now = new Date();
  utimesSync(sizeStamp, now, now);
}

if (existsSync(targetDir) && existsSync("rust-toolchain.toml")) {
  const toolchain = currentToolchain();
  const recorded = existsSync(toolchainMarker) ? readFileSync(toolchainMarker, "utf8") : toolchain;
  if (recorded === toolchain) {
    writeFileSync(toolchainMarker, toolchain);
    if (sizeCheckIsDue()) {
      pruneOversizedFolders(Number(process.env.TARGET_MAX_GB ?? "5"));
      markSizeChecked();
    }
  } else {
    console.warn(`prune: deleted ${targetDir}, because a different toolchain built it.`);
    rmSync(targetDir, { force: true, recursive: true });
  }
}
