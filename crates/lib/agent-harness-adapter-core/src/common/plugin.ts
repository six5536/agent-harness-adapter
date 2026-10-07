// Written by __TOOL_COMMENT__ harness install (agent-harness-adapter). Do not edit: install again instead.
// Runs the tool's hook commands on __AGENT__'s events: the event as JSON on
// stdin, the answer as JSON on stdout ({"answer": "allow" | "deny" |
// "continue" | "context", "reason"?, "text"?}). A command that fails, times
// out or answers something else allows.
import { spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import type { Plugin } from "__PLUGIN_TYPES__";

type Answer = { answer?: string; reason?: string; text?: string };
type Spec = { event: string; command: string; timeout?: number };

const TOOL: string = __TOOL__;
const HOOKS: Spec[] = __HOOKS__;

function run(spec: Spec, payload: Record<string, unknown>, cwd: string): Promise<Answer> {
	return new Promise((resolve) => {
		let out = "";
		let settled = false;
		const finish = (answer: Answer) => {
			if (!settled) {
				settled = true;
				resolve(answer);
			}
		};
		const child = spawn(spec.command, { shell: true, cwd, stdio: ["pipe", "pipe", "ignore"] });
		const timer = spec.timeout
			? setTimeout(() => {
					child.kill();
					finish({});
				}, spec.timeout)
			: undefined;
		child.stdout.on("data", (chunk) => {
			out += String(chunk);
		});
		child.on("error", () => finish({}));
		child.on("close", (code) => {
			if (timer) clearTimeout(timer);
			if (code !== 0) return finish({});
			try {
				const parsed: unknown = JSON.parse(out);
				finish(typeof parsed === "object" && parsed !== null ? (parsed as Answer) : {});
			} catch {
				finish({});
			}
		});
		child.stdin.on("error", () => {});
		child.stdin.end(JSON.stringify({ ...payload, cwd }));
	});
}

/** Run every hook of `event`; the first answer of `kind` wins. */
async function first(event: string, kind: string, payload: Record<string, unknown>, cwd: string): Promise<Answer | undefined> {
	let found: Answer | undefined;
	for (const spec of HOOKS.filter((h) => h.event === event)) {
		const answer = await run(spec, { event, ...payload }, cwd);
		if (!found && answer.answer === kind) found = answer;
	}
	return found;
}

let lastTime = 0;
let counter = 0;

/** A part id as __AGENT__ makes them ("prt_", the time, random), so the part sorts after the message's own. */
function partId(): string {
	const now = Date.now();
	counter = now === lastTime ? counter + 1 : 0;
	lastTime = now;
	const time = (BigInt(now) * 4096n + BigInt(counter)).toString(16).padStart(12, "0").slice(-12);
	const chars = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
	return `prt_${time}${[...randomBytes(14)].map((b) => chars[b % 62]).join("")}`;
}

export default (async ({ client, directory }) => {
	// Subagents' sessions (the task tool): their tools are hooked, but they
	// have no session start, prompt or stop of their own.
	const children = new Set<string>();
	const started = new Set<string>();
	// Sessions whose last stop asked to go on (`continuing`); a new prompt
	// starts over.
	const continued = new Set<string>();
	// Sessions whose stop is being decided (null), or whose continuation, the
	// plugin's own next message, has not arrived yet (its text): a stop can
	// be reported as idle more than once, and is decided once.
	const pending = new Map<string, string | null>();
	const last = new Map<string, { agent?: string; model?: { providerID: string; modelID: string } }>();

	return {
		event: async ({ event }) => {
			if (event.type === "session.created" && event.properties.info.parentID) {
				children.add(event.properties.info.id);
			}
			if (event.type !== "session.idle") return;
			const id = event.properties.sessionID;
			if (children.has(id) || pending.has(id)) return;
			pending.set(id, null);
			const a = await first("stop", "continue", { session_id: id, continuing: continued.has(id) }, directory);
			if (!a) {
				continued.delete(id);
				pending.delete(id);
				return;
			}
			continued.add(id);
			pending.set(id, a.reason ?? "");
			try {
				await client.session.promptAsync({
					path: { id },
					body: { ...last.get(id), parts: [{ type: "text", text: a.reason ?? "" }] },
				});
			} catch {
				pending.delete(id);
			}
		},

		// The session starts with its first prompt: __AGENT__ takes context
		// only with a prompt.
		"chat.message": async (input, output) => {
			const id = input.sessionID;
			if (children.has(id)) return;
			last.set(id, { agent: input.agent, model: input.model });
			const prompt = output.parts
				.flatMap((p) => (p.type === "text" && !p.synthetic ? [p.text] : []))
				.join("\n");
			const own = pending.get(id);
			pending.delete(id);
			if (typeof own === "string" && own === prompt) return;
			continued.delete(id);
			const texts: string[] = [];
			if (!started.has(id)) {
				started.add(id);
				const s = await first("session-start", "context", { session_id: id, source: "startup" }, directory);
				if (s?.text) texts.push(s.text);
			}
			const a = await first("prompt-submit", "context", { session_id: id, prompt }, directory);
			if (a?.text) texts.push(a.text);
			if (texts.length > 0) {
				output.parts.push({
					id: partId(),
					sessionID: id,
					messageID: output.message.id,
					type: "text",
					text: texts.join("\n\n"),
					synthetic: true,
				});
			}
		},

		"tool.execute.before": async (input, output) => {
			const a = await first(
				"pre-tool",
				"deny",
				{ session_id: input.sessionID, tool_name: input.tool, tool_input: output.args },
				directory,
			);
			if (a) throw new Error(a.reason ?? `${TOOL} denied this tool call`);
		},

		"tool.execute.after": async (input, output) => {
			const a = await first(
				"post-tool",
				"context",
				{ session_id: input.sessionID, tool_name: input.tool, tool_input: input.args, tool_output: output.output },
				directory,
			);
			if (a?.text) output.output = `${output.output}\n\n${a.text}`;
		},

		"experimental.session.compacting": async (input) => {
			await first("pre-compact", "", { session_id: input.sessionID }, directory);
		},
	};
}) satisfies Plugin;
