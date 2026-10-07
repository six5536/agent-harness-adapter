# Contributing to agent-harness-adapter

How to get set up, what to run before you push, and how a release is cut.

> Status: pre-1.0. Minor versions may contain breaking changes.

## Prerequisites

Toolchains are pinned and managed with [mise](https://mise.jdx.dev/):

- `rust-toolchain.toml` pins the development toolchain (with `rustfmt` and `clippy`).
- `.mise.toml` pins everything else: a `nightly` Rust used only for coverage, the
  MSRV toolchain (`1.85`), and the cargo tools (`cargo-nextest`, `cargo-llvm-cov`).

```sh
mise install     # install all pinned tools
```

## Before you push

Run everything CI runs:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo nextest run
cargo test --doc
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo +1.85 check                     # MSRV
cargo publish --dry-run               # the package builds from what it ships
```

Coverage (slow; needs the nightly toolchain). The gate is 90% of lines:

```sh
cargo +nightly llvm-cov nextest --html                 # HTML report
cargo +nightly llvm-cov nextest --fail-under-lines 90  # the CI gate
```

Glue that genuinely cannot be tested is marked `#[cfg_attr(coverage_nightly, coverage(off))]`,
which is why coverage runs on nightly.

`cargo deny check licenses bans sources` also gates CI, but only fails when you
change dependencies.

## Documentation expectations

- Every public item has a doc comment (`#![warn(missing_docs)]`).
- `cargo doc` builds clean under `RUSTDOCFLAGS=-D warnings`.
- Rustdoc examples run as doctests.
- The specs in `.zen/specs/` move with the code: a change to behaviour or API
  updates `REQ-KIT` / `DESIGN-KIT` and the `@zen-*` markers.

## Tests

Tests run under `cargo-nextest`.

- **Unit and property tests** sit beside the code they cover, in `#[cfg(test)] mod tests`,
  including the `proptest` properties of the internals.
- **Integration tests** (`crates/lib/agent-harness-adapter-core/tests/`) drive `install` / `status` through the public API with a
  test `Tool` over a temporary directory.

## Consumers

`tests/consumers/validate-consumers.sh` builds and tests the tools that use the library
against this checkout before a release. Their repositories are private: set
`SMLLM_REPO` and `SOKF_REPO` to their clone URLs. The checkouts land in
`consumers/`, which is not committed.

## Commits and pull requests

- Use [Conventional Commits](https://www.conventionalcommits.org/)
  (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`).
- Keep PRs focused, and update the README and `CHANGELOG.md` when behaviour or API changes.
- CI runs the tests on Linux, macOS and Windows.

## Dependencies

Adding a dependency needs a clear reason. Reach for the standard library or a
crate already in the tree first. When you do add one, take the latest version
that supports the MSRV. No public API exposes a type from another crate's 0.x
release.

## MSRV

The minimum supported Rust version is in `Cargo.toml` (`rust-version`) and is
checked in CI. Raising it is a minor release, made only when needed.

## Releasing

Releases are tag-driven (`.github/workflows/release.yml`, on a `v*` tag). The
version lives only in `Cargo.toml`.

1. Set `version` in `Cargo.toml` and add a `## [X.Y.Z]` section to `CHANGELOG.md`.
   The release workflow refuses a version without one, and that section becomes the
   GitHub release notes.
2. Run `tests/consumers/validate-consumers.sh`.
3. Commit, tag `vX.Y.Z`, review with `git show vX.Y.Z`, then `git push --follow-tags`.
   Pushing the tag triggers the publish, which cannot be undone (only yanked).
4. The workflow checks that the tag matches `Cargo.toml` and has a changelog
   section, runs the full CI gate, runs `cargo publish` (with the
   `CARGO_REGISTRY_TOKEN` secret), and creates the GitHub release.
