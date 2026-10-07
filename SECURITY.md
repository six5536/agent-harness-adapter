# Security Policy

## Supported versions

agent-harness-adapter is pre-1.0. Security fixes target the **latest release** and
the `main` branch only. Older versions are not patched, so upgrade to the
latest release to pick up a fix.

## Reporting a vulnerability

Please report security issues **privately** through GitHub's private
vulnerability reporting:

1. Open the [Security tab](https://github.com/six5536/agent-harness-adapter/security)
   of the repository.
2. Click **Report a vulnerability** to start a private advisory.

Don't open a public issue for a suspected vulnerability. The maintainer will
acknowledge the report and coordinate a fix and a disclosure date with you.

## Scope

The adapter writes files a harness reads (instructions files, settings JSON, a
record file) on behalf of the tool that uses it. In scope:

- A write outside the root directory the tool names, or through a path the
  tool did not supply.
- A write that is not atomic, or that changes a file `install` reported as
  left alone (an `edited` part without `--force`, a declined part).
- A refusal that still writes some files.
- Supply-chain problems in what the release pipeline publishes: a publish from
  outside the release workflow, or a dependency that got past the `cargo-deny`
  and `cargo-audit` gates.
