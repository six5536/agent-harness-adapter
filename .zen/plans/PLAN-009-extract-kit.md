# PLAN-009: Extract agent-harness-kit into its own repository and publish it

| Meta               | Value                                                                 |
| ------------------ | --------------------------------------------------------------------- |
| Status             | in progress: P1–P7 done (2026-10-06); P8 (publish) needs the CARGO_REGISTRY_TOKEN secret |
| Workflow direction | top-down from a mechanical baseline (restructure → architecture → requirements → design → code → docs → release) |
| Traces to          | smllm ARCHITECTURE (agent-harness-kit component), HOST-10, HOST-11, HOST-Claude, CLI conventions, NFR-4, NFR-6, CFG-Findings |

## 1. Goal

This repository, a clone of smllm, becomes **agent-harness-kit**: the kit crate on its own, published on crates.io, used by smllm, sokf and other tools. Before the first publish its API is put right (D9-7). smllm then depends on the published crate instead of its own copy.

## 2. Starting point

- `crates/lib/agent-harness-kit`: about 4.6k lines (src + tests). It is `std` and has no smllm dependency.
- Modules: `harness`, `hook`, `report`, `cli`, `fs` (plus private `hash`, `error`).
- Dependencies: `serde` (std), `serde_json` (`preserve_order`), `thiserror`, `toml_edit`. Dev dependencies: `insta`, `proptest`.
- The kit has no specs of its own. Its behaviour is specified inside smllm's HOST, CLI, CFG and NFR specs.
- `origin` still points at the private smllm repository. Nothing may be pushed until it is changed.

## 3. Decisions

- D9-1: Fresh history. The extracted tree becomes one initial commit in a new git repository. smllm's history stays in smllm.
- D9-2: Publish to crates.io now, as `agent-harness-kit` 0.1.0. The name is free (checked 2026-10-06).
- D9-3: The source lives in a public repository, `github.com/six5536/agent-harness-kit` (`repository`/`homepage` in Cargo.toml). crates.io is the only package registry.
- D9-4: The crate sits at the repository root (`Cargo.toml`, `src/`, `tests/`). There is no workspace and no `crates/` folder.
- D9-5: Strip the repository to a Rust crate. Remove everything smllm owns: the other crates, `packages/`, `plugin/`, `.claude-plugin/`, `examples/`, `schema/`, `scripts/`, `docs/`, `.smllm/`, `reference_code/`, `submodules/sokf`, npm (`package.json`, lock, `node_modules`), the smllm hooks and permission in `.claude/settings.json`, the smllm server in `.mcp.json`, and the smllm-only sections in `AGENTS.md`.
- D9-6: Write new specs for the kit: `ARCHITECTURE.md`, `REQ-KIT-harness-kit.md` and `DESIGN-KIT-harness-kit.md`, from the current behaviour. Add `@zen-component` / `@zen-impl` / `@zen-test` markers to the code and tests. Delete smllm's specs, plans and tasks; this plan stays as the record.
- D9-7: The API is put right before the first publish, because every `pub` item becomes API once it is on crates.io: narrowed (D9-10), typed (D9-13, D9-14), made idiomatic (D9-15), kept free of other crates' 0.x types (D9-16), harness-neutral (D9-17). Compatibility with smllm is not a constraint; smllm adapts (F11). Behaviour, file formats and the text output stay the same, except where a decision says otherwise. Doc comments that use `smllm` as an example may stay.
- D9-8: Keep the MIT license and Rust 2024 edition; the license holder stays as it is today. MSRV is `rust-version = "1.85"` (the edition 2024 floor; the newest std feature the kit uses, `Option::is_none_or`, is 1.82). A CI job checks it on 1.85. Development stays on 1.98 (clippy, fmt, coverage). README policy: an MSRV raise is a minor release, made only when needed.
- D9-9: Versions come from `Cargo.toml` only. A release is a `vX.Y.Z` tag that matches `Cargo.toml` and has a CHANGELOG section, and it is published by CI with a `CARGO_REGISTRY_TOKEN` secret.
- D9-10: The public API (module layout per D9-17) is everything a tool needs, plus pieces that are useful on their own. Everything else becomes `pub(crate)`.
  - Public:
    - `cli`, `fs`, `claude` and `report`, all whole, and `LoopGuard`.
    - `Error`, `Result`.
    - `harness`:
      - the install surface: `Tool`, `Scope`, `Profile`, `Part`, `MergeOp`, `ExternalPart`, `DeclinedStore`, `TomlDeclined`, `InstallOptions`, `HarnessResult`, `PartResult`, `State`, `Action`, `install`, `status`;
      - standalone helpers: `Markers` with `find` / `render` (marker regions, D9-15).
  - Crate-private: the merge internals (`apply`, `extract`, `remove`, `render_merge`, `render_unmerge`, `json_text`, `indent_of`), state detection (`observe`, `expected`, `state`, `hash`, `Found`, `Observed`), writes (`Plan`, `apply_plan`, `write_if_changed`), the record (`Record`, `read_record`, `render_record`), `parse_json`, `parse_toml` (D9-16), `render_files`, `target_path`, `Content`, `Target` (D9-15), `set_declined_text`, `hash_text` and `normalise`.
  - Tests: the proptests and integration tests that reach internals (`properties_harness.rs`, part of `harness.rs`) move into `#[cfg(test)]` modules in `src/`. `tests/` keeps tests that go through the public API only.
- D9-11: Dependencies.
  - Keep `serde`, `serde_json` with `preserve_order`, and `toml_edit`.
  - `preserve_order` is needed to keep the user's key order when merging. Cargo turns it on for the whole build, so the README states this.
  - `toml_edit` reads and writes the record (`harness.toml`) and the declined parts, keeping comments.
  - Drop `thiserror` and write `Display` and `Error` for `Error` by hand. That removes `syn` v3 and four other crates from the build.
  - Not now: replacing the merge internals with an extracted `smllm-json`. It lacks pretty output, an editing API and serde interop, and `MergeOp` would change. Revisit in a later plan if a user objects to `preserve_order`.
- D9-12: Make the API safe to grow and free of smllm wording.
  - Add `#[non_exhaustive]` to the public types that may grow: `HookInput`, `Answer`, `InstallOptions`, `PartResult`, `HarnessResult`, `Error`, `State`, `Action`, `Severity`, `MergeOp` and `Finding`. (`Part`, `Profile` and `Report` keep their fields private, D9-15.) Each one that is built outside the crate gets a constructor.
  - Rewrite doc text that is specific to smllm, for example "a rejected event" on `EXIT_ERRORS`.
- D9-13: Structured errors. `Error` (`#[non_exhaustive]`) gets one variant per kind of refusal, and each message stays word for word what it is today.
  - `UnknownScope { name }`
  - `UnknownProfile { harness }`
  - `UnknownPart { harness, part }`
  - `File { file, message }`: a file that does not parse, or has the wrong shape (JSON, TOML, the record, the declined parts); `file` is the display name
  - `Refused(String)`: for an `ExternalPart` or a `DeclinedStore` written by a user
  - `Io { path, source }`
  - `Internal(String)`

  `Harness(String)` goes. smllm's external part moves to `Refused` (F11).
- D9-14: `PartResult` gets two typed fields instead of the `state` string.
  - `state: State` is what was found before any write.
  - `action: Option<Action>` is what `install` did: `Action { Created, Rewrote, Updated }` (`#[non_exhaustive]`). `None` means left as found, and `status` always gives `None`.
  - `PartResult::verb()` gives the report word (the action, else the state), so `to_text()` output is unchanged.
  - With `--force`, an edited part reads `Edited` + `Updated`.
  - The JSON gains `action` (left out when `None`), and `state` becomes the state that was found. This is a visible change to `install --json`; it goes in the CHANGELOG and F11.
- D9-15: Idiom fixes (accepted).
  1. `Part`: one private enum per kind replaces the pub `target` + `content` pair, which allows invalid combinations (an external part's target is ignored; a merge into `Target::Instructions`). Built only through the existing constructors, with a `name()` accessor. `Content` and `Target` stop being public.
  2. `Profile`: drop `hooks` and `has_hook`. Nothing uses them, and they describe a CLI's hook command, not install data. `Profile::new(harness, parts)`.
  3. `MergeOp`: paths become key segments (`impl IntoIterator<Item = impl Into<String>>`), so a key may contain `.`. Every value parameter takes `impl Into<Value>`.
  4. `InstallOptions::new(harness, scope)` with `.without(..)` and `.force(..)`. `HarnessResult.root` becomes a `PathBuf`.
  5. `Markers`: `find_region` / `render_region` become the methods `Markers::find` / `Markers::render`.
  6. `Finding`: `authority: Option<String>` (it was `""` for none). Built as `Finding::error(path, message)` with `.line(n)` / `.authority(a)`, replacing the five positional arguments. `Severity` loses `Default`.
  7. `Report`: private lists, kept ordered and de-duplicated as findings are pushed (no `finish()` step to forget), read through `errors()` / `warnings()` / `info()`. `report_text(report, warnings, info)` becomes `Report::to_text(&self, show: &[Severity])`; errors are always shown.
  8. `HookInput::parse` returns `Result`, so the caller chooses to treat bad input as empty (smllm keeps doing that with `unwrap_or_default`).
  9. `fs::write_atomic` takes `impl AsRef<[u8]>`.
- D9-16: No type from another crate's 0.x release (`toml_edit`) appears in the public API, so a bump of one of them is never a breaking kit release. `parse_toml` and `parse_json` become crate-private. smllm parses its `config.toml` with `toml_edit` itself and maps the failure to `Error::File` (F11).
- D9-17: Harness-neutral core, one module per harness. The kit must serve any harness, and adding one must never break the API.
  - Public modules: `harness` (neutral), `claude` (Claude Code), `report`, `cli` and `fs`. `LoopGuard` moves to the root, and the `hook` module goes.
  - `claude`: `HookInput`, `Answer`, `emit`, `instructions_file` (the `CLAUDE.md` / `@AGENTS.md` rule), `instructions(name, block) -> Part`, and `hook_command(event, prefix, command) -> MergeOp` (the shape of `settings.json` hooks).
  - `harness` stays neutral:
    - `MergeOp::HookGroup` is renamed `MergeOp::GroupEntry`: the tool owns entries inside groups of a JSON array, found by a command prefix.
    - `Part::instructions` is replaced by `Part::region_chosen(name, choose, block)`, where `choose: fn(&Path) -> Result<String>` picks the file under the root at install time (the private `Target::Chosen`). `claude::instructions` uses it.
  - Growth without breaks:
    - `Scope` is `#[non_exhaustive]`, ready for scopes such as a local one.
    - New part kinds come as new `Part` constructors over the private kind enum (D9-15).
    - New trait methods on `Tool`, `ExternalPart` and `DeclinedStore` always have defaults.
    - A new harness is a new module beside `claude`.
- D9-18: Before the first publish, check the API against both consumers, smllm and sokf, from this repository. Their repositories are private, and no committed file names their URLs, this plan included.
  - Each consumer is ported on a branch of its own repository: smllm per F11, and sokf by replacing `sokf-core/src/harness/` with the kit.
  - Checkouts: `consumers/smllm` and `consumers/sokf` are gitignored and never committed, so this public repo names no private URL in its files. `scripts/validate-consumers.sh` reads the clone URLs from `SMLLM_REPO` / `SOKF_REPO` (no defaults), clones or pulls each one, checks out its port branch, and runs `cargo nextest run` and `cargo clippy -D warnings` with `--config 'patch.crates-io.agent-harness-kit.path="<this repo>"'`, so neither consumer's `Cargo.toml` changes for the check. It runs locally before every release. CI does not run it, because the consumers are private.
  - A misfit is fixed in the kit, never worked around in a consumer.
  - Publish only when both build and pass their tests. Then both switch to `agent-harness-kit = "0.1"`.
- D9-19: No provenance. Remove the `// Derived from sokf …` header from every file, and remove every mention of sokf as the code's origin: README, crate docs, ARCHITECTURE rules, CHANGELOG. sokf is named only as a consumer (D9-18).
- D9-20: The sokf port (P7) showed where the kit is narrower than a second tool needs. What helps any tool goes into the kit now; the rest stays in sokf.
  - Into the kit:
    - `EntryMatch` (prefix or contained text) for group entries, since a tool's program may be renamed.
    - An exact region round trip: only the framing blank line is dropped, and an empty block gets none.
    - An optional `schemars` feature for the result types.
  - Kept in sokf:
    - Its `/`-prefixed path display.
    - Its report module, which has fix operations, code projections and its own text form.
  - sokf adopts the kit's behaviour in three places, as fixes:
    - The tool owns its entries, never the whole group, so the user's hooks in a shared group are kept.
    - Declined parts are never read.
    - The stray-marker rule.
  - Incompatibility, accepted by the user (no one uses sokf yet): the kit's record hashes (a region by its words, merge entries as an array) differ from sokf's. After the upgrade, a sokf part that is stale but unedited reads as `edited` once and needs `--force`. Current parts are unaffected, and the next install rewrites their hashes.

## 4. Functional requirements

| ID | Requirement |
| -- | ----------- |
| F1 | The repository root is the crate `agent-harness-kit`, built from `crates/lib/agent-harness-kit` at commit `8b07ed4`, with its behaviour and its tests (adapted to the API of D9-10..D9-17). |
| F2 | `Cargo.toml` is self-contained: the dependency versions taken from the old workspace, the release profile only if a library needs it (it does not, so drop it), and the crates.io metadata (description, keywords, categories, readme, license, repository, `rust-version`). |
| F3 | `ARCHITECTURE.md` describes the kit only: purpose, modules, dependencies, rules (no tool content, no tool dependency, atomic writes, exit codes), directory structure, developer commands, release status. |
| F4 | `REQ-KIT` states, in EARS, what the kit guarantees today: part kinds and states; install/status; never writing on a refusal (NFR-6 semantics); target file choice (HOST-10); region rules (HOST-11, PLAN-003 F21); JSON merge ownership (PLAN-003 F20); declined parts (F19); Claude Code hook input, answers, `emit` and its instructions file rule; `LoopGuard`; findings and the text report; exit codes and broken pipes; `fs::read_text` / `write_atomic`. |
| F5 | `DESIGN-KIT` describes the target API (D9-10..D9-17) and maps components (`KIT-Harness`, `KIT-Claude`, `KIT-LoopGuard`, `KIT-Report`, `KIT-Cli`, `KIT-Fs`) to the ACs, with correctness properties for the existing proptests. Markers in the code and tests complete the trace. |
| F6 | `README.md` (repository root = crate readme) covers what the kit is, installation, a minimal `Tool` example, the modules table (add the missing `fs` row), MSRV, the `preserve_order` note (D9-11) and the license. |
| F7 | `CHANGELOG.md` restarts with a `0.1.0` section for the first release. |
| F8 | The CI workflows (`checks.yml`): fmt, clippy `-D warnings`, nextest, doctests, `cargo doc` with warnings as errors, `cargo check` on the MSRV toolchain (1.85), `cargo publish --dry-run`, coverage ≥ 90% lines, cargo-deny. `ci.yml` stays the trigger (push and PR) of the reusable `checks.yml`. Keep the macOS / Windows test matrix and `audit.yml`. |
| F9 | `release.yml`: on a `v*` tag, check that the tag matches `Cargo.toml` and that the CHANGELOG has a section for it, run the checks, `cargo publish`, and create a GitHub release with that CHANGELOG section. |
| F10 | `AGENTS.md`, `.zen/rules/rust-rules.md`, the devcontainer, `.mise.toml`, `deny.toml`, `.gitignore`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `.github/dependabot.yml` (drop npm), the issue and PR templates and `rust-toolchain.toml` (stays 1.98 for development) name the kit and lose their smllm-only parts: the rules' WASM section, zig, wasm-bindgen, node, the npm ignores, and the smllm note in `AGENTS.md`. |
| F11 | smllm port (on its own repo, D9-18): depend on the kit, delete `crates/lib/agent-harness-kit`, adapt to the new API (errors, `PartResult`, `claude` module, `Finding` / `Report`, `HookInput::parse`, its own TOML parse), point its ARCHITECTURE and its HOST and CLI specs at the external crate and the `REQ-KIT` IDs. |
| F12 | sokf port (on its own repo, D9-18): replace `sokf-core/src/harness/` and its hook, report and CLI helpers with the kit; sokf's behaviour and tests stay the same. |
| F13 | `scripts/validate-consumers.sh` and the `consumers/` gitignore entry (D9-18). |

## 5. Non-functional requirements

| ID | Requirement |
| -- | ----------- |
| N1 | Files ≤ 800 lines. The module rules in `rust-rules.md` still apply. |
| N2 | No new runtime dependency. Dependencies stay at their latest versions (rust-rules). |
| N3 | Line coverage ≥ 90%, enforced in CI. |
| N4 | `cargo package --list` ships only `src/`, `README.md`, `LICENSE`, `CHANGELOG.md` and `Cargo.toml`, with no specs, tests snapshots or dev files (`include`/`exclude`). |
| N5 | Every public item is documented (`#![warn(missing_docs)]` stays); docs.rs builds cleanly. |

## 6. Phases

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P1 Restructure | Mechanical: move the crate to the root (F2) and delete the smllm content (D9-5), trim the tooling (F10). API and behaviour unchanged. | `cargo nextest run`, `clippy -D warnings` and `cargo doc` pass; `git grep -i smllm` finds only doc examples |
| P2 Specs | ARCHITECTURE (F3), REQ-KIT (F4), DESIGN-KIT for the target API (F5). Delete smllm's specs, plans (this one excepted) and tasks. | Every requirement has an AC; every AC is assigned to a component |
| P3 API | Rewrite to DESIGN-KIT (D9-10..D9-19, F1), adding the markers as it goes. Tests are adapted, never weakened. | P1's checks pass; every behaviour test from `8b07ed4` exists in some form; every AC has an `@zen-impl` and an `@zen-test`, or a stated reason why not |
| P4 Docs | README (F6), CHANGELOG (F7). | `cargo package --list` matches N4; `cargo publish --dry-run` passes |
| P5 CI / release | `ci.yml`, `checks.yml`, `release.yml`, `audit.yml`, deny (F8, F9). | Each workflow step's command passes locally; the workflows run for real in P6 |
| P6 New repository | Remove `.git` (after confirming nothing unpushed is needed), `git init`, one initial commit, add origin `six5536/agent-harness-kit`, push. Add the `CARGO_REGISTRY_TOKEN` secret. | The repository is public and CI is green |
| P7 Validate | F11, F12 and F13 against this checkout (D9-18); fix the kit for any misfit (through DESIGN-KIT first). | smllm and sokf build, pass clippy and pass their tests on the local kit |
| P8 Release | Tag `v0.1.0` and publish. | The crate is on crates.io and docs.rs |
| P9 Switch consumers | smllm and sokf depend on `agent-harness-kit = "0.1"`; merge their port branches. | Both pass CI against the published crate |

## 7. Risks

- P6 deletes this clone's `.git`, which holds smllm's history and has `origin` pointed at smllm. Check `git status` and unpushed commits first (the devcontainer rename is the only local change). Never push to the smllm remote.
- `[patch.crates-io]` for a crate not yet on crates.io: if Cargo refuses it, the port branches use a `path` dependency to `../..` until P9.
- A crates.io publish cannot be undone, only yanked. Run P8 only after a dry run and a review of the package list.

## 8. Implementation notes

- The unused unmerge code (`render_unmerge`, `remove`) was dropped (uninstall is out of scope in REQ-KIT). `remove` survives only as a helper inside the merge property test.
- `node` stays in `.mise.toml`, because the devcontainer installs Claude Code through mise's npm backend. The npm workspace itself is gone.
- `Cargo.lock` is committed, and CI runs with `--locked`. The MSRV job builds the same lockfile on 1.85; it caught a test that compared a `PathBuf` with a `String`, which only newer standard libraries allow.
- The README examples run as doctests (`#[cfg(doctest)]` include in `lib.rs`).
- `LICENSE` was the Elastic License 2.0 (since smllm's first commit) while `Cargo.toml`, README and D9-8 said MIT; the user chose MIT and the file now holds the MIT text.
- P7: smllm (134 tests) and sokf (626 tests, 2 skipped) pass on the kit, on local branches `agent-harness-kit-0.1` with a path dependency; both get pushed in P9, once they depend on the published crate. The two schema fixes from the sokf port (plain-text descriptions, `action` optional) went into the kit. Three requests stayed out of the kit, as test conveniences or sokf conventions rather than generic needs: a public part hash, reading a part's contents, and a `Tool::display_path` hook.

