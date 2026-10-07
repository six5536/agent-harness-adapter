# Real-agent checks

These checks install a test tool with `agent-harness-adapter` into a
throwaway project, drive each agent non-interactively, and check what
reached it (PLAN-013 §3). They need the agents' own logins, so they run on
demand, not in CI.

```sh
cargo build -p agent-harness-adapter
node tests/agents/run.mjs setup              # pinned agents, into target/agent-checks
node tests/agents/run.mjs login <agent>      # once per agent
node tests/agents/run.mjs run [--agents claude,codex,gemini,copilot,pi] [--<agent>-model <m>] [--codex-full-access]
```

Every agent runs with `HOME` set to `target/agent-checks/home`, so your own
agent setup is never touched. The test projects are fresh folders in the
system's temporary directory, away from the run's files, so an agent that
searches cannot find the run's words. Each run keeps every reply
(`ask-<n>.out`), the hook log and `results.json` under
`target/agent-checks/run-<time>/<agent>/`.

## Layout

| File | What it holds |
| ---- | ------------- |
| `run.mjs` | the command line: `setup`, `login`, `run`, the results table |
| `checks.mjs` | the check suite, the same for every agent |
| `tool.mjs` | the test tool the hooks run: answers from the run's words, logs every call |
| `lib.mjs` | helpers the checks and the agents share |
| `agents/index.mjs` | the list of agents, and what an agent module holds |
| `agents/<agent>.mjs` | one agent: package and version, login, how to prompt it, how to trust a project, how to read its whole reply, what it cannot do |

To add an agent: a new `agents/<agent>.mjs` (see the fields in
`agents/index.mjs`) and its line in `agents/index.mjs`.

## Checks

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

A check an agent cannot pass by design shows `n/a`. A run where the agent
ran out of quota or hit a rate limit ends with a `quota` warning: its
failures after that say nothing about the adapter.

## Agents

| Agent | Login | How it runs |
| ----- | ----- | ----------- |
| `claude` (Claude Code) | `login claude` signs in | `-p` (no trust dialog), `--dangerously-skip-permissions`; the `CLAUDE*` variables of a surrounding Claude Code session are removed, so it uses only the checks' `HOME` |
| `codex` (Codex) | device code | `exec` with `--dangerously-bypass-hook-trust` in place of approving new hooks in `/hooks`; the project is trusted in the checks' Codex config; `--codex-full-access` turns its sandbox off where it cannot start (e.g. containers without user namespaces) |
| `gemini` (Gemini CLI) | prints how to sign in on its screen, or set `GEMINI_API_KEY` | `--approval-mode yolo`, `GEMINI_CLI_TRUST_WORKSPACE=true`; the free tier allows about 20 requests a day (3 per run) |
| `copilot` (Copilot CLI) | device code; needs Copilot quota | `--allow-all --no-ask-user`; the project is added to `trustedFolders` in its `config.json` (it loads repository hooks only from a trusted folder); it takes no context from hooks, so A4 and A7b show `n/a` |
| `pi` (Pi) | prints how to `/login` on its screen | `--approve` trusts the project; `pi -p` prints only its last message, so the checks read its session file |
