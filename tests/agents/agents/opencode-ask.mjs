// One prompt to OpenCode through its server, printing every assistant text
// and error of the session. `opencode run` exits as soon as the session
// first goes idle, before a stop hook can continue it; the server keeps
// running, so the session is followed until it stays idle.
//
//   node opencode-ask.mjs <opencode> <prompt> [provider/model]
//
// Runs in the project (the server serves its working directory) with the
// environment it is given.
import { spawn } from "node:child_process";

const [bin, text, model] = process.argv.slice(2);
const QUIET_MS = 15_000;
const LIMIT_MS = 540_000;

const server = spawn(bin, ["serve", "--hostname", "127.0.0.1", "--port", "0"], {
  stdio: ["ignore", "pipe", "inherit"],
});
const stop = (code) => {
  server.kill();
  process.exit(code);
};
setTimeout(() => {
  console.error("opencode-ask: timed out");
  stop(1);
}, LIMIT_MS).unref();

const base = await new Promise((resolve, reject) => {
  let out = "";
  server.stdout.on("data", (chunk) => {
    out += String(chunk);
    const m = /listening on (http:\/\/\S+)/.exec(out);
    if (m) resolve(m[1]);
  });
  server.on("exit", (code) => reject(new Error(`opencode serve exited (${code}): ${out}`)));
});

async function call(method, path, body) {
  const r = await fetch(`${base}${path}`, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!r.ok) throw new Error(`${method} ${path}: ${r.status} ${await r.text()}`);
  const t = await r.text();
  return t ? JSON.parse(t) : undefined;
}

try {
  const session = await call("POST", "/session", {});
  const [providerID, ...rest] = model ? model.split("/") : [];
  await call("POST", `/session/${session.id}/prompt_async`, {
    ...(model ? { model: { providerID, modelID: rest.join("/") } } : {}),
    parts: [{ type: "text", text }],
  });
  // Idle (or unlisted) for QUIET_MS after the first answer: done.
  let quietSince;
  for (;;) {
    await new Promise((r) => setTimeout(r, 1000));
    const status = (await call("GET", "/session/status"))?.[session.id];
    const messages = await call("GET", `/session/${session.id}/message`);
    const answered = messages.some((m) => m.info.role === "assistant" && m.info.time?.completed);
    if (status?.type === "busy" || status?.type === "retry" || !answered) quietSince = undefined;
    else if (quietSince === undefined) quietSince = Date.now();
    else if (Date.now() - quietSince >= QUIET_MS) break;
  }
  for (const m of await call("GET", `/session/${session.id}/message`)) {
    if (m.info.role !== "assistant") continue;
    if (m.info.error) console.log(JSON.stringify(m.info.error));
    for (const p of m.parts) if (p.type === "text") console.log(p.text);
  }
  stop(0);
} catch (e) {
  console.error(`opencode-ask: ${e.message}`);
  stop(1);
}
