# Deferred Items - Phase 01 Foundation

## Workspace Dependency Compilation Issues

**Discovered during:** Plan 01-04, Task 2 verification
**Impact:** `cargo test` and `cargo clippy` fail at workspace level due to upstream crate API changes

### mcp-preview (crates/mcp-preview/src/handlers/websocket.rs)
- **Issue:** axum 0.8.7 changed `Message::Text` to expect `Utf8Bytes` instead of `String`
- **Fix:** Add `.into()` calls on lines 70 and 99

### mcp-tester (crates/mcp-tester/src/oauth.rs)
- **Issue:** rand 0.10 moved `random()` method to `RngExt` trait
- **Fix:** Add `use rand::RngExt;` import on line 11

**Note:** These are pre-existing issues in workspace dependencies, not caused by cargo-pmcp changes. All cargo-pmcp tests pass when compiled from cached artifacts.
