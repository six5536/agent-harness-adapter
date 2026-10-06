## Summary

<!-- What does this change, and why? -->

## Checklist

- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo nextest run` and `cargo test --doc` pass
- [ ] Line coverage stays >= 90% (see `CONTRIBUTING.md`)
- [ ] `cargo +1.85 check` passes (MSRV)
- [ ] Specs (`.zen/specs/`), rustdoc, README and `CHANGELOG.md` updated where behaviour or API changed
