# PLAN-013: Release readiness: a release dry run, and Codex and Pi for real

| Field              | Value |
| ------------------ | ----- |
| Status             | in-progress: P3–P5c done (2026-10-07): Codex and Pi pass every check; P1–P2 (release dry run) next |
| Workflow direction | bottom-up (CI and manual checks → fixes through the specs) |
| Traces to          | PLAN-011 (P9, D11-11, N4), PLAN-010 (N4), REQ-HAR (HAR-2 Codex, HAR-7 Pi), REQ-AHA, `.github/workflows/release.yml` |

## 1. Goal

Before the first tag (PLAN-011 P9):

1. Run the whole release once without publishing, so a failure cannot leave 0.1.0 half-published (crates.io never takes a version twice).
2. Check that Codex and Pi load what `agent-harness-adapter install` writes, and that bridged hooks behave in the real agents. So far only Claude Code has been used by a real tool (smllm).

## 2. Decisions

- D13-1: `release.yml` gets a manual trigger (`workflow_dispatch`, input `dry_run`, default `true`). A dry run:
  - takes the version from the workspace `Cargo.toml`, not a tag, and still runs `verify-version` and the CHANGELOG check;
  - runs the checks and every build job (binaries, wheels, sdist, Node addons) with their smoke tests;
  - in `publish`: places the artefacts, runs every dry-run publish (cargo, npm), then stops;
  - PyPI: `twine check` on the wheels and sdist instead of uploading;
  - GitHub release: builds the archives and checksums and uploads them as a workflow artefact, creates no release.
  - A tag push stays a real release, unchanged.
- D13-2: The dry run's artefacts are also checked by hand here (linux-arm64): the static binary, the abi3 wheel in a clean venv, the Node addon through the packed npm package.
- D13-3: Agent tests use a scratch project and an isolated `HOME`, so nothing touches the user's own Codex or Pi setup. Credentials are the user's: they log in themselves (`! codex login`; Pi: a provider key or its own `/login`); nothing is stored in the repo.
- D13-4: The test tool is `examples/python-tool` with additions for the checks in §3 (a stop hook that continues once, a post-tool context). Its manifest stays the example users read.
- D13-5: Every finding goes through the specs first (REQ-HAR / DESIGN-HAR), then the adapter, then a test; findings that cannot be fixed are written down as known limits in the README's harness table.

## 3. Agent checks (per agent: Codex, Pi)

| # | Check | How |
| - | ----- | --- |
| A1 | Install writes the expected files; `status` is `current`; a second install writes nothing | `agent-harness-adapter install/status --harness codex` (then `pi`) in the scratch project |
| A2 | The agent loads the instructions | Ask "what does this project use for shell commands?"; it names mytool |
| A3 | The agent sees the skill | Codex: `.agents/skills`; Pi: its skills listing |
| A4 | Session start adds context | The model can quote "mytool is watching shell commands" |
| A5 | Pre-tool deny blocks a shell command | Ask it to run `rm -rf /tmp/aha-smoke`; the call is refused with mytool's reason |
| A6 | Pre-tool allow lets a safe command run | `ls` runs |
| A7 | Stop continue runs once, then the agent stops | The stop hook asks once; no loop |
| A8 | The trust / approval notes the install printed are right | Codex: project trust and `/hooks` approval; Pi: project trust and `/reload` |
| A9 | User scope | Repeat A1–A2 with `--scope user` under the isolated `HOME` |
| A10 | Uninstall (PLAN-014) | After `uninstall`, the agent no longer sees the instructions or skill and the hooks no longer run |

Each check is recorded pass / fail with the agent's version in §6.

## 4. Phases

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P1 Dry-run mode | D13-1 in `release.yml`; actionlint | Lint clean, committed |
| P2 Dry run | Trigger it on `main` (the user, from the Actions tab, or a token); fix what fails until green; D13-2 | One green dry run; artefacts checked by hand |
| P3 Agent setup | Install Codex and Pi in the scratch area; the user logs in; scratch project with the example tool (D13-4) | Both agents answer a prompt |
| P4 Codex | §3 checks | Results in §6; fixes landed |
| P5 Pi | §3 checks | Results in §6; fixes landed |
| P5b Agent checks in the repo | The scratch checks (§3) as a script under `tests/agents/`, run on demand against installed, logged-in agents with an isolated `HOME`; Codex and Pi first, written so other agents can be added | `tests/agents` runs §3 for Codex and Pi and prints a pass / fail table |
| P5c Retest | Run P5b after the findings' fixes (`ff4d029`) and PLAN-014 | All of §3, A10 included, pass or are written down as known limits |
| P6 Close | README harness table notes; PLAN-011 P9 unblocked | Committed and pushed |

## 5. Resolved

- D13-6: 0.1.0 ships `uninstall` (PLAN-014). The agent checks gain A10: uninstall leaves the agent without the tool's instructions, skill and hooks.

## 6. Results

Agents: Codex CLI 0.160.1, Pi 1.0.4 (npm, in the session's scratch area), isolated `HOME`, scratch git project, `examples/python-tool` manifest.

| Check | Codex 0.160.1 (gpt-6-luna) | Pi 1.0.4 (openai-codex/gpt-6-luna) |
| ----- | ----- | -- |
| A1 install / status / no-op re-install | pass | pass (`AGENTS.md`, `.agents/skills` shared from Codex) |
| A2 instructions | pass | pass |
| A3 skill | pass (the model ran `mytool.py check` on its own) | pass |
| A4 session-start context | pass, once hooks are trusted | **fail**: the hook answers, Pi drops it (finding 1) |
| A5 pre-tool deny | pass (`echo git push --force` blocked with mytool's reason) | pass |
| A6 pre-tool allow | pass | pass |
| A7 stop continues once | pass (continued once, then `continuing` → allow) | **fail**: the stop hook never runs (finding 3) |
| A7b post-tool context | pass | pass (appended to the tool result; the model did not act on it) |
| A8 notes | right: Codex runs new hooks only once trusted (`/hooks`, or `--dangerously-bypass-hook-trust`); the project needed trust | right: `--approve` / trust needed for project files |
| A9 user scope | pass | pass |
| A10 uninstall | after PLAN-014 | after PLAN-014 |

Test conditions: Codex's own sandbox cannot start in the dev container (`bwrap`), so Codex ran with `--sandbox danger-full-access`; Codex also refuses `rm -rf` itself, before any hook, so the deny test used `echo git push --force`.

### Retest (P5c, 2026-10-07, `tests/agents/run.mjs`)

After the fixes (`ff4d029`) and uninstall (`5671768`): Codex 0.160.1 and Pi 1.0.4 pass A1–A10, A7b included (Codex with `--codex-full-access` in the dev container; Pi with `--pi-model openai-codex/gpt-6-luna`, as the ChatGPT account lacks Pi's default model). The first Pi run failed A2–A4 and A9 in the checker only: `pi -p` prints just the last message, so the checks now read Pi's session file.

### More agents (2026-10-07)

`tests/agents` gained Claude Code 2.1.292, Gemini CLI 0.63.0 and Copilot CLI 1.0.92.

- Gemini CLI: passes A1–A10, A7b included.
- Copilot CLI: with quota, passes A1–A3, A5–A7, A9, A10; A4 and A7b are n/a (Copilot takes no context from hooks, HAR-5_AC-5). It first ran no hook at all: its debug log says it loads repository hooks only when the folder is trusted or opted in. Finding 5: our Copilot harness gave no note about that; it now says so (HAR-5_AC-10). The checks trust the folder through `trustedFolders` in Copilot's `config.json`.
- Claude Code: waits for a login in the checks' own `HOME` (the user's real login is not used, so user-scope checks never touch the real `~/.claude`).
- Finding 4 (all, small): after uninstall, Codex still said "new hooks run only once approved in Codex's /hooks" (and Pi and Gemini their trust notes). Notes now separate what was written (trust, approval) from any change (restart, reload): `PartResult::wrote` / `changed`; HAR-2_AC-7, HAR-4_AC-9, HAR-7_AC-7.

### Findings

- Finding 1 (Pi, session-start context): Pi has no way to add context at session start; the adapter refuses that answer (by design, HAR-7), so a tool's session-start context silently vanishes on Pi while Claude and Codex deliver it. Fix: the extension keeps the text and adds it to the first prompt (`before_agent_start`), and the adapter accepts `context` at session start. Spec: HAR-7_AC-3.
- Finding 2 (Pi, Cursor, Copilot: no tool matcher): a hook with `tools = "shell"` runs for every tool there (documented: the hook filters on the tool kind), so the example's post-tool context also fired after `read`. Fix for bridged hooks: the template carries `--tools <kind>` and `agent-harness-adapter hook` allows without running the command for other kinds. Spec: AHA-1_AC-8, AHA-4.
- Finding 3 (Pi, stop): bug. At `agent_before_settle` Pi's `context.canContinue` is false whenever the last message is the model's answer, which is always the case at a stop; it turns true only once a handler adds an entry. The extension returned early on it, so stop hooks never ran on Pi. Our earlier check used a mock with `canContinue: true`. Fix: run the stop hooks at every settle, answer `continue` with a `custom_message` entry, and reset the `continuing` flag at each new prompt (`before_agent_start`). Spec: HAR-7_AC-3, HAR-7_AC-4.

Codex 0.160.1 reports `hooks` as a stable feature, on by default.
