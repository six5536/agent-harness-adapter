# Real-agent checks

`run.mjs` installs a test tool with `agent-harness-adapter` into a throwaway
project, drives each agent non-interactively, and checks what reached it
(PLAN-013 §3). It needs the agents' own logins, so it runs on demand, not in
CI.

```sh
cargo build -p agent-harness-adapter
node scripts/agents/run.mjs setup              # pinned Codex and Pi, into target/agent-checks
node scripts/agents/run.mjs login codex        # once: device-code login
node scripts/agents/run.mjs login pi           # once: prints how to /login in Pi
node scripts/agents/run.mjs run [--agents codex,pi] [--codex-full-access] [--pi-model <m>] [--codex-model <m>]
```

Every agent runs with `HOME` set to `target/agent-checks/home`, so your own
agent setup is never touched. Each run writes the agents' outputs, the hook
log and `results.json` under `target/agent-checks/run-<time>/`.

| Check | How it is decided |
| ----- | ----------------- |
| A1 install, status, no-op re-install | the adapter's `status` and second `install` |
| A2 instructions | the reply holds the run's project codeword |
| A3 skill | the reply names the `aha-check` skill |
| A4 session-start context | the reply holds the session word |
| A5 pre-tool deny | `touch aha-denied-marker` is refused: no file, a deny in the hook log |
| A6 pre-tool allow | `echo ok > aha-allowed.txt` runs |
| A7 stop continues once | the stop hook continues once, then allows (`continuing`); the reply holds the stop word |
| A7b post-tool context (warning only) | the reply holds the after-tool word; a model may ignore it |
| A9 user scope | from an empty project, the reply holds the codeword |
| A10 uninstall | `status` absent at user scope; the project holds nothing of the tool, the reply has no codeword and no hook runs |

Notes:

- Codex runs new project hooks only after you approve them in its `/hooks`;
  the checks pass `--dangerously-bypass-hook-trust` in its place. The
  project is trusted in the checks' own Codex config.
- `--codex-full-access` turns Codex's sandbox off, for machines where it
  cannot start (e.g. containers without user namespaces).
- Pi trusts the project through `--approve`. `pi -p` prints only its last
  message, so the checks read Pi's session file for the whole reply.
- To add an agent: an entry in `AGENTS` in `run.mjs` (package, version, how
  to prompt it, how to trust a project, how to read its whole reply).
