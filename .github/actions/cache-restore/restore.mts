// Runs the action cache-restore: with the input phase `restore`, it restores the caches
// before mise installs the tools, and with `build`, it turns sccache on after it.

import { homedir } from "node:os";
import process from "node:process";
import { restore, runnerContext, turnOnBuild } from "../cache-s3/cache-s3.mts";

const context = runnerContext(process.env, homedir());
const tools = process.env.INPUT_TOOLS ?? "";
try {
  if (process.env.INPUT_PHASE === "build") {
    turnOnBuild(tools, context);
  } else {
    await restore(tools, context);
  }
} catch (error) {
  console.log(`::error::${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
}
