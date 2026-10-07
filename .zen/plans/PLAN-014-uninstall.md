# PLAN-014: Uninstall

| Field              | Value |
| ------------------ | ----- |
| Status             | done (2026-10-07); the agent check (PLAN-013 A10) follows |
| Workflow direction | top-down (requirements → design → code & tests → docs) |
| Traces to          | REQ-KIT (KIT-2..KIT-8, KIT-19), DESIGN-KIT, REQ-AHA (AHA-3), REQ-BND (BND-1), PLAN-013 §5, PLAN-011 P9 |

## 1. Goal

Ship 0.1.0 with `uninstall`: take a tool's integration back out of the named harnesses at a scope, leaving the user's own content and anything another installed harness still uses. Library, CLI and both bindings.

## 2. Decisions

- D14-1: What is removed, per part of each named harness, by kind:
  - `file`: the part's files; then each directory the removal left empty, up to (not including) the root.
  - `region`: the block and its marker lines.
  - `merge`: the tool's entries, i.e. the inverse of each op: an array entry by equality, an object / TOML member by key, the owned entries by their match, the owned entries inside groups (a group left empty goes, as on install).
  - `external`: `ExternalPart::remove` (new method, default: not supported → the part is kept and reported).
  - A file the removal leaves empty (only whitespace, `{}`, or an empty TOML document) is deleted.
- D14-2: States decide, as on install (KIT-3):
  - `current` or `stale` (the user did not change it): removed.
  - `edited`: kept and reported; `--force` removes it. A region that is edited is removed whole with `--force` (the markers bound it).
  - `absent`, `skipped` (declined): nothing to do.
- D14-3: Shared content stays when still used. The harnesses left installed are those in the record minus the named ones. A part is kept, reported `kept (used by <ids>)`, when a remaining harness reads that item at the part's location (`Harness::reads`). Example: uninstall `codex` while `pi` stays: `AGENTS.md`'s block and `.agents/skills` stay.
- D14-4: The record drops each named harness's table; a record with no table left is deleted. The declined store's entry of each named harness is cleared (an uninstall forgets the choice).
- D14-5: `all` stands for the installed harnesses (the record), not every harness with the scope, so `uninstall --harness all` never touches a harness that was never installed. A named harness that is not installed is a no-op with an empty result entry, not an error.
- D14-6: Same guarantees as install: every removal is planned before any write; a refusal (a file that does not parse, an unknown harness) writes nothing (KIT-6); writes are atomic and only on change (KIT-7); external parts are removed first.
- D14-7: Result: the install result type. New `Action::Removed`; a part kept for D14-2 or D14-3 has no action and a note in `by` (the harnesses still using it) or its `edited` state. Text report: `removed <path> (<part>)`, `kept <path> (<part>, used by pi)`, `edited <path> (<part>)` plus the `--force` note.
- D14-8: API: `harness::uninstall(tool, &UninstallOptions { harnesses, scope, force })`. CLI: `agent-harness-adapter uninstall --manifest <file> --harness <ids|all> [--scope] [--root] [--force] [--json]`. Bindings: `uninstall(manifest, harnesses, scope, root, home, force)` / `uninstall(...)` in JS.
- D14-9: Contracts: the result's JSON gains the action `removed` (additive: the result schema is regenerated; no version field changes, nothing is published yet).
- D14-10: Out of scope: removing a harness's own config switches the tool never wrote; consumers (smllm, sokf) adding an `uninstall` command (they can, through the library, after the release).

## 3. Functional requirements

| ID | Requirement |
| -- | ----------- |
| F1 | Install then uninstall of the same harnesses at a scope leaves the tree byte-identical to before the install, when nothing else touched it (every harness, every scope, every part kind). |
| F2 | User content in the same files survives: text outside the region, other JSON / TOML entries and their order and formatting, other files in a skill directory. |
| F3 | D14-2 edited handling and `--force`. |
| F4 | D14-3 shared locations: uninstalling one of several harnesses keeps what the others read; uninstalling the last removes it. |
| F5 | D14-4 record and declined store. |
| F6 | D14-6 refusals write nothing. |
| F7 | CLI and bindings expose it (D14-8) with the same results. |

## 4. Non-functional requirements

| ID | Requirement |
| -- | ----------- |
| N1 | Library coverage stays ≥ 90%; property tests: install∘uninstall is the identity on a random tree (F1), and uninstall twice equals once. |
| N2 | No new dependencies. Files ≤ 800 lines (`install.rs` is near the limit: the removal goes in its own module). |

## 5. Specs

- REQ-KIT: new requirement "Uninstall" (D14-1..D14-6), KIT-3 states reused; `Action::Removed` in KIT-4's results.
- DESIGN-KIT: `KIT-Uninstall` component, the inverse merge ops, a property for F1.
- REQ-AHA / DESIGN-AHA: the `uninstall` command; REQ-BND / DESIGN-BND: the binding functions.
- README (core, CLI, bindings), CHANGELOG 0.1.0, ARCHITECTURE (install sequence gains its inverse).

## 6. Phases

| Phase | Work | Done when |
| ----- | ---- | --------- |
| P1 Specs | §5 | Reviewed |
| P2 Removal primitives | Inverse merge ops (JSON, TOML), region removal, file removal with empty-dir cleanup, empty-file deletion; unit and property tests | Tests pass |
| P3 `harness::uninstall` | Planning over the states, shared-location keep rule, record and declined store, `ExternalPart::remove`; integration tests per harness and scope, F1 property | F1–F6 |
| P4 CLI and bindings | Command, Python and Node functions, schema regenerated, release smoke step | F7 |
| P5 Docs | READMEs, CHANGELOG, ARCHITECTURE | Committed; PLAN-013 continues |

## 7. Implementation notes

- A part uninstall keeps because other installed harnesses read it gets a new action, `kept`, so its line reads `kept AGENTS.md (instructions, used by pi)` instead of borrowing install's `by` wording.
- The record is deleted whenever no table is left, even when unchanged: install of no harness writes an empty record, which the round-trip property found.
- `ExternalPart` gains `removable` (default `false`) and `remove` (default: a refusal), so existing external parts (smllm's `claude mcp`) compile and are kept with a warning until they add removal.
- Byte-for-byte round trip (KIT_P-12) holds for every harness and scope, with and without the user's own files beside the tool's.
