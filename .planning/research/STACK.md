# Technology Stack

**Project:** pmcp-tasks (MCP Tasks for PMCP SDK)
**Researched:** 2026-02-21
**Context:** Brownfield -- adding `pmcp-tasks` crate to existing Rust workspace with MSRV 1.82.0

## Recommended Stack

### Core Framework (Inherited from Workspace)

These are already in the workspace and MUST be reused for consistency. No version changes needed.

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `pmcp` | path dep | Parent SDK, protocol types, server traits | The whole point -- `pmcp-tasks` extends this | HIGH |
| `serde` | 1.0 | Serialization/deserialization | Already in workspace; all protocol types are serde-based | HIGH |
| `serde_json` | 1.0 | JSON handling | Already in workspace; `preserve_order` feature already enabled | HIGH |
| `tokio` | 1 | Async runtime | Already in workspace; `sync` + `time` features needed for store locking and TTL timers | HIGH |
| `tracing` | 0.1 | Structured logging/instrumentation | Already in workspace; consistent observability | HIGH |

### New Dependencies for pmcp-tasks

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `async-trait` | 0.1 | Dyn-safe async traits for `TaskStore` | **Required.** Native `async fn in trait` (Rust 1.75+) is NOT dyn-safe. The `TaskStore` trait must be used as `Arc<dyn TaskStore>` for pluggable backends. `async-trait` desugars to `Pin<Box<dyn Future + Send>>` which enables this. The parent crate already uses it. | HIGH |
| `thiserror` | 2.0 | Derive Error for `TaskError` | Already in workspace at 2.0. Clean error type derivation without boilerplate. Latest: 2.0.18. | HIGH |
| `uuid` | 1.17+ | UUIDv4 task ID generation | Already in workspace with `v4` + `serde` features. 122 bits of entropy sufficient for task ID security. Latest: ~1.21. Use workspace version. | HIGH |
| `chrono` | 0.4 | ISO 8601 timestamps, TTL epoch calculation | Already in workspace with `serde` feature. Needed for `created_at`, `last_updated_at`, and DynamoDB TTL epoch seconds conversion. Latest: 0.4.43. | HIGH |
| `parking_lot` | 0.12 | Fast RwLock for in-memory store | Already in workspace. 1.5-5x faster than `std::sync::Mutex`. Used for `InMemoryTaskStore` internal HashMap. Latest: 0.12.5. | HIGH |

### DynamoDB Backend (Feature-Gated)

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `aws-sdk-dynamodb` | 1 | DynamoDB client | Official AWS SDK for Rust. Actively maintained (~1.100+ releases). Supports conditional writes, GSI queries, native TTL. Use semver `1` to allow patch updates. | HIGH |
| `aws-config` | 1 | AWS credential/region resolution | Required companion to `aws-sdk-dynamodb`. Provides `load_defaults()` for Lambda environment auto-configuration. Use `BehaviorVersion::latest()`. Latest: ~1.8. | HIGH |

### Dev/Test Dependencies

| Technology | Version | Purpose | Why | Confidence |
|------------|---------|---------|-----|------------|
| `tokio` (full) | 1 | Test runtime | Full feature set needed for `#[tokio::test]` | HIGH |
| `proptest` | 1.7+ | Property-based testing | Already in workspace dev-deps. State machine transition properties, variable merge commutativity, owner isolation. Latest: ~1.9. | HIGH |
| `insta` | 1.43+ | Snapshot testing | Already in workspace dev-deps. Ideal for JSON serialization round-trip tests (protocol types must match spec exactly). Use `json` + `redactions` features. Latest: ~1.46. | HIGH |
| `pretty_assertions` | 1.4 | Readable test diffs | Already in workspace dev-deps. | HIGH |
| `rstest` | 0.26 | Parameterized tests | Already in workspace dev-deps. Run same store tests against multiple backends via `#[rstest]` fixtures. | HIGH |
| `mockall` | 0.14 | Mock `TaskStore` trait | Already in workspace dev-deps. Generate mock implementations for unit testing `TaskContext`, `TaskHandler`, and `TaskMiddleware` without real storage. Latest: ~0.13. Use workspace version 0.14. | HIGH |

## Dependency Alignment Strategy

**Critical principle:** `pmcp-tasks` MUST NOT introduce version conflicts with the parent `pmcp` workspace.

### Shared Dependencies (use workspace version)

These dependencies are already in the workspace root `Cargo.toml`. The `pmcp-tasks` crate should reference them at compatible versions:

```toml
# These MUST match workspace versions to avoid duplicate compilation
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
async-trait = "0.1"
thiserror = "2.0"
uuid = { version = "1.17", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["sync", "time"] }
tracing = "0.1"
parking_lot = "0.12"
```

### New Dependencies (only in pmcp-tasks)

These are unique to `pmcp-tasks` and will not conflict:

```toml
# DynamoDB backend (optional, feature-gated)
aws-sdk-dynamodb = { version = "1", optional = true }
aws-config = { version = "1", optional = true }
```

### Why NOT use workspace dependency inheritance

The workspace `[workspace.dependencies]` section is not currently used in this project. Adding it would require refactoring the root `Cargo.toml` and all existing workspace members. This is out of scope for the tasks feature. Use explicit version pins matching the workspace root instead.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Async trait dispatch | `async-trait` 0.1 | Native `async fn in trait` (Rust 1.75+) | Native async traits are NOT dyn-safe. `TaskStore` requires `Arc<dyn TaskStore>` for pluggable backends. `async-trait` remains the only stable solution for dyn dispatch + Send. The `trait-variant` crate is not yet mature enough. |
| Error handling | `thiserror` 2.0 | `anyhow` | `thiserror` for library code (structured errors). `anyhow` is for application code. `pmcp-tasks` is a library -- callers need to match on `TaskError` variants. |
| UUID generation | `uuid` 1.x (v4) | `ulid` or `nanoid` | UUIDv4 is spec-compatible, already in workspace, 122 bits of entropy. ULID's sortability is unnecessary (DynamoDB GSI handles ordering via `CREATED#{iso8601}` sort key). |
| Timestamps | `chrono` 0.4 | `time` 0.3 | `chrono` already in workspace. Switching to `time` would introduce a new dependency for no benefit. `chrono` handles ISO 8601 and epoch seconds conversion needed for DynamoDB TTL. |
| In-memory store locking | `parking_lot` RwLock | `tokio::sync::RwLock` | `parking_lot::RwLock` is synchronous (no `.await` needed for lock acquisition). The in-memory store holds locks briefly for HashMap operations -- no async work inside the lock. Synchronous locks avoid the pitfall of holding async locks across yield points. The parent crate already uses this pattern. |
| DynamoDB client | `aws-sdk-dynamodb` 1.x | `rusoto` | `rusoto` is deprecated (archived 2023). `aws-sdk-dynamodb` is the official AWS SDK for Rust, actively maintained with weekly releases. |
| State machine crate | Hand-rolled enum + `can_transition_to()` | `rust-fsm`, `sm`, `stateflow` | The task state machine has exactly 5 states and ~8 transitions. A crate adds complexity for no benefit. The design doc already defines `TaskStatus::valid_transitions()` which is 15 lines of code. External state machine crates are for complex automata, not simple status enums. |
| Property testing | `proptest` | `quickcheck` | Both are in workspace dev-deps. `proptest` is more expressive (shrinking strategies, regex-based generators). Use `proptest` for new code. |
| Snapshot testing | `insta` | Manual assertion | `insta` with JSON feature gives exact serialization verification against spec JSON examples. One `assert_json_snapshot!` replaces 20 lines of manual field assertions. Already in workspace. |
| DynamoDB table library | Direct `aws-sdk-dynamodb` calls | `modyne` (single-table design crate) | `modyne` adds abstraction overhead for a simple two-entity schema (TASK + owner). The DynamoDB schema has one table, one GSI, and straightforward PK/SK patterns. Direct SDK calls with typed helpers are clearer and more maintainable. |

## Feature Flags

```toml
[features]
default = []
dynamodb = ["aws-sdk-dynamodb", "aws-config"]
```

**Why no default features:** The in-memory backend has zero extra dependencies. Users who only need dev/testing or single-instance deployment should not pull in the AWS SDK. The `dynamodb` feature adds ~40 transitive dependencies from the AWS SDK.

**Future features (not now):**
- `redis` -- Redis backend (not in scope for initial release)
- `notifications` -- SSE-based task status notifications (deferred)

## Cargo.toml for pmcp-tasks

```toml
[package]
name = "pmcp-tasks"
version = "0.1.0"
edition = "2021"
description = "MCP Tasks support for the PMCP SDK (experimental)"
license = "MIT"
rust-version = "1.82.0"

[features]
default = []
dynamodb = ["dep:aws-sdk-dynamodb", "dep:aws-config"]

[dependencies]
# Core (always included -- zero extra deps beyond pmcp)
pmcp = { path = "../..", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0", features = ["preserve_order"] }
async-trait = "0.1"
thiserror = "2.0"
uuid = { version = "1.17", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1", features = ["sync", "time"] }
tracing = "0.1"
parking_lot = "0.12"

# DynamoDB backend (optional)
aws-sdk-dynamodb = { version = "1", optional = true }
aws-config = { version = "1", optional = true }

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
proptest = "1.7"
insta = { version = "1.43", features = ["json", "redactions"] }
pretty_assertions = "1.4"
rstest = "0.26"
mockall = "0.14"
serde_json = { version = "1.0", features = ["preserve_order"] }
```

## Key Technical Decisions

### 1. async-trait is Required, Not Optional

The `TaskStore` trait uses `Arc<dyn TaskStore>` throughout:
- `TaskContext` holds `Arc<dyn TaskStore>`
- `TaskHandler` holds `Arc<dyn TaskStore>`
- `TaskMiddleware` holds `Arc<dyn TaskStore>`

Native `async fn in trait` makes traits non-dyn-safe. Until Rust stabilizes dyn async traits (no timeline), `async-trait` is the only production-ready solution. This matches the parent crate's approach.

**Confidence:** HIGH -- verified via Rust lang team blog posts and async fundamentals initiative docs.

### 2. parking_lot::RwLock for In-Memory Store (Not tokio::sync::RwLock)

The in-memory store wraps `HashMap<String, TaskRecord>` in a lock. Operations are pure CPU-bound HashMap lookups/inserts. Using `tokio::sync::RwLock` would:
- Require `.await` on every lock acquisition
- Risk holding the lock across yield points if misused
- Add no benefit since there is no async work inside the critical section

`parking_lot::RwLock` is the correct choice for protecting synchronous data structures accessed from async code, as long as the critical section is short (which it is -- HashMap operations are O(1)).

**Confidence:** HIGH -- this pattern is established in the parent crate and widely documented.

### 3. Semver Range for AWS SDK (version = "1")

The AWS SDK for Rust releases frequently (~weekly). Using `version = "1"` instead of pinning to e.g. `1.101.0` allows:
- Automatic security patches
- Bug fixes without Cargo.toml changes
- Compatibility with users who have different AWS SDK versions in their dependency tree

The 1.x series maintains backward compatibility per semver.

**Confidence:** HIGH -- AWS SDK follows semver; verified via crates.io release history.

### 4. DynamoDB TTL in Epoch Seconds (Not Milliseconds)

DynamoDB's native TTL feature requires a Number attribute in **Unix epoch seconds**. The MCP spec defines TTL in **milliseconds**. The DynamoDB backend must:
- Accept TTL in milliseconds from the MCP protocol
- Convert to epoch seconds for the `ttl_epoch` DynamoDB attribute: `created_at_epoch_secs + (ttl_ms / 1000)`
- Filter expired items on read (DynamoDB deletes within ~48 hours, not immediately)

**Confidence:** HIGH -- verified via AWS documentation.

### 5. ConditionalCheckFailedException for Atomic State Transitions

DynamoDB conditional writes enforce the state machine atomically:

```
UpdateExpression: SET #status = :new_status, #updated = :now
ConditionExpression: #status IN (:valid_from_states)
```

When a race condition occurs, DynamoDB returns `ConditionalCheckFailedException`. The store maps this to `TaskError::InvalidTransition` or retries with exponential backoff depending on the semantic.

Use `ReturnValuesOnConditionCheckFailure::ALL_OLD` for debugging support.

**Confidence:** HIGH -- verified via AWS docs and aws-sdk-rust issue tracker.

## What NOT to Use

| Technology | Why Not |
|------------|---------|
| `rusoto` | Deprecated/archived since 2023. Use `aws-sdk-dynamodb` instead. |
| `diesel` / `sea-orm` | These are SQL ORMs. DynamoDB is a key-value store with its own SDK. |
| `redis` crate | Out of scope for initial release. DynamoDB is the primary backend; in-memory for dev. |
| `rust-fsm` / `sm` / `stateflow` | Over-engineering for 5-state, 8-transition state machine. Hand-rolled enum is clearer. |
| `trait-variant` | Experimental/immature. `async-trait` is battle-tested with 500M+ downloads. |
| `modyne` (DynamoDB single-table) | Adds abstraction for a trivially simple schema. Direct SDK calls are more transparent. |
| `dashmap` | Not needed. The in-memory store uses `parking_lot::RwLock<HashMap>` which gives explicit read/write lock control needed for atomic status transitions. `DashMap` is great for concurrent key-value access but doesn't easily support "read-modify-write" atomicity needed for state machine transitions. |
| `serde_dynamo` | Convenience crate for DynamoDB serialization. Adds a dependency for something the AWS SDK's `AttributeValue` builders handle directly. Keep the dependency tree minimal. |

## Version Verification Summary

| Crate | Pinned Version | Latest Verified | Source | Notes |
|-------|---------------|----------------|--------|-------|
| `serde` | 1.0 | 1.0.228 | WebSearch/crates.io | Semver-compatible |
| `serde_json` | 1.0 | 1.0.149 | WebSearch/crates.io | Semver-compatible |
| `async-trait` | 0.1 | 0.1.83+ | WebSearch/crates.io | Stable API |
| `thiserror` | 2.0 | 2.0.18 | WebSearch/docs.rs | Breaking from 1.x, workspace already on 2.0 |
| `uuid` | 1.17 | ~1.21 | WebSearch/crates.io | Minimum 1.17 for features used in workspace |
| `chrono` | 0.4 | 0.4.43 | WebSearch/crates.io | Stable 0.4.x line |
| `tokio` | 1 | ~1.47 (LTS) | WebSearch/crates.io | LTS tracks available through Sep 2026 |
| `parking_lot` | 0.12 | 0.12.5 | WebSearch/crates.io | Note: MSRV 1.84 for latest, but 0.12.3 supports 1.82 |
| `tracing` | 0.1 | 0.1.x | WebSearch/crates.io | Stable API |
| `aws-sdk-dynamodb` | 1 | ~1.101+ | WebSearch/crates.io | Weekly releases |
| `aws-config` | 1 | ~1.8.13 | WebSearch/crates.io | Companion to AWS SDK |
| `proptest` | 1.7 | ~1.9 | WebSearch/crates.io | Note: MSRV 1.84 for 1.9, 1.7 for 1.82 |
| `insta` | 1.43 | ~1.46 | WebSearch/crates.io | Active development |
| `mockall` | 0.14 | 0.13.1 | WebSearch/crates.io | Workspace uses 0.14 (check compatibility) |

### MSRV Concern: parking_lot 0.12.5 and proptest 1.9

**parking_lot 0.12.5** raised its MSRV to 1.84. Since this workspace uses MSRV 1.82.0, use `parking_lot = ">=0.12.3, <0.12.5"` or let Cargo resolve to a compatible version within the 0.12.x range. Alternatively, the workspace may already have a lockfile pinning this. Verify during implementation.

**proptest 1.9** raised its MSRV to 1.84. Pin to `proptest = "1.7"` in dev-dependencies to maintain MSRV 1.82 compatibility.

**Confidence:** MEDIUM -- MSRV bumps in patch releases are unusual but documented. Verify with `cargo check` during implementation.

## Sources

- [crates.io: aws-sdk-dynamodb](https://crates.io/crates/aws-sdk-dynamodb) -- Version and release frequency
- [crates.io: aws-config](https://crates.io/crates/aws-config) -- Companion crate version
- [docs.rs: async-trait](https://docs.rs/async-trait) -- Dyn safety explanation
- [Rust Blog: Stabilizing async fn in traits](https://blog.rust-lang.org/inside-rust/2023/05/03/stabilizing-async-fn-in-trait.html) -- Native async traits NOT dyn-safe
- [Niko Matsakis: Dyn async traits](https://smallcultfollowing.com/babysteps/series/dyn-async-traits/) -- Future plans for dyn async
- [AWS: DynamoDB TTL](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/TTL.html) -- Epoch seconds requirement
- [AWS: DynamoDB Conditional Writes](https://aws.amazon.com/blogs/database/handle-conditional-write-errors-in-high-concurrency-scenarios-with-amazon-dynamodb/) -- ConditionalCheckFailedException handling
- [AWS: Error handling](https://docs.rs/aws-sdk-dynamodb/latest/aws_sdk_dynamodb/enum.Error.html) -- Rust SDK error types
- [GitHub: parking_lot](https://github.com/Amanieu/parking_lot) -- MSRV and performance claims
- [crates.io: proptest](https://crates.io/crates/proptest) -- Property testing
- [crates.io: insta](https://crates.io/crates/insta) -- Snapshot testing
- [Serde: Struct flattening](https://serde.rs/attr-flatten.html) -- Flatten + HashMap gotchas
- [modyne](https://github.com/neoeinstein/modyne) -- DynamoDB single-table design (considered, not recommended)
- [Alex DeBrie: Single-Table Design](https://www.alexdebrie.com/posts/dynamodb-single-table/) -- DynamoDB design patterns
