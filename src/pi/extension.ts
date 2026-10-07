// Written by __TOOL_COMMENT__ harness install (agent-harness-kit). Do not edit: install again instead.
// Runs the tool's hook commands on Pi's events: the event as JSON on stdin,
// the answer as JSON on stdout ({"answer": "allow" | "deny" | "continue" |
// "context", "reason"?, "text"?}). A command that fails, times out or
// answers something else allows.
import { spawn } from "node:child_process";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

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

export default function (pi: ExtensionAPI) {
	let continued = false;

	pi.on("session_start", async (event, ctx) => {
		await first("session-start", "", { source: event.reason }, ctx.cwd);
	});

	pi.on("session_shutdown", async (event, ctx) => {
		await first("session-end", "", { reason: event.reason }, ctx.cwd);
	});

	pi.on("before_agent_start", async (event, ctx) => {
		const a = await first("prompt-submit", "context", { prompt: event.prompt }, ctx.cwd);
		if (a?.text) return { message: { customType: TOOL, content: a.text, display: false } };
	});

	pi.on("tool_call", async (event, ctx) => {
		const a = await first("pre-tool", "deny", { tool_name: event.toolName, tool_input: event.input }, ctx.cwd);
		if (a) return { block: true, reason: a.reason ?? `${TOOL} denied this tool call` };
	});

	pi.on("tool_result", async (event, ctx) => {
		const a = await first(
			"post-tool",
			"context",
			{ tool_name: event.toolName, tool_input: event.input, tool_output: event.content },
			ctx.cwd,
		);
		if (a?.text) return { content: [...event.content, { type: "text" as const, text: a.text }] };
	});

	pi.on("session_before_compact", async (event, ctx) => {
		await first("pre-compact", "", { reason: event.reason }, ctx.cwd);
	});

	pi.on("agent_before_settle", async (event, ctx) => {
		if (!event.context.canContinue) {
			continued = false;
			return;
		}
		const a = await first("stop", "continue", { continuing: continued }, ctx.cwd);
		continued = a !== undefined;
		if (a) {
			return {
				continue: true,
				entries: [{ type: "custom_message" as const, customType: TOOL, content: a.reason ?? "", display: true }],
			};
		}
	});
}
