// The agents the checks can drive, each pinned to the version last checked
// (PLAN-013 §6). An agent is a module with:
//
//   id, name, pkg, version, bin    who it is and how npm installs it
//   login(a)                       log in, or say how, under the checks' HOME
//   prompt(a, text)                the arguments of one non-interactive run
//   ask(a, text, cwd)?             one run, when the arguments are not enough: { stdout, stderr }
//   prepare(a, project)?           trust a project, as answering its prompt does
//   transcript(a, since)?          the whole reply, when its output lacks some
//   env?                           variables it needs
//   flags?                         its own options: --<id>-<flag>
//   unsupported?                   checks it cannot pass by design
//   own(file)?                     a project file the agent itself adds ("dir/name")
//
// `a` is the agent as the checks use it: { def, home, binPath, env, model, flags }.
import claude from "./claude.mjs";
import codex from "./codex.mjs";
import copilot from "./copilot.mjs";
import gemini from "./gemini.mjs";
import opencode from "./opencode.mjs";
import pi from "./pi.mjs";

export const AGENTS = Object.fromEntries([claude, codex, gemini, copilot, pi, opencode].map((a) => [a.id, a]));
