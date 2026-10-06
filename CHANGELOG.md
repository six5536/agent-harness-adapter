# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
agent-harness-kit uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
While it is pre-1.0, minor versions may contain breaking changes.

Every released tag needs its own section here. The release workflow refuses to
publish a version it cannot find a heading for, and that section becomes the
GitHub release notes.

## [Unreleased]

## [0.1.0]

First release as a crate of its own.

### Added

- `harness`: `Tool`, `Profile`, `Part` (`files`, `region`, `region_chosen`, `merge`,
  `external`), `MergeOp` (`array_entry`, `object_member`, `group_entry` with `EntryMatch`), `install` /
  `status` with `InstallOptions`, `HarnessResult`, `PartResult` (`State` and
  `Option<Action>`), `DeclinedStore` / `TomlDeclined`, `Markers`.
- `claude`: `HookInput`, `Answer`, `emit`, `instructions_file`, `instructions`,
  `hook_command`.
- `LoopGuard`, `report` (`Finding`, `Report`, `Severity`), `cli`, `fs`.
- `Error` with one variant per kind of refusal.
- Optional feature `schemars`: `JsonSchema` on the result types.
- MSRV 1.85.

[Unreleased]: https://github.com/six5536/agent-harness-kit/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/six5536/agent-harness-kit/releases/tag/v0.1.0
