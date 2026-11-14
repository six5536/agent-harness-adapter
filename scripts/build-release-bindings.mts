// Builds a release artefact of a binding into dist/, for the build workflow to
// upload and the publish to read:
//
//   build-release-bindings.mts node <rust-target> <platform>
//     the Node addon, as dist/node-<platform>.node. A Linux target builds
//     against glibc 2.17 with zig, so that one addon serves every glibc
//     distribution of the last decade.
//   build-release-bindings.mts wheel <rust-target>
//     the Python binding's abi3 wheel, which serves CPython 3.9 and later, in
//     dist/. A Linux target builds with zig for manylinux2014.
//   build-release-bindings.mts sdist
//     the Python binding's source distribution, in dist/.
//
// Each artefact is a file at the top of dist/, because the GitHub release
// packs each folder of dist/ as an archive of the command.

import { join } from "node:path";
import process from "node:process";
import { fail, run } from "./lib/run.mts";

const name = "build-release-bindings";
const manifest = "crates/bind/agent-harness-adapter-py/Cargo.toml";
const kind = process.argv.at(2);
const target = process.argv.at(3);
const platform = process.argv.at(4);

function node(rustTarget: string, nodePlatform: string): void {
  run(name, "rustup", ["target", "add", rustTarget]);
  run(name, "node", [
    "scripts/build-node.mts",
    "--release",
    "--target",
    rustTarget,
    ...(rustTarget.endsWith("-linux-gnu") ? ["--glibc", "2.17"] : []),
    "--out",
    join("dist", `node-${nodePlatform}.node`),
  ]);
}

// sokf-implements: BND-4_AC-1
function wheel(rustTarget: string): void {
  run(name, "rustup", ["target", "add", rustTarget]);
  const linux = rustTarget.includes("-linux-");
  run(name, "maturin", [
    "build",
    "--release",
    "--locked",
    "--manifest-path",
    manifest,
    "--target",
    rustTarget,
    "--out",
    "dist",
    ...(linux ? ["--zig", "--compatibility", "manylinux2014"] : []),
  ]);
}

if (kind === "node" && target !== undefined && platform !== undefined) {
  node(target, platform);
} else if (kind === "wheel" && target !== undefined) {
  wheel(target);
} else if (kind === "sdist") {
  run(name, "maturin", ["sdist", "--manifest-path", manifest, "--out", "dist"]);
} else {
  fail(name, "usage: build-release-bindings.mts node <target> <platform> | wheel <target> | sdist");
}
