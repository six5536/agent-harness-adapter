// Runs the action cache-save: it saves each cache that cache-restore missed, and stops
// sccache, which prints its statistics. A failure only warns, because the job's own work
// is done, and a later step, such as the upload of an artifact, still has to run.

import { homedir } from "node:os";
import process from "node:process";
import { runnerContext, save } from "../cache-s3/cache-s3.mts";

try {
  await save(runnerContext(process.env, homedir()));
} catch (error) {
  console.log(`::warning::${error instanceof Error ? error.message : String(error)}`);
}
