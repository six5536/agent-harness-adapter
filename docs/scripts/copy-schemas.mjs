// Copies the published JSON schemas (../schema) to public/schema, where the
// site serves them (PLAN-016 F12); run before every build.
import fs from "node:fs";

const from = new URL("../../schema/", import.meta.url);
const to = new URL("../public/schema/", import.meta.url);
fs.rmSync(to, { recursive: true, force: true });
fs.mkdirSync(to, { recursive: true });
for (const name of fs.readdirSync(from)) {
  if (name.endsWith(".json")) fs.copyFileSync(new URL(name, from), new URL(name, to));
}
