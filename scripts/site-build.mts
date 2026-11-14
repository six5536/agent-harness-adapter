// Builds the documentation site in docs/: the Python binding's reference with
// pdoc, which imports the built package, then the Astro site, whose build
// writes the Node binding's reference with TypeDoc, copies the JSON schemas,
// and fails on a TypeDoc warning or a broken link. The site lands in
// docs/dist.
//
// Usage: site-build.mts

import { rmSync } from "node:fs";
import { join } from "node:path";
import { installPythonBinding, run, targetDir, venvPython, windows } from "./lib/run.mts";

const name = "site-build";
const python = venvPython(name, join(targetDir, `${name}-venv`));

installPythonBinding(name, python);
run(name, python, ["-m", "pip", "install", "--quiet", "pdoc==16.0.0"]);
// pdoc writes a page for each module, and leaves the pages of a module that
// no longer exists, so the reference starts empty.
rmSync("docs/public/api/python", { recursive: true, force: true });
run(name, python, ["-m", "pdoc", "agent_harness_adapter", "-o", "docs/public/api/python"]);

const npm = windows ? "npm.cmd" : "npm";
run(name, npm, ["ci", "--no-audit", "--no-fund"], { cwd: "docs", shell: windows });
run(name, npm, ["run", "build"], { cwd: "docs", shell: windows });
