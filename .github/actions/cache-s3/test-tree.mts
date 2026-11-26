// The tree of files that the tests of cache-s3.mts and archive.mts archive, and a snapshot
// of a tree that compares two trees entry by entry.

import {
  chmodSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readlinkSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import process from "node:process";
import { linkKinds } from "./archive.mts";
import type { Context } from "./cache-s3.mts";

const windows = process.platform === "win32";

// A link to a folder, as npm and mise make one: a junction on Windows, with an absolute
// target, and a symbolic link with a relative target elsewhere.
function folderLink(folder: string, relative: string, link: string): void {
  if (windows) {
    symlinkSync(join(folder, relative), link, "junction");
  } else {
    symlinkSync(relative, link);
  }
}

// A folder `.tools` under `home` with what a cache of the tools holds: files of several
// modes, an empty folder, the nested node_modules of npm with a link to the package of
// this platform and one to the optional package of another platform, which leads nowhere,
// mise's link `latest` to an install, links to a file and to a folder, links that lead
// nowhere of each kind, and a path longer than Windows' 260 characters.
export function toolsTree(home: string): void {
  const tools = join(home, ".tools");
  const installs = join(tools, "mise/installs/npm-typescript");
  const store = join(installs, "7.0.2/node_modules/.mise");
  const packages = join(store, "typescript@7.0.2/node_modules");
  mkdirSync(join(packages, "typescript/lib"), { recursive: true });
  mkdirSync(join(packages, "@typescript"), { recursive: true });
  mkdirSync(join(store, "@typescript+typescript-win32-x64@7.0.2"), { recursive: true });
  writeFileSync(join(packages, "typescript/lib/tsc.js"), "export {};\n");
  writeFileSync(join(store, "@typescript+typescript-win32-x64@7.0.2/tsgo.exe"), "binary\n");
  folderLink(
    join(packages, "@typescript"),
    "../../../@typescript+typescript-win32-x64@7.0.2",
    join(packages, "@typescript/typescript-win32-x64"),
  );
  folderLink(
    join(packages, "@typescript"),
    "../../../@typescript+typescript-aix-ppc64@7.0.2",
    join(packages, "@typescript/typescript-aix-ppc64"),
  );
  folderLink(installs, "7.0.2", join(installs, "latest"));
  mkdirSync(join(tools, "bin"), { recursive: true });
  mkdirSync(join(tools, "empty"), { recursive: true });
  writeFileSync(join(tools, "a.txt"), "a\n");
  writeFileSync(join(tools, "bin/run"), "#!/bin/sh\n");
  chmodSync(join(tools, "bin/run"), 0o755);
  writeFileSync(join(tools, "read-only.txt"), "r\n");
  chmodSync(join(tools, "read-only.txt"), 0o444);
  // A mode that a umask of 022 would change, unless the restore keeps the archive's.
  writeFileSync(join(tools, "shared.txt"), "s\n");
  chmodSync(join(tools, "shared.txt"), 0o666);
  mkdirSync(join(tools, "links"), { recursive: true });
  symlinkSync("../a.txt", join(tools, "links/to-file"), "file");
  symlinkSync("../bin", join(tools, "links/to-folder"), "dir");
  symlinkSync("../none.txt", join(tools, "links/file-to-nowhere"), "file");
  symlinkSync("../none", join(tools, "links/folder-to-nowhere"), "dir");
  let deep = join(tools, "deep");
  while (deep.length < 300) {
    deep = join(deep, "a-folder-name-of-forty-characters-long-x");
  }
  mkdirSync(deep, { recursive: true });
  writeFileSync(join(deep, "file.txt"), "deep\n");
}

export interface Entry {
  path: string;
  type: "file" | "folder" | "link";
  size?: number;
  mode: number;
  target?: string;
  kind?: string;
}

// Each entry of a tree, without following a link, in the order of a walk that sorts each
// folder: its type, the size of a file, the mode of a file or a folder, and the target of a link, with its kind
// on Windows. Windows keeps of a mode only whether a file can be written.
export function snapshot(root: string, context: Context): Entry[] {
  const entries: Entry[] = [];
  const visit = (path: string): void => {
    const full = join(root, path);
    const stat = lstatSync(full);
    const mode = windows ? stat.mode & 0o200 : stat.mode & 0o7777;
    // A link's own mode means nothing to the system, which reads the mode of its target.
    if (stat.isSymbolicLink()) {
      entries.push({ path, type: "link", mode: 0, target: readlinkSync(full) });
      return;
    }
    if (!stat.isDirectory()) {
      entries.push({ path, type: "file", size: stat.size, mode });
      return;
    }
    entries.push({ path, type: "folder", mode });
    for (const name of readdirSync(full).toSorted()) {
      visit(path === "" ? name : `${path}/${name}`);
    }
  };
  visit("");
  if (windows) {
    const links = entries.filter((entry) => entry.type === "link");
    const kinds = linkKinds(
      links.map((entry) => join(root, entry.path)),
      context,
    );
    for (const [index, entry] of links.entries()) {
      entry.kind = kinds[index];
    }
  }
  return entries;
}
