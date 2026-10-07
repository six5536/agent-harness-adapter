// Devin CLI (`devin`; Devin Local in Devin Desktop runs the same agent).
// Not on npm: a pinned release bundle from Devin's own download site.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";

import { fail, run } from "../lib.mjs";

const VERSION = "3000.11.3";
const BASE = "https://static.devin.ai/cli";

/** Devin's name for this machine. */
function target() {
  const arch = { x64: "x86_64", arm64: "aarch64" }[process.arch];
  const os = { linux: "unknown-linux", darwin: "apple-darwin", win32: "pc-windows" }[process.platform];
  if (!arch || !os) fail(`Devin has no build for ${process.platform}/${process.arch}`);
  return `${arch}-${os}`;
}

/** The trajectory `--export` wrote: the agent's own messages only. */
function agentText(file) {
  if (!existsSync(file)) return "";
  try {
    const t = JSON.parse(readFileSync(file, "utf8"));
    return (t.steps ?? [])
      .filter((s) => s.source === "agent")
      .map((s) => (typeof s.message === "string" ? s.message : JSON.stringify(s.message ?? "")))
      .join("\n");
  } catch {
    return "";
  }
}

let asked = 0;

export default {
  id: "devin",
  name: "Devin CLI",
  pkg: "devin",
  version: VERSION,
  bin: "devin",
  binDir: ["devin", "bin"],
  install: (dir) => {
    const t = target();
    const manifest = run("curl", ["-fsSL", `${BASE}/${VERSION}/manifest.json`]);
    if (manifest.status !== 0) fail(`no Devin manifest for ${VERSION}`);
    const entry = (JSON.parse(manifest.stdout).platforms ?? {})[t];
    if (!entry) fail(`Devin ${VERSION} has no build for ${t}`);
    const tarball = join(dir, `devin-${VERSION}.tar.gz`);
    if (run("curl", ["-fsSL", "-o", tarball, entry.url]).status !== 0) fail("Devin download failed");
    const sum = createHash("sha256").update(readFileSync(tarball)).digest("hex");
    if (sum !== entry.sha256) fail(`Devin bundle checksum ${sum} is not the manifest's ${entry.sha256}`);
    const out = join(dir, "devin");
    rmSync(out, { recursive: true, force: true });
    mkdirSync(out, { recursive: true });
    if (run("tar", ["-xzf", tarball, "-C", out]).status !== 0) fail("could not unpack Devin");
    rmSync(tarball);
  },
  login: (a) => run(a.binPath, ["auth", "login", "--force-manual-token-flow"], { env: a.env, stdio: "inherit" }),
  // Hooks load only in a trusted folder; `-p` cannot ask, so trust is not
  // checked for the run. `-p` prints the last reply only: the whole
  // trajectory is exported and its agent messages read (`transcript`).
  prepare: (a) => mkdirSync(join(a.home, "devin-exports"), { recursive: true }),
  prompt: (a, text) => [
    "-p",
    text,
    "--permission-mode",
    "dangerous",
    "--respect-workspace-trust",
    "false",
    "--export",
    join(a.home, "devin-exports", `ask-${++asked}.json`),
    ...(a.model ? ["--model", a.model] : []),
  ],
  transcript: (a) => agentText(join(a.home, "devin-exports", `ask-${asked}.json`)),
};
