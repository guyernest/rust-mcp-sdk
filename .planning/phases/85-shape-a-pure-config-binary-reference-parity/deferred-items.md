# Phase 85 — Deferred / Out-of-Scope Items

Items discovered during execution that fall outside the current plan's scope
(SCOPE BOUNDARY rule: only auto-fix issues directly caused by the current task's
changes). Logged here, NOT fixed.

## Discovered during Plan 85-03 (Wave 1, scaffold pmcp-sql-server + vendor fixtures)

### Pre-existing clippy lints surfaced by local toolchain (rust-1.95.0)

`cargo clippy -p pmcp-sql-server --all-features -- -D warnings` escalates
warnings across the whole compilation graph and so surfaces lints in the
`pmcp-server-toolkit` dependency, NOT in the new `pmcp-sql-server` crate. These
are the same pre-existing rust-1.95.0 lints already logged in Phase 84's
`deferred-items.md` (local toolchain newer than CI's pinned stable):

| File | Lines | Lint | Owner / disposition |
|------|-------|------|--------------------|
| `crates/pmcp-server-toolkit/src/builder_ext.rs` | 284 | `clippy::needless_return` | Phase 83 code; fix when CI toolchain advances |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | 207-208 | `clippy::field_reassign_with_default` | Phase 83 code; same disposition |

`crates/pmcp-sql-server/src/*` is clippy-clean at `-D warnings` (0 warnings
attributable to this crate's sources). The deferral matches the Phase 84
precedent and the STATE.md note that broad `make quality-gate` is blocked by
pre-existing unrelated rust-1.95.0 pedantic lints in workspace dependencies.
