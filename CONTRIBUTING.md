# Contributing to agent-harness-adapter

This repository is a read-only mirror of agent-harness-adapter. Each release reaches it as one commit, which an export from the project's own repository creates, and nobody commits to it directly. This guide explains how to report a bug or ask for a feature, and how to build agent-harness-adapter from this repository.

- [Report a bug or ask for a feature](#report-a-bug-or-ask-for-a-feature)
- [Build agent-harness-adapter](#build-agent-harness-adapter)

## Report a bug or ask for a feature

Open an issue, with the bug report or the feature request template. A pull request can't be merged here, because the next release replaces the whole tree. When you have a change in mind, describe it in an issue, and include a patch there if you have one.

## Build agent-harness-adapter

The project pins its toolchains in `rust-toolchain.toml` and `.mise.toml`, which [mise](https://mise.jdx.dev/) installs:

```sh
mise install
cargo build --locked -p agent-harness-adapter
```

The Node binding builds with `npm run build:node`, and the documentation site, in `docs/`, with `npm ci && npm run build` there.

`.github/workflows/release.yml` is the release: it builds the CLI for five platforms, the Node addons, and the Python wheels, publishes the crates to crates.io and the packages to npm, creates the GitHub release with the notes that the tag's message carries, and deploys the documentation site. The GitHub releases hold the notes of every release: they are the project's changelog.
