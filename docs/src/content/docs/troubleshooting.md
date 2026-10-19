---
title: Troubleshooting
description: What to check when an agent does not pick something up, and what install's warnings mean.
---

## A hook does not run

1. **Is the agent loading the new files?** Several agents need a step after
   an install: approve new hooks in Codex's `/hooks` and trust the project;
   trust the folder in Gemini CLI, Copilot, Pi and Devin; restart Codex,
   OpenCode, Kilo Code and Qwen Code. `install` prints these as `note:` lines.
2. **Is the command on `PATH`** where the agent runs? Each agent's hook entry
   runs `agent-harness-adapter`. Without it the hook cannot start, and allows.
   Set `adapter` in the config to an absolute path if the agent's `PATH` differs
   from your shell's.
3. **Does the agent have the event?** `install` notes
   `<agent> has no <event> event: those hooks do not run there`. See the table on
   [Hooks](/agent-harness-adapter/write-once/hooks/#what-each-agent-can-do).
4. **Is it a new event?** Run `install` again after you add a hook on an event
   `.agents/hooks.json` did not use before.
5. **Does the matcher match?** It must match Claude Code's whole tool name:
   `Bash`, not `bash` or `shell`.
6. **Run it by hand.** Pipe Claude Code's input into the dispatcher and read
   what it says on stderr:

   ```sh
   echo '{"tool_name":"Bash","tool_input":{"command":"rm -rf build"}}' \
     | agent-harness-adapter hook --hooks .agents/hooks.json claude pre-tool
   ```

## A hook blocks nothing

A hook that fails, times out or prints something the agent cannot express
allows, with a note on stderr. Check that the script is executable, that it
exits 2 (or prints a block answer) and that it finishes within its `timeout`.

## What install prints

| Line | Means | Do |
| --- | --- | --- |
| `edited … ` | changed by hand since the last install; left alone | keep your change, or `install --force` to overwrite it |
| `stale …` | out of date with your sources | run `install` |
| `missing source: config/mcp.json (mcp)` | a source you named in the config does not exist (an unused standard place, such as `.mcp.json`, is not listed) | create it, or fix the path; meanwhile it installs nothing |
| `warning: … may load the skills twice` | the agent also reads Claude Code's copies in `.claude/skills` | nothing: the copies are identical |
| `unsupported: permissions` | the agent has no file for that item | nothing |
| `kept … used by <agent>` (uninstall) | another installed agent still reads it | uninstall that agent too, to remove it |

## The config is not found

Commands look for `.agents/agent-harness-adapter.toml` from the working
directory up to the repository's root, never at or above your home
directory. Run them inside the repository, or pass `--root`. Your own setup is
`--scope user`.

## Updating the command

Update it as you installed it (`npm install -g @six5536/agent-harness-adapter`
or `cargo install agent-harness-adapter`), then run `install` once: it rewrites
what a new version writes differently, and `status` tells you if anything is
left. Pre-1.0, read the notes of each
[release](https://github.com/six5536/agent-harness-adapter/releases) for
breaking changes.
