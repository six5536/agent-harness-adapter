// Gemini CLI (`gemini`).
import { loginOnScreen } from "../lib.mjs";

export default {
  id: "gemini",
  name: "Gemini CLI",
  pkg: "@google/gemini-cli",
  version: "0.63.0",
  bin: "gemini",
  // Trusts the folder in a headless run.
  env: { GEMINI_CLI_TRUST_WORKSPACE: "true" },
  login: (a) => loginOnScreen(a, 'pick "Sign in with Google" (or set GEMINI_API_KEY instead)'),
  // yolo approves every tool; the hooks refuse what they must.
  prompt: (a, text) => [
    "-p",
    text,
    "--approval-mode",
    "yolo",
    "-o",
    "stream-json",
    ...(a.model ? ["-m", a.model] : []),
  ],
};
