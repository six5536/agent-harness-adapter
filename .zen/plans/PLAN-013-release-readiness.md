# PLAN-013: Release readiness: a release dry run, and Codex and Pi for real

| Field              | Value |
| ------------------ | ----- |
| Status             | in-progress: P3 started (2026-10-07); P1–P2 next |
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
| P6 Close | README harness table notes; PLAN-011 P9 unblocked | Committed and pushed |

## 5. Resolved

- D13-6: 0.1.0 ships `uninstall` (PLAN-014). The agent checks gain A10: uninstall leaves the agent without the tool's instructions, skill and hooks.

## 6. Results

Agents: Codex CLI 0.160.1, Pi 1.0.4 (npm, in the session's scratch area), isolated `HOME`, scratch git project, `examples/python-tool` manifest.

| Check | Codex | Pi |
| ----- | ----- | -- |
| A1 install / status / no-op re-install | pass (2026-10-07) | pass (`AGENTS.md` and `.agents/skills` shared from Codex; `.pi/extensions/mytool.ts` written) |
| A2 instructions | pass: `codex debug prompt-input` holds the block between the markers | needs a login |
| A3 skill | pass: listed in the model input (`mytool: Check a shell command…`) | needs a login |
| A4–A9 | need a login | need a login |

Codex 0.160.1 reports `hooks` as a stable feature, on by default.
