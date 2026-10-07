// agent-harness-kit for Node. The JSON shapes are those of `ahk schema`.

export type Scope = "project" | "user" | "local";

export interface Options {
  scope?: Scope;
  /** The project directory (default: the working directory). */
  root?: string;
  /** The home directory (default: HOME, else USERPROFILE). */
  home?: string;
}

export interface InstallOptions extends Options {
  /** Overwrite parts edited by hand. */
  force?: boolean;
  /** Parts to leave out, replacing those declined before. */
  without?: string[];
}

export interface PartResult {
  part: string;
  state: "skipped" | "shared" | "absent" | "current" | "stale" | "edited";
  action?: "created" | "rewrote" | "updated";
  path: string;
  by?: string;
}

export interface Result {
  scope: Scope;
  root: string;
  harnesses: { harness: string; parts: PartResult[]; unsupported: string[]; notes: string[] }[];
  warnings: string[];
}

export type Event =
  | "session-start"
  | "session-end"
  | "prompt-submit"
  | "pre-tool"
  | "post-tool"
  | "stop"
  | "pre-compact";

export interface HookInput {
  v: 1;
  harness: string;
  event: Event;
  session_id?: string;
  cwd?: string;
  transcript_path?: string;
  prompt?: string;
  tool?: { name: string; kind: "shell" | "read" | "write" | "mcp" | "other"; input: unknown };
  tool_output?: unknown;
  source?: string;
  continuing?: boolean;
  last_message?: string;
  raw?: unknown;
}

export type Answer =
  | { answer: "allow"; stderr?: string }
  | { answer: "deny"; reason: string }
  | { answer: "continue"; reason: string }
  | { answer: "context"; text: string };

/** A manifest file's path, or a manifest object (text files resolve against the working directory). */
export type Manifest = string | Record<string, unknown>;

export function install(manifest: Manifest, harnesses: string[], options?: InstallOptions): Result;
export function status(manifest: Manifest, harnesses?: string[], options?: Options): Result;
export function parseHook(harness: string, event: Event | string, text: string): HookInput;
export function answerHook(
  harness: string,
  event: Event | string,
  answer: Answer,
): { stdout: string; stderr: string | null; exit: number };
export function runHook(
  harness: string,
  event: Event | string,
  decide: (input: HookInput) => Answer | Promise<Answer>,
): Promise<number>;
export function schema(contract: "manifest" | "hook-input" | "hook-answer" | "result"): object;
export const version: string;
export const MANIFEST_VERSION: number;
export const HOOK_VERSION: number;
