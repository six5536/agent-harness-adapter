/**
 * agent-harness-adapter for Node: one agent setup, every coding agent.
 * The JSON shapes are those of `agent-harness-adapter schema`.
 * @packageDocumentation
 */

/** Where to install: the project, the user's home directory, or the project's git-ignored files. */
export type Scope = "project" | "user" | "local";

/** Options every function shares. */
export interface Options {
  /** Default "project". */
  scope?: Scope;
  /** The project directory (default: the config's root with a null manifest, else the working directory). */
  root?: string;
  /** The home directory (default: HOME, else USERPROFILE). */
  home?: string;
}

/** Options of {@link install}. */
export interface InstallOptions extends Options {
  /** Overwrite parts edited by hand. */
  force?: boolean;
  /** Parts to leave out, replacing those declined before. */
  without?: string[];
}

/** Options of {@link uninstall}. */
export interface UninstallOptions extends Options {
  /** Also remove parts edited by hand. */
  force?: boolean;
}

/** One part of a harness's install: what it is, where, and its state. */
export interface PartResult {
  /** The item: "instructions", "skills", "hooks", "mcp", … */
  part: string;
  /** Its state: "edited" was changed by hand; "shared" is served by another harness's copy or by the source itself. */
  state: "skipped" | "shared" | "absent" | "current" | "stale" | "edited";
  /** What the call did, if anything. */
  action?: "created" | "rewrote" | "updated" | "removed" | "kept";
  /** The file or folder, relative to the root. */
  path: string;
  /** The harness whose copy serves it, when another harness's copy does. */
  by?: string;
}

/** The result of {@link install}, {@link uninstall} and {@link status}: `agent-harness-adapter install --json`. */
export interface Result {
  scope: Scope;
  root: string;
  harnesses: { harness: string; parts: PartResult[]; unsupported: string[]; notes: string[] }[];
  warnings: string[];
}

/** A hook event. */
export type Event =
  | "session-start"
  | "session-end"
  | "prompt-submit"
  | "pre-tool"
  | "post-tool"
  | "stop"
  | "pre-compact";

/** The hook contract's input: Claude Code's hook input, in every harness. */
export interface HookInput {
  /** Claude Code's event name, e.g. "PreToolUse". */
  hook_event_name:
    | "SessionStart"
    | "SessionEnd"
    | "UserPromptSubmit"
    | "PreToolUse"
    | "PostToolUse"
    | "Stop"
    | "PreCompact";
  /** The harness that ran the hook, e.g. "codex". */
  harness: string;
  session_id?: string;
  transcript_path?: string;
  cwd?: string;
  /** Prompt submit: the user's prompt. */
  prompt?: string;
  /** The tool, as Claude Code names it, e.g. "Bash". */
  tool_name?: string;
  /** The tool's input as Claude Code gives it: `command` for Bash, `file_path` for a file tool. */
  tool_input?: Record<string, unknown>;
  /** What the tool call does. */
  tool_kind?: "shell" | "read" | "write" | "mcp" | "other";
  /** Post tool: the tool's output. */
  tool_response?: unknown;
  /** Session start: what started it, e.g. "startup". */
  source?: string;
  /** Stop: the agent is already continuing because a stop hook asked it to. */
  stop_hook_active?: boolean;
  last_assistant_message?: string;
  [key: string]: unknown;
}

/** The hook contract's answer: Claude Code's answer JSON. */
export interface Answer {
  /** Block a prompt or a tool, or (at stop) keep the agent going. */
  decision?: "block";
  reason?: string;
  hookSpecificOutput?: {
    hookEventName?: string;
    permissionDecision?: "allow" | "deny" | "ask";
    permissionDecisionReason?: string;
    additionalContext?: string;
    [key: string]: unknown;
  };
  [key: string]: unknown;
}

/**
 * A manifest file's path, a manifest object (text files and sources resolve
 * against the working directory), or null: the config
 * `.agents/agent-harness-adapter.toml`, found as the command finds it.
 */
export type Manifest = string | Record<string, unknown> | null;

/** Install the manifest's integration into `harnesses` (ids, or "all"). */
export function install(manifest: Manifest, harnesses: string[], options?: InstallOptions): Result;
/** Take the integration back out of `harnesses` ("all": the installed ones); parts edited by hand stay unless `force`. */
export function uninstall(manifest: Manifest, harnesses: string[], options?: UninstallOptions): Result;
/** The state of each part, for `harnesses` or the installed ones; nothing is written. */
export function status(manifest: Manifest, harnesses?: string[], options?: Options): Result;
/** The hook contract's input (Claude Code's) for the text `harness` sent at `event`. */
export function parseHook(harness: string, event: Event | string, text: string): HookInput;
/** What a hook command writes for `answer` (Claude Code's answer JSON), in `harness`'s form. */
export function answerHook(
  harness: string,
  event: Event | string,
  answer: Answer,
): { stdout: string; stderr: string | null; exit: number };
/**
 * Be a hook command: read the harness's input on stdin, call `decide` with
 * Claude Code's input, write its answer (null allows) in the harness's form,
 * and resolve to the exit code.
 */
export function runHook(
  harness: string,
  event: Event | string,
  decide: (input: HookInput) => Answer | null | Promise<Answer | null>,
): Promise<number>;
/** The JSON Schema of a published format. */
export function schema(contract: "manifest" | "hooks" | "hook-input" | "hook-answer" | "result"): object;
/** This package's version. */
export const version: string;
/** The manifest `version` this package reads. */
export const MANIFEST_VERSION: number;
