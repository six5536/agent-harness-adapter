# Rust Rules

## Cargo.toml

- All dependencies should be at their latest versions unless there is a good reason not to be. Check latest version when adding a dependency.
- No dependency will be added without user confirmation.

## Rust Module Rules

- `mod.rs` contains only `mod` declarations and `pub use` re-exports; all code lives in named files.
- Declare submodules privately (`mod foo;`) and expose their API via `pub use foo::Item;`; flatten re-exports at `lib.rs` so callers write `crate::Item`.
- Default to private; widen visibility via `pub(crate)` → `pub(super)` → `pub` only as needed.
- Group imports `std` → external → `crate`/`super`/`self`, collapse with nested paths, and prefer `use crate::...` over `super::super::.`
- Import types and traits directly; import the parent module for free functions (`module::func()`); no glob imports except preludes, enum variants in `match`, and tests.

## Library API

- No public API exposes a type from another crate's 0.x release (PLAN-009 D9-16).
- Public enums and structs that may grow are `#[non_exhaustive]`; each one built outside the crate has a constructor.
- New trait methods on public traits have default bodies.
- Harness-specific formats live in that harness's module (`claude`); `harness` stays neutral.
- Dependencies support the MSRV (`rust-version` in `Cargo.toml`).

## Unsafe Rust Code

- all `unsafe` blocks must be isolated in a dedicated module named `*_unsafe.rs` behind safe public functions.
- no unsafe code without user confirmation, even then must be clearly documented.
