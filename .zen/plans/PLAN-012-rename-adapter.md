# PLAN-012: Rename to `agent-harness-adapter` (project and CLI)

| Field              | Value |
| ------------------ | ----- |
| Status             | in-progress: P0–P6 done (2026-10-07); P7 is the user's (§2 D12-9) |
| Workflow direction | lateral (layout → specs → code & tests → packaging → docs) |
| Traces to          | smllm, sokf (§5), ARCHITECTURE, REQ-AHK → REQ-AHA, DESIGN-AHK → DESIGN-AHA, REQ-KIT, DESIGN-KIT, REQ-BND, DESIGN-BND, PLAN-011 (D11-1, D11-7, D11-8, D11-9, D11-11, P9) |

## 1. Goal

Rename the project before its first release (PLAN-011 P9), when it is still free to do so:

- `agent-harness-kit` → `agent-harness-adapter`: on npm, tuanle96/agent-harness-kit (v0.24.0, "harness engineering kit for Claude Code") has the same name, works in the same area and installs an `agent-harness-kit` command.
- CLI `ahk` → `agent-harness-adapter`: "AHK" means AutoHotkey (and `.ahk` is its file extension).

Availability, checked 2026-10-07:
- `agent-harness-adapter` and `agent-harness-adapter-core`: free on crates.io, npm, PyPI and Homebrew.
- `@six5536/agent-harness-adapter`, `six5536-agent-harness-adapter`, and the GitHub repo `six5536/agent-harness-adapter`: free.

Short names that were turned down:
- `ahd`: a dormant PyPI package (2022) and a dormant npm package (2017) each install an `ahd` command.
- `aha`: the ANSI HTML Adapter installs an `aha` command and is in Homebrew and Debian/Ubuntu. Installed hook commands could run it instead.

## 2. Decisions

- D12-1: Name map (applied in this order, case-sensitive):

  | From | To |
  | ---- | -- |
  | `agent-harness-kit` (crates, npm, paths, URLs, prose) | `agent-harness-adapter` |
  | `agent_harness_kit` (Rust crates, Python module) | `agent_harness_adapter` |
  | `six5536-agent-harness-kit` (PyPI) | `six5536-agent-harness-adapter` (follows from row 1) |
  | `ahk` (binary, `bin/ahk.js`, commands in docs and hook templates) | `agent-harness-adapter` |
  | `ahk` (temp dir prefix in `test_support.rs`) | `aha` |
  | `AHK` (spec code, schema titles, env vars `AHK_NODE_ADDON` / `AHK_P*`) | `AHA` |
  | Manifest key `ahk` (path of the bridge command, `manifest/file.rs`) | `adapter` |

  Every rule matches whole words only. `ahk` inside other words is left alone.
- D12-2: There is one command name and no short alias, so it never clashes. Hook templates and docs use the full name, e.g. `agent-harness-adapter hook {harness} {event} -- <command>` (D11-7, D11-9). A short alias can be added later if one comes up that's free.
- D12-3: Nothing has been published (no tags, `@six5536/*` is 404), so the contracts keep `version`/`v` = 1. The manifest key (`ahk` → `adapter`) and the schema titles change without a version bump. This is an exception to D11-8, which would need a bump if anything had been published.
- D12-4: Spec code `AHK` → `AHA`: `REQ-AHK-ahk.md` → `REQ-AHA-cli.md` and `DESIGN-AHK-ahk.md` → `DESIGN-AHA-cli.md`. Every ID and every `@zen-*` marker changes with it (38 IDs, 17 files).
- D12-5: Codes `KIT`, `HAR` and `BND` stay; IDs are labels, not names. `REQ-KIT-harness-kit.md` / `DESIGN-KIT-harness-kit.md` → `REQ-KIT-harness-adapter.md` / `DESIGN-KIT-harness-adapter.md`.
- D12-6: Prose: "the kit" / "harness kit", where it names the product, → "the adapter" / "the library" / "the core", whichever reads best. "Kit" in its everyday sense (a set of parts) stays. This is a reviewed pass, not a find-and-replace.
- D12-7: History stays: PLAN-009, PLAN-010, PLAN-011 and the existing CHANGELOG entries are not rewritten. PLAN-011 gets one line pointing to PLAN-012. Its P9 release then publishes under the new names.
- D12-8: Devcontainer volume names stay (`agent-harness-kit-cargo-target`, `-sccache`, `-mise-data`, `-claude`). Renaming them would throw away the build caches and the `~/.claude` volume. Only the display `name` changes.
- D12-9: These are outside this repo and are for the user to do, not in this commit: renaming the GitHub repo (GitHub then redirects the old URLs), the git remote URL, the local folder `/workspaces/agent-harness-kit`, and the npm/PyPI trusted-publisher settings for the new names.
- D12-10: One commit for the whole rename, made with `git mv` so file history follows the renames.

## 3. Functional requirements

| ID | Requirement |
| -- | ----------- |
| F1 | `cargo build --workspace`, every Rust test, `cargo clippy -D warnings` and `cargo fmt --check` pass. |
| F2 | The npm launcher tests, the Node binding tests and the Python binding tests pass; `scripts/launcher-smoke.mjs` and `scripts/release-smoke.mjs` pass with `agent-harness-adapter`. |
| F3 | `agent-harness-adapter schema <x>` output equals `schema/*.json`, regenerated (titles `AHA ...`). |
| F4 | `git grep -IiE 'agent[-_]harness[-_]kit\|\bahk\b'` finds only the exceptions in D12-7 and D12-8. |
| F5 | Version scripts (`set-version.mjs`, `verify-version.mjs`) and `package-lock.json` agree on the new package names (`npm install` regenerates the lock). |
| F6 | Traceability holds: every `@zen-impl`/`@zen-test` `AHA-*` marker resolves to an ID in REQ-AHA/DESIGN-AHA, and no `AHK-*` IDs remain. |

## 4. Phases

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P0 | Wait for a clean tree (`git status` empty, other agent's work committed). Re-run the inventory: it may have changed. | Tree clean |
| P1 | Directories and files (`git mv`): `crates/{lib,app,bind}/agent-harness-kit*`, `packages/agent-harness-kit*`, `python/agent_harness_kit`, `bin/ahk.js` → `bin/agent-harness-adapter.js`, spec files (D12-4, D12-5). | `git status` shows renames only |
| P2 | Manifests: root and crate `Cargo.toml` (package, `[[bin]]` name, `[workspace.dependencies]`, lib names), every `package.json` (name, `bin`, optionalDependencies, repository/bugs/homepage URLs), `pyproject.toml` (name, `module-name`), `build-node.mjs`, `validate-consumers.sh`, workflows `checks.yml` / `release.yml` (paths, artefact names, binary name). | `cargo metadata`, `npm install` succeed |
| P3 | Code & tests: Rust `use agent_harness_kit_core` → `agent_harness_adapter_core`, manifest key `ahk` → `adapter` and its default command, the default hook template, schema titles, env vars, JS launcher/addon/tests, Python package/tests, `examples/`. `@zen-*` markers `AHK` → `AHA`. | F1, F2 |
| P4 | Regenerate `schema/*.json` from `agent-harness-adapter schema`. | F3 |
| P5 | Specs & docs: REQ-AHA/DESIGN-AHA IDs, ARCHITECTURE (layout, names), REQ/DESIGN-KIT, -BND, -HAR mentions, READMEs (root, crates, packages, badges), CHANGELOG "Unreleased" entry for the rename, prose pass (D12-6), PLAN-011 pointer (D12-7). | F4, F6 |
| P6 | Full check: F1–F6. One commit (D12-10). Then the consumers (§5, P6a–P6d, F7). | All green |
| P7 | User: rename the GitHub repo, update the remote, set up trusted publishing for the new names (D12-9), push the consumer commits (D12-12). Then PLAN-011 P9 can release. | Repo renamed |

## 5. Consumers: smllm and sokf

Both use the library through a path dependency (D11-4), checked out under `consumers/` (gitignored) on branch `agent-harness-kit-0.1`. Neither uses the `ahk` CLI or its manifest; they call the Rust library directly. After P1 renames the directory, both stop building until they are updated, so they are updated in the same session as P1–P6.

| Repo | References | Where |
| ---- | ---------- | ----- |
| smllm | 74 in 23 files; 37 `agent_harness_kit_core` | root `Cargo.toml` (workspace dep + path), `crates/app/smllm`, `crates/lib/smllm-format`, `.zen/specs` (ARCHITECTURE, DESIGN-HOST/-CLI/-CFG/-STO/-NFR, REQ-HOST), CHANGELOG |
| sokf | 80 in 29 files; 45 `agent_harness_kit_core` | root `Cargo.toml` (workspace dep + path, `version = "0.1.0"`), `crates/app/sokf`, `crates/lib/sokf-core` (src, tests, README), `knowledge/` (architecture, contracts, designs, json), README, CHANGELOG |

- D12-11: Same name map as D12-1 (rows 1–2 only): `agent-harness-kit-core` → `agent-harness-adapter-core`, `agent_harness_kit_core` → `agent_harness_adapter_core`, and the path → `../../crates/lib/agent-harness-adapter-core`. Prose that calls it "the kit" gets the D12-6 pass. Their own plans and CHANGELOG entries that are already done stay as they are (D12-7); each CHANGELOG gets a new "Unreleased" line instead.
- D12-12: Each repo gets one commit on its current branch (`agent-harness-kit-0.1`), following that repo's own conventions (`AGENTS.md`/`CLAUDE.md`). The branch name stays: it is local, and renaming it is the user's call. Nothing is pushed without the user's go-ahead.
- D12-13: `scripts/validate-consumers.sh` (P2) patches `agent-harness-adapter-core` to the new path. It is the check for both repos.

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P6a | Before P1: check that `consumers/smllm` and `consumers/sokf` have clean trees, and re-run their inventory. | Both clean |
| P6b | smllm: `Cargo.toml` deps and path, `use` paths, specs and docs (D12-11). `cargo update -p agent-harness-adapter-core` refreshes `Cargo.lock`. Commit (D12-12). | `validate-consumers.sh smllm` passes |
| P6c | sokf: same as P6b. | `validate-consumers.sh sokf` passes |
| P6d | `git grep -IE 'agent[-_]harness[-_]kit'` in each finds only history (D12-11). | F7 |

F7: Both consumers build, pass clippy and pass their tests against the renamed library via `scripts/validate-consumers.sh`, and their only remaining old-name hits are history.

## 6. Risks

- The other agent's in-flight changes touch renamed files (`schema.rs`, `pyproject.toml`, `pi/adapter.rs`, the Node addon). P0 must wait for them to land; doing the rename first would cause conflicts on every renamed path.
- A blind `ahk`/`kit` replace would break words that contain them. Use word-boundary rules (D12-1) and the reviewed prose pass (D12-6).
- In docs and hook templates, `agent-harness-adapter` is longer than `ahk`. Check that README examples and generated hook entries still read well, and wrap lines where needed.

## 7. Implementation notes (2026-10-07)

- CHANGELOG: the `[0.1.0]` section was renamed in place rather than left as history with an "Unreleased" rename line (D12-7). 0.1.0 has never been released, so that section is the notes of the first release and must name what ships.
- `ahk` as a code identifier (not a command name) became `adapter` (Rust test helper, Python example variable) or `aha` (the binding tests' module alias); every temp-dir prefix became `aha-`, not only `test_support.rs` (D12-1). The env var with the binary's path in the binding tests and CI is `AHA`.
- The CLI's own error variant `Error::Kit` became `Error::Core`; the version refusal reads "version N is not one agent-harness-adapter reads (1)".
- Prose (D12-6): "the kit" became "the library" in library code, specs and the core README, and "the adapter" in the project README and SECURITY.md.
- Consumers (§5): renamed and committed locally (smllm `e9ec8cd`, sokf `04164a8`, on `agent-harness-kit-0.1`); both pass `scripts/validate-consumers.sh` (smllm 143 tests; sokf 638, 2 skipped). Their prose about "the kit" now says "the adapter" (sokf's test module `common/kit.rs` became `common/adapter.rs`). Their *Unreleased* CHANGELOG entries that named the kit were corrected in place (not yet released, same reasoning as above); released entries, plans and architecture change-log lines stay. Old-name hits left: smllm's released CHANGELOG entry and two finished plans; none in sokf.
- Checked: 177 Rust tests, clippy, fmt, docs, MSRV 1.85, publish dry run, coverage (library 98.5%, CLI 96.5%, binding glue 95.2%), npm launcher 9/9, Node binding 10/10, Python binding 7/7, the Python example, release and launcher smoke tests, actionlint, the trace (no `AHK` IDs left), F4 (only the devcontainer volumes and history plans).
