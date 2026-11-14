# Contributing to agent-harness-adapter

This repository is a read-only mirror of agent-harness-adapter. Each release reaches it as one commit, which an export from the project's own repository creates, and nobody commits to it directly. This guide explains how to report a bug or ask for a feature, and how to build agent-harness-adapter from this repository.

- [Report a bug or ask for a feature](#report-a-bug-or-ask-for-a-feature)
- [Build agent-harness-adapter](#build-agent-harness-adapter)

## Report a bug or ask for a feature

Open an issue, with the bug report or the feature request template. A pull request can't be merged here, because the next release replaces the whole tree. When you have a change in mind, describe it in an issue, and include a patch there if you have one.

## Build agent-harness-adapter

The project pins its toolchains in `rust-toolchain.toml`, in `.mise.toml`, and in the files of `.config/mise/conf.d/`, which [mise](https://mise.jdx.dev/) installs:

```sh
mise install
cargo build --locked -p agent-harness-adapter
```

The Node binding's addon builds with `mise run build:node`, and the documentation site, in `docs/`, with `mise run site:build`.

To build the release artefacts as the release does, install the JavaScript workspace and run a build task, such as the command for Linux on x64:

```sh
npm install
mise run build:release:ubuntu:linux-x64
```

`mise tasks ls --hidden` lists a task for each artefact, and the middle part of each name says which operating system it runs on: the command, the Node addon, and the Python wheel for each of five platforms, and the Python source distribution.

`.github/workflows/publish.yml` is the release: with `build.yml`, it builds every artefact, smoke-tests the command, publishes the crates and the packages, and creates the GitHub release with the notes that the tag's message carries, and after it `docs.yml` deploys the documentation site. The GitHub releases hold the notes of every release: they are the project's changelog.
