// Pi (`pi`).
import { readFileSync, statSync } from "node:fs";
import { join } from "node:path";

import { loginOnScreen, walk } from "../lib.mjs";

export default {
  id: "pi",
  name: "Pi",
  pkg: "@earendil-works/pi-coding-agent",
  version: "1.0.4",
  bin: "pi",
  login: (a) => loginOnScreen(a, "type /login and pick a provider"),
  // --approve trusts the project's files for this run, as answering Pi's
  // trust prompt does.
  prompt: (a, text) => ["--approve", ...(a.model ? ["--model", a.model] : []), "-p", text],
  // `pi -p` prints only the last message (after a stop that continued, the
  // reply to it): read every assistant message of the session file.
  transcript: (a, since) => {
    const files = walk(join(a.home, ".pi", "agent", "sessions")).filter(
      (f) => f.endsWith(".jsonl") && statSync(f).mtimeMs >= since,
    );
    const newest = files.sort((x, y) => statSync(y).mtimeMs - statSync(x).mtimeMs)[0];
    if (!newest) return "";
    return readFileSync(newest, "utf8")
      .split("\n")
      .filter(Boolean)
      .map((l) => JSON.parse(l).message)
      .filter((m) => m?.role === "assistant")
      .flatMap((m) => (Array.isArray(m.content) ? m.content : []))
      .filter((c) => c.type === "text")
      .map((c) => c.text)
      .join("\n");
  },
};
