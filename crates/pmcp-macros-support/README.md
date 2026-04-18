# pmcp-macros-support

Pure helpers for [`pmcp-macros`](https://crates.io/crates/pmcp-macros). This crate
exists because proc-macro crates cannot expose arbitrary public API, so the
rustdoc-harvest normalizer lives here instead, where property tests and fuzz
targets can consume it.

**This crate is a workspace-internal implementation detail of pmcp.** External
users should depend on `pmcp` (with the `macros` feature) or `pmcp-macros` directly,
never on this crate. API stability is not guaranteed.

## Version compatibility

- `pmcp-macros-support 0.1.x` → shipped alongside `pmcp-macros 0.6.x` → shipped alongside `pmcp >= 2.4.0`.
