# Phase 11: DynamoDB Backend - Research

**Researched:** 2026-02-23
**Domain:** AWS DynamoDB StorageBackend implementation, single-table design, conditional writes, TTL, Rust aws-sdk-dynamodb crate
**Confidence:** HIGH

## Summary

Phase 11 implements `DynamoDbBackend`, a `StorageBackend` trait implementation that persists tasks in Amazon DynamoDB. The backend is feature-gated behind `dynamodb` and uses the official `aws-sdk-dynamodb` crate (v1.107.0) with `aws-config` for client construction. The implementation maps the 6 `StorageBackend` methods to DynamoDB operations: `GetItem`, `PutItem` (with `ConditionExpression` for CAS), `DeleteItem`, `Query` (for `list_by_prefix`), and a no-op `cleanup_expired` (DynamoDB's native TTL handles expiration automatically).

The table uses a single-table design with composite primary key: `PK = OWNER#<owner_id>` (partition key) and `SK = TASK#<task_id>` (sort key). This provides natural owner isolation -- `list_by_prefix` becomes a `Query` on `PK = :owner_pk` which returns only that owner's tasks. The `version` attribute (monotonic u64) enables CAS via `ConditionExpression: #version = :expected`. The `expires_at` attribute stores epoch seconds for DynamoDB's native TTL, which automatically deletes items within ~48 hours of expiry.

**Primary recommendation:** Implement `DynamoDbBackend` as a thin adapter mapping `StorageBackend` methods to DynamoDB API calls. Store the entire serialized `TaskRecord` JSON as a single `data` attribute (type `S`). Keep attributes minimal: `PK`, `SK`, `version`, `data`, `expires_at` (optional). The 350KB variable size cap is already enforced by `GenericTaskStore` via `StoreConfig::max_variable_size_bytes` -- the backend never sees oversized items.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Table design & key schema**: Owner-scoped partition key: `PK = OWNER#<owner_id>`, `SK = TASK#<task_id>` -- natural owner isolation, efficient list-by-owner queries. Single-table design. Human-readable attribute names: `owner_id`, `task_id`, `status`, `version`, `expires_at`, `data`. TTL attribute (`expires_at`) only set on tasks that have an explicit expiry configured -- no dummy far-future values for tasks without expiry. DynamoDB native TTL enabled on the `expires_at` attribute for automatic cleanup.
- **Configuration & construction**: Two construction paths: `DynamoDbBackend::new(client, table_name)` accepts a pre-built `aws_sdk_dynamodb::Client` for full flexibility; `from_env()` or similar auto-discovers configuration from the standard AWS SDK config chain. Default table name: `pmcp_tasks`, overridable via config. No special endpoint URL support in the backend -- developers configure DynamoDB Local/LocalStack via standard AWS SDK environment variables (`AWS_ENDPOINT_URL`) or by constructing the client themselves. Backend assumes table exists with correct schema; fails fast with clear error if table is missing or misconfigured.
- **Integration testing strategy**: Tests run against real DynamoDB in AWS (not DynamoDB Local). Gated behind `dynamodb-tests` feature flag: `cargo test --features dynamodb-tests`. Test isolation via shared table with unique owner prefix per test run (random UUID prefix, no table creation/deletion overhead). TTL testing: verify the `expires_at` attribute is correctly set on items, do not wait for DynamoDB's actual TTL deletion (up to 48 hours).
- **Error mapping & retries**: Use the AWS SDK's built-in retry with exponential backoff for transient failures (throttling, network). Phase 9's "no auto-retry" applies to our code layer, not the SDK's transport layer. DynamoDB `ConditionalCheckFailedException` maps to `StorageError::VersionConflict`. All unexpected DynamoDB errors map to generic `StorageError::Backend(Box<dyn Error>)` -- keep the error model backend-agnostic, no DynamoDB-specific variants. Size limit (350KB) enforced in GenericTaskStore via StoreConfig, not in DynamoDbBackend -- backend never sees oversized items.

### Claude's Discretion
- CAS error detail: whether to include actual_version by doing an extra read after ConditionalCheckFailedException, or report expected_version only
- Internal DynamoDB attribute mapping details (how TaskRecord fields map to DynamoDB attributes)
- Construction ergonomics (`from_env` naming, builder pattern, etc.)
- GSI design if needed for any access patterns beyond PK/SK queries

### Deferred Ideas (OUT OF SCOPE)
- `cargo pmcp tasks init --backend dynamodb` CLI command for table provisioning -- future phase
- DynamoDB Local support as a first-class testing option -- currently handled via AWS SDK config
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DYNA-01 | DynamoDbBackend implements StorageBackend behind `dynamodb` feature flag | Standard Stack section defines the feature flag setup with `aws-sdk-dynamodb` and `aws-config` as optional deps. Architecture Pattern 1 shows the full trait implementation mapping. |
| DYNA-02 | Single-table design with composite keys for task storage and owner isolation | Architecture Pattern 2 defines PK=`OWNER#<owner_id>`, SK=`TASK#<task_id>`. Query on PK provides owner-scoped listing. |
| DYNA-03 | ConditionExpression for atomic state transitions (concurrent mutation safety) | Architecture Pattern 3 shows the CAS pattern: `#version = :expected` condition on PutItem with version increment. ConditionalCheckFailedException maps to StorageError::VersionConflict. |
| DYNA-04 | Native DynamoDB TTL for automatic expired task cleanup | Architecture Pattern 4 covers TTL: `expires_at` stored as epoch seconds (Number type), cleanup_expired is a no-op. |
| DYNA-05 | Automatic variable size cap at ~350KB to stay within DynamoDB's 400KB item limit | Already enforced by GenericTaskStore via StoreConfig::max_variable_size_bytes. The planner sets this to 350KB for DynamoDB deployments. No backend-level enforcement needed. |
| DYNA-06 | Cloud-only integration tests against real DynamoDB table | Architecture Pattern 5 defines the `dynamodb-tests` feature flag, test isolation via random owner prefix, and TTL verification strategy. |
| TEST-02 | Per-backend integration tests for DynamoDbBackend against cloud DynamoDB | Same as DYNA-06 -- tests exercise all StorageBackend methods against real DynamoDB. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| aws-sdk-dynamodb | 1.107.0 | DynamoDB API client | Official AWS SDK for Rust, actively maintained |
| aws-config | 1.1.7+ | AWS SDK config loading | Standard config chain (env vars, profiles, IMDS) |
| tokio | 1.x | Async runtime | Already in workspace, required by AWS SDK |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| async-trait | 0.1 | Async trait for StorageBackend | Already in deps |
| uuid | 1.17 | Test isolation (random owner prefixes) | Already in deps |
| chrono | 0.4 | DateTime to epoch seconds conversion for TTL | Already in deps |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| aws-sdk-dynamodb | rusoto_dynamodb | rusoto is deprecated, aws-sdk-dynamodb is the official replacement |
| Single `data` attribute (String) | Separate attributes per TaskRecord field | Single blob is simpler, avoids mapping complexity, matches the "dumb KV store" philosophy; separate attributes would only help DynamoDB-native queries which we don't need (domain logic is in GenericTaskStore) |
| PK/SK composite keys | Single PK with task_id | Composite keys enable efficient Query for owner-scoped listing without a GSI |

**Installation (in crates/pmcp-tasks/Cargo.toml):**
```toml
[dependencies]
aws-sdk-dynamodb = { version = "1.107.0", optional = true }
aws-config = { version = "1.1.7", features = ["behavior-version-latest"], optional = true }

[features]
dynamodb = ["dep:aws-sdk-dynamodb", "dep:aws-config"]
dynamodb-tests = ["dynamodb"]

[dev-dependencies]
# aws-sdk-dynamodb and aws-config are pulled in via dynamodb-tests feature
```

## Architecture Patterns

### Recommended Module Structure
```
crates/pmcp-tasks/src/
|-- store/
|   |-- mod.rs              # existing: StoreConfig, TaskStore, blanket impls
|   |-- backend.rs           # existing: StorageBackend trait
|   |-- generic.rs           # existing: GenericTaskStore<B>
|   |-- memory.rs            # existing: InMemoryBackend, InMemoryTaskStore
|   +-- dynamodb.rs          # NEW: DynamoDbBackend (behind #[cfg(feature = "dynamodb")])
+-- lib.rs                   # conditional module + re-exports
```

### Pattern 1: DynamoDbBackend Struct and Construction

**What:** A struct holding a pre-built `aws_sdk_dynamodb::Client` and table name, implementing `StorageBackend`.

**Design:**
```rust
#[cfg(feature = "dynamodb")]
pub mod dynamodb;

// In dynamodb.rs:
use aws_sdk_dynamodb::Client;
use aws_sdk_dynamodb::types::AttributeValue;
use crate::store::backend::{StorageBackend, StorageError, VersionedRecord};

/// DynamoDB storage backend for task persistence.
///
/// Stores task records in a single DynamoDB table using composite
/// primary keys: `PK = OWNER#<owner_id>`, `SK = TASK#<task_id>`.
/// All domain logic lives in `GenericTaskStore`; this backend is
/// a dumb KV adapter.
///
/// # Construction
///
/// ```rust,no_run
/// use aws_sdk_dynamodb::Client;
/// // Pre-built client (full control):
/// let backend = DynamoDbBackend::new(client, "my_tasks_table");
///
/// // From environment (standard AWS config chain):
/// let backend = DynamoDbBackend::from_env().await;
/// ```
#[derive(Debug, Clone)]
pub struct DynamoDbBackend {
    client: Client,
    table_name: String,
}

impl DynamoDbBackend {
    /// Creates a backend with a pre-built DynamoDB client.
    ///
    /// The table must already exist with the correct schema:
    /// - PK (String, partition key)
    /// - SK (String, sort key)
    /// - version (Number)
    /// - data (String)
    /// - expires_at (Number, optional, TTL attribute)
    pub fn new(client: Client, table_name: impl Into<String>) -> Self {
        Self {
            client,
            table_name: table_name.into(),
        }
    }

    /// Creates a backend using the standard AWS SDK config chain.
    ///
    /// Loads credentials/region from environment variables, AWS profiles,
    /// or IMDS (for EC2/Lambda). Uses `pmcp_tasks` as the default table name.
    pub async fn from_env() -> Self {
        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);
        Self::new(client, "pmcp_tasks")
    }

    /// Creates a backend from env with a custom table name.
    pub async fn from_env_with_table(table_name: impl Into<String>) -> Self {
        let config = aws_config::load_from_env().await;
        let client = Client::new(&config);
        Self::new(client, table_name)
    }
}
```

### Pattern 2: DynamoDB Key Schema (Single-Table Design)

**What:** The composite key structure for owner-isolated task storage.

**Table Schema:**
```
Table: pmcp_tasks
  PK (String)  - Partition key - Format: OWNER#<owner_id>
  SK (String)  - Sort key      - Format: TASK#<task_id>

Attributes per item:
  PK         (S) - OWNER#session-abc
  SK         (S) - TASK#550e8400-e29b-41d4-a716-446655440000
  version    (N) - 3
  data       (S) - {"task":{...},"ownerId":"session-abc",...}
  expires_at (N) - 1709251200  (epoch seconds, ONLY present if task has TTL)
```

**Key construction helpers:**
```rust
/// Constructs the DynamoDB partition key from an owner_id.
fn make_pk(owner_id: &str) -> String {
    format!("OWNER#{owner_id}")
}

/// Constructs the DynamoDB sort key from a task_id.
fn make_sk(task_id: &str) -> String {
    format!("TASK#{task_id}")
}

/// Extracts owner_id from a PK value.
fn parse_pk(pk: &str) -> Option<&str> {
    pk.strip_prefix("OWNER#")
}

/// Extracts task_id from an SK value.
fn parse_sk(sk: &str) -> Option<&str> {
    sk.strip_prefix("TASK#")
}
```

**Why this design:**
- PK scoping: All items for one owner share a partition key, making Query efficient (single partition read).
- SK ordering: Tasks within an owner can be sorted naturally. `begins_with(SK, "TASK#")` in KeyConditionExpression scopes to tasks.
- Owner isolation: A Query on `PK = OWNER#owner-a` physically cannot return items belonging to `OWNER#owner-b`.
- No GSI needed for core CRUD: Query on PK + SK covers get, list-by-owner, and delete. A GSI would only be needed for cross-owner queries (not in scope).

**Mapping StorageBackend composite keys to DynamoDB keys:**

The `StorageBackend` trait uses composite string keys in the format `{owner_id}:{task_id}`. The DynamoDB backend splits this into PK/SK:
```rust
fn split_key(key: &str) -> Result<(String, String), StorageError> {
    let (owner_id, task_id) = key.split_once(':')
        .ok_or_else(|| StorageError::Backend {
            message: format!("invalid key format: {key}"),
            source: None,
        })?;
    Ok((make_pk(owner_id), make_sk(task_id)))
}

fn split_prefix(prefix: &str) -> Result<String, StorageError> {
    // Prefix format is "{owner_id}:" -- extract owner_id, build PK
    let owner_id = prefix.strip_suffix(':')
        .ok_or_else(|| StorageError::Backend {
            message: format!("invalid prefix format: {prefix}"),
            source: None,
        })?;
    Ok(make_pk(owner_id))
}
```

### Pattern 3: CAS via ConditionExpression

**What:** Atomic compare-and-set using DynamoDB's `ConditionExpression` on the `version` attribute.

**Implementation for `put_if_version`:**
```rust
async fn put_if_version(
    &self,
    key: &str,
    data: &[u8],
    expected_version: u64,
) -> Result<u64, StorageError> {
    let (pk, sk) = split_key(key)?;
    let new_version = expected_version + 1;
    let data_str = String::from_utf8(data.to_vec())
        .map_err(|e| StorageError::Backend {
            message: format!("data is not valid UTF-8: {e}"),
            source: Some(Box::new(e)),
        })?;

    let result = self.client
        .put_item()
        .table_name(&self.table_name)
        .item("PK", AttributeValue::S(pk))
        .item("SK", AttributeValue::S(sk))
        .item("version", AttributeValue::N(new_version.to_string()))
        .item("data", AttributeValue::S(data_str))
        // Conditionally set expires_at if present in the TaskRecord
        // (handled by a helper method)
        .condition_expression("#v = :expected")
        .expression_attribute_names("#v", "version")
        .expression_attribute_values(
            ":expected",
            AttributeValue::N(expected_version.to_string()),
        )
        .send()
        .await;

    match result {
        Ok(_) => Ok(new_version),
        Err(sdk_err) => {
            if let Some(service_err) = sdk_err.as_service_error() {
                if service_err.is_conditional_check_failed_exception() {
                    // CAS failed -- version mismatch
                    return Err(StorageError::VersionConflict {
                        key: key.to_string(),
                        expected: expected_version,
                        actual: expected_version, // see Discretion note below
                    });
                }
            }
            Err(map_sdk_error(sdk_err, key))
        }
    }
}
```

**Discretion: CAS error detail.**
When `ConditionalCheckFailedException` fires, we know the expected version but NOT the actual version without an extra read. Recommendation: report `actual = expected` (since we don't know the real value) and let `GenericTaskStore` surface this as `TaskError::ConcurrentModification` with the expected version. An extra GetItem to discover the actual version adds latency and another potential race. The caller already knows to retry -- they don't need the actual version number.

### Pattern 4: TTL via `expires_at` Epoch Seconds

**What:** DynamoDB's native TTL expects a Number attribute containing epoch seconds. Items with `expires_at` in the past are automatically deleted by DynamoDB (within ~48 hours).

**Implementation:**
```rust
/// Converts a TaskRecord's expires_at DateTime<Utc> to epoch seconds for DynamoDB TTL.
///
/// Returns None if the record has no expiry, in which case the attribute
/// is omitted from the DynamoDB item (per locked decision: no dummy values).
fn extract_ttl_epoch(data: &[u8]) -> Option<i64> {
    // Parse the serialized JSON to extract expiresAt
    let value: serde_json::Value = serde_json::from_slice(data).ok()?;
    let expires_at_str = value.get("expiresAt")?.as_str()?;
    let dt = chrono::DateTime::parse_from_rfc3339(expires_at_str).ok()?;
    Some(dt.timestamp())
}
```

**In put/put_if_version:**
```rust
// Conditionally add expires_at attribute
if let Some(epoch) = extract_ttl_epoch(data) {
    builder = builder.item("expires_at", AttributeValue::N(epoch.to_string()));
}
```

**cleanup_expired is a no-op:**
```rust
async fn cleanup_expired(&self) -> Result<usize, StorageError> {
    // DynamoDB's native TTL handles cleanup automatically.
    // No-op: return 0 records removed.
    Ok(0)
}
```

### Pattern 5: Integration Test Strategy

**What:** Tests run against real DynamoDB, gated behind `dynamodb-tests` feature flag, with test isolation via unique owner prefixes.

```rust
#[cfg(all(test, feature = "dynamodb-tests"))]
mod integration_tests {
    use super::*;
    use uuid::Uuid;

    /// Creates a test backend pointing at the shared DynamoDB table.
    /// Each test run uses a unique owner prefix for isolation.
    async fn test_backend() -> (DynamoDbBackend, String) {
        let backend = DynamoDbBackend::from_env().await;
        let test_prefix = format!("test-{}", Uuid::new_v4());
        (backend, test_prefix)
    }

    #[tokio::test]
    async fn put_and_get_round_trip() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        let data = b"test data";
        let version = backend.put(&key, data).await.unwrap();
        assert_eq!(version, 1);

        let record = backend.get(&key).await.unwrap();
        assert_eq!(record.data, data);
        assert_eq!(record.version, 1);
    }

    // ... tests for all 6 StorageBackend methods
}
```

**Test table setup:** Tests assume a table named `pmcp_tasks` exists with the correct schema. CI/CD must provision this table before running `cargo test --features dynamodb-tests`. The table schema is documented in the code and in this research for the planner to reference.

### Pattern 6: Error Mapping

**What:** Map AWS SDK errors to `StorageError` variants.

```rust
fn map_sdk_error<E: std::fmt::Display + std::error::Error + Send + Sync + 'static>(
    err: aws_smithy_runtime_api::client::result::SdkError<E>,
    key: &str,
) -> StorageError {
    // Check for ResourceNotFoundException (table doesn't exist)
    let message = format!("DynamoDB error for key {key}: {err}");
    StorageError::Backend {
        message,
        source: Some(Box::new(err)),
    }
}
```

**Key error mappings:**
| DynamoDB Error | StorageError | Notes |
|----------------|-------------|-------|
| ConditionalCheckFailedException | VersionConflict | CAS failure on put_if_version |
| ResourceNotFoundException | Backend (table missing) | Fail fast with clear message |
| ProvisionedThroughputExceededException | Backend (will be retried by SDK) | SDK auto-retries; only surfaces if all retries exhausted |
| ItemCollectionSizeLimitExceededException | CapacityExceeded | Unlikely but mapped |
| All other errors | Backend { message, source } | Preserve error chain |

### Anti-Patterns to Avoid

- **Storing version in the `data` blob:** Version is a separate DynamoDB attribute used in `ConditionExpression`. It MUST NOT be inside the serialized JSON blob (where it's `#[serde(skip)]`).
- **Scanning the entire table for list_by_prefix:** Use `Query` with `KeyConditionExpression`, NEVER `Scan`. Scan reads every item in the table.
- **Creating/deleting tables in tests:** Use shared table with unique owner prefixes per test run. Table creation is slow and costs money.
- **Setting `expires_at` to a far-future value for tasks without TTL:** Omit the attribute entirely (per locked decision). DynamoDB only deletes items where `expires_at` is in the past.
- **Implementing domain logic in the backend:** State machine validation, owner checking, variable merge -- all in GenericTaskStore. Backend is a dumb KV adapter.
- **Using Scan for cleanup_expired:** DynamoDB's native TTL handles this. cleanup_expired is a no-op.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AWS credential loading | Custom credential parsing | `aws-config::load_from_env()` | Handles env vars, profiles, IMDS, ECS task role, Lambda role |
| Retry with backoff | Custom retry loop | AWS SDK's built-in retry | SDK already retries throttling, 500s, network errors with exponential backoff |
| CAS primitive | Two-phase read-then-write | DynamoDB `ConditionExpression` | Atomic server-side condition check, no race window |
| TTL cleanup | Background scan + delete | DynamoDB native TTL | Free, no consumed write capacity, automatic |
| Item serialization to DynamoDB types | Custom `AttributeValue` mapping | Single `data` attribute (String) holding JSON | The entire TaskRecord serialized by GenericTaskStore goes into one String attribute; no per-field mapping needed |
| Size limit enforcement | Backend-level size checking | `GenericTaskStore` via `StoreConfig::max_variable_size_bytes` | Already enforced before data reaches the backend |

**Key insight:** The DynamoDB backend should be ~150-200 lines of code. It is a thin adapter: split key -> call DynamoDB API -> map errors. All intelligence is in `GenericTaskStore`.

## Common Pitfalls

### Pitfall 1: Using Scan Instead of Query
**What goes wrong:** `list_by_prefix` implemented as a `Scan` with `FilterExpression` reads every item in the table, consuming massive read capacity.
**Why it happens:** Confusion between `Query` (uses primary key index) and `Scan` (full table read).
**How to avoid:** Always use `Query` with `KeyConditionExpression: PK = :pk_val`. This hits the partition key index directly.
**Warning signs:** Excessive read capacity consumption, slow list operations, DynamoDB throttling.

### Pitfall 2: Number Attribute Formatting
**What goes wrong:** `AttributeValue::N` takes a String (not an i64/u64). Passing `"3.0"` vs `"3"` can cause condition expression mismatches.
**Why it happens:** DynamoDB's wire protocol represents numbers as strings. The Rust SDK's `AttributeValue::N` constructor takes `impl Into<String>`.
**How to avoid:** Always use `version.to_string()` and `epoch.to_string()`. Never use floating-point formatting for integer values. Parse with `str::parse::<u64>()` on read.
**Warning signs:** ConditionExpression failures that shouldn't fail; version parsing errors.

### Pitfall 3: Missing TTL Attribute on Tasks Without Expiry
**What goes wrong:** Setting `expires_at` to 0 or a far-future value causes DynamoDB TTL to either immediately delete the item (0 = epoch 1970, which is in the past) or waste TTL evaluation.
**Why it happens:** Assumption that every item needs the TTL attribute.
**How to avoid:** Only set `expires_at` when the task has an explicit TTL. Omit the attribute entirely for tasks without expiry (per locked decision).
**Warning signs:** Tasks disappearing unexpectedly; TTL deleting items that shouldn't expire.

### Pitfall 4: ConditionalCheckFailedException Without Version Info
**What goes wrong:** On CAS failure, the `ConditionalCheckFailedException` does not contain the actual current version. If the code assumes `actual_version` is available, it will panic or produce incorrect error messages.
**Why it happens:** DynamoDB only tells you the condition failed, not what the actual values were (unless you use `ReturnValuesOnConditionCheckFailure`).
**How to avoid:** Use `ReturnValuesOnConditionCheckFailure::ALL_OLD` on put_if_version calls to get the previous item, or accept that `actual_version` in `StorageError::VersionConflict` will be approximate. Recommendation: report `actual = expected` and let the caller retry.
**Warning signs:** Incorrect `actual_version` values in error messages.

### Pitfall 5: Forgetting to Increment Version on Unconditional Put
**What goes wrong:** The `put` method (unconditional) must read the current version, increment, and write. Without reading first, you lose the version chain.
**Why it happens:** DynamoDB `PutItem` without conditions overwrites the entire item, so you need to know the current version to increment.
**How to avoid:** Implement `put` as: GetItem -> if exists, version+1; else, version=1 -> PutItem. Use `attribute_not_exists(PK)` condition for new items to detect races.
**Warning signs:** Version always stuck at 1; CAS operations failing because versions don't increment.

### Pitfall 6: Data Attribute Type Mismatch
**What goes wrong:** Storing the serialized JSON as `AttributeValue::B` (Binary) instead of `AttributeValue::S` (String) makes the data unreadable in the DynamoDB console.
**Why it happens:** JSON bytes are technically binary data. But the locked decision says "human-readable attribute names" and "debuggable via DynamoDB console".
**How to avoid:** Store the serialized JSON as `AttributeValue::S` (String type). The data is valid UTF-8 JSON, so String is the correct DynamoDB type.
**Warning signs:** DynamoDB console shows base64-encoded blobs instead of readable JSON.

## Code Examples

### Example 1: Get Operation
```rust
// Source: StorageBackend::get -> DynamoDB GetItem
async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
    let (pk, sk) = split_key(key)?;

    let result = self.client
        .get_item()
        .table_name(&self.table_name)
        .key("PK", AttributeValue::S(pk))
        .key("SK", AttributeValue::S(sk))
        .send()
        .await
        .map_err(|e| map_sdk_error(e, key))?;

    let item = result.item()
        .ok_or_else(|| StorageError::NotFound { key: key.to_string() })?;

    let version = item.get("version")
        .and_then(|v| v.as_n().ok())
        .and_then(|n| n.parse::<u64>().ok())
        .ok_or_else(|| StorageError::Backend {
            message: format!("missing or invalid version attribute for key {key}"),
            source: None,
        })?;

    let data = item.get("data")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| StorageError::Backend {
            message: format!("missing or invalid data attribute for key {key}"),
            source: None,
        })?;

    Ok(VersionedRecord {
        data: data.as_bytes().to_vec(),
        version,
    })
}
```

### Example 2: Unconditional Put
```rust
// Source: StorageBackend::put -> DynamoDB GetItem + PutItem
async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
    let (pk, sk) = split_key(key)?;
    let data_str = std::str::from_utf8(data)
        .map_err(|e| StorageError::Backend {
            message: format!("data is not valid UTF-8: {e}"),
            source: Some(Box::new(e)),
        })?;

    // Read current version (if exists)
    let current_version = match self.get(key).await {
        Ok(record) => record.version,
        Err(StorageError::NotFound { .. }) => 0,
        Err(e) => return Err(e),
    };
    let new_version = current_version + 1;

    let mut builder = self.client
        .put_item()
        .table_name(&self.table_name)
        .item("PK", AttributeValue::S(pk))
        .item("SK", AttributeValue::S(sk))
        .item("version", AttributeValue::N(new_version.to_string()))
        .item("data", AttributeValue::S(data_str.to_string()));

    if let Some(epoch) = extract_ttl_epoch(data) {
        builder = builder.item("expires_at", AttributeValue::N(epoch.to_string()));
    }

    builder.send().await.map_err(|e| map_sdk_error(e, key))?;
    Ok(new_version)
}
```

### Example 3: List By Prefix (Query)
```rust
// Source: StorageBackend::list_by_prefix -> DynamoDB Query
async fn list_by_prefix(
    &self,
    prefix: &str,
) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
    let pk = split_prefix(prefix)?;

    let mut results = Vec::new();
    let mut exclusive_start_key = None;

    loop {
        let mut query = self.client
            .query()
            .table_name(&self.table_name)
            .key_condition_expression("PK = :pk")
            .expression_attribute_values(":pk", AttributeValue::S(pk.clone()));

        if let Some(start_key) = exclusive_start_key.take() {
            query = query.set_exclusive_start_key(Some(start_key));
        }

        let output = query.send().await
            .map_err(|e| map_sdk_error(e, prefix))?;

        if let Some(items) = output.items() {
            for item in items {
                // Reconstruct the composite key from PK/SK
                let item_pk = item.get("PK").and_then(|v| v.as_s().ok());
                let item_sk = item.get("SK").and_then(|v| v.as_s().ok());
                let version_str = item.get("version").and_then(|v| v.as_n().ok());
                let data_str = item.get("data").and_then(|v| v.as_s().ok());

                if let (Some(pk_val), Some(sk_val), Some(ver), Some(data)) =
                    (item_pk, item_sk, version_str, data_str)
                {
                    if let (Some(owner), Some(task_id)) = (parse_pk(pk_val), parse_sk(sk_val)) {
                        let composite_key = format!("{owner}:{task_id}");
                        let version: u64 = ver.parse().unwrap_or(0);
                        results.push((
                            composite_key,
                            VersionedRecord {
                                data: data.as_bytes().to_vec(),
                                version,
                            },
                        ));
                    }
                }
            }
        }

        // Handle pagination
        match output.last_evaluated_key() {
            Some(last_key) if !last_key.is_empty() => {
                exclusive_start_key = Some(last_key.clone());
            }
            _ => break,
        }
    }

    Ok(results)
}
```

### Example 4: Feature Flag Setup in Cargo.toml
```toml
# In crates/pmcp-tasks/Cargo.toml
[dependencies]
aws-sdk-dynamodb = { version = "1.107.0", optional = true }
aws-config = { version = "1.1.7", features = ["behavior-version-latest"], optional = true }

[features]
dynamodb = ["dep:aws-sdk-dynamodb", "dep:aws-config"]
dynamodb-tests = ["dynamodb"]
```

```rust
// In lib.rs -- conditional module inclusion
#[cfg(feature = "dynamodb")]
pub use store::dynamodb::DynamoDbBackend;

// In store/mod.rs -- conditional module
#[cfg(feature = "dynamodb")]
pub mod dynamodb;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| rusoto (deprecated) | aws-sdk-dynamodb | 2023 (GA) | Official AWS SDK for Rust, actively maintained |
| Expected (legacy conditional) | ConditionExpression | 2017+ | Modern expression syntax, more powerful |
| Manual TTL scanning | DynamoDB native TTL | 2017 | Free automatic cleanup, no write capacity consumed |
| Multiple tables per entity | Single-table design | Established pattern | Better performance, lower cost, simpler IAM |
| Custom retry logic | AWS SDK built-in retry | Default in SDK | Exponential backoff with jitter, handles throttling |

**Deprecated/outdated:**
- `rusoto` crate: Deprecated. Use `aws-sdk-dynamodb` instead.
- `Expected` parameter in PutItem/UpdateItem: Legacy. Use `ConditionExpression` instead.
- `AttributeUpdates` in UpdateItem: Legacy. Use `UpdateExpression` instead.

## Open Questions

1. **Unconditional `put` atomicity**
   - What we know: `put` is called by GenericTaskStore for `create` (new tasks only). It needs to assign version 1 for new keys or increment for existing keys.
   - What's unclear: Whether to use GetItem + PutItem (two API calls) or PutItem with `attribute_not_exists(PK)` + separate UpdateItem for existing keys.
   - Recommendation: For `create`, GenericTaskStore always calls `put` for new keys. Use PutItem with `attribute_not_exists(PK)` condition to ensure version=1 for new items. If the condition fails (item already exists), do a GetItem to get current version, then PutItem with new version. This handles the rare race condition. Alternatively, since `put` is unconditional by contract, simply do GetItem + PutItem -- the unconditional nature means overwriting is acceptable.

2. **SdkError type parameter complexity**
   - What we know: The AWS SDK returns `SdkError<PutItemError>`, `SdkError<GetItemError>`, etc. Each operation has its own error type.
   - What's unclear: The exact type signatures for the generic error mapping function.
   - Recommendation: Write per-operation error mapping functions (or a macro) rather than trying to be fully generic. The `as_service_error()` method on `SdkError` provides access to the operation-specific error.

3. **Table validation on construction**
   - What we know: Locked decision says "fails fast with clear error if table is missing or misconfigured."
   - What's unclear: Whether to validate the table schema on construction (DescribeTable) or lazily on first operation.
   - Recommendation: Do NOT validate on construction. Let the first operation fail with `ResourceNotFoundException` and map it to a clear `StorageError::Backend` message. This avoids an extra API call and works better in Lambda cold starts.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `crates/pmcp-tasks/src/store/backend.rs` (StorageBackend trait, 521 lines)
- Existing codebase: `crates/pmcp-tasks/src/store/generic.rs` (GenericTaskStore, 1270 lines)
- Existing codebase: `crates/pmcp-tasks/src/store/memory.rs` (InMemoryBackend reference implementation)
- Existing codebase: `crates/pmcp-tasks/src/domain/record.rs` (TaskRecord serialization)
- Phase 9 research: `.planning/phases/09-storage-abstraction-layer/09-RESEARCH.md`
- Phase 11 context: `.planning/phases/11-dynamodb-backend/11-CONTEXT.md`
- [aws-sdk-dynamodb docs.rs](https://docs.rs/aws-sdk-dynamodb/latest/aws_sdk_dynamodb/) - Client API, PutItemError enum, operation builders
- [DynamoDB PutItemError::ConditionalCheckFailedException](https://docs.rs/aws-sdk-dynamodb/latest/aws_sdk_dynamodb/operation/put_item/enum.PutItemError.html) - 11 variants including `is_conditional_check_failed_exception()` helper method
- [AWS DynamoDB Developer Guide - ConditionExpression](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Expressions.ConditionExpressions.html)
- [AWS DynamoDB Developer Guide - TTL](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/TTL.html) - Epoch seconds format, automatic deletion

### Secondary (MEDIUM confidence)
- [AWS DynamoDB & Rust Cheat Sheet (Dynobase)](https://dynobase.dev/dynamodb-rust/) - Cargo dependencies, AttributeValue usage, PutItem/Query examples
- [StratusGrid - Query DynamoDB with Rust](https://stratusgrid.com/blog/how-to-query-data-from-aws-dynamodb-tables-with-rust) - Query pagination pattern with `into_paginator()`
- [aws-sdk-rust GitHub Issue #847](https://github.com/awslabs/aws-sdk-rust/issues/847) - ConditionalCheckFailedException handling pattern with `as_service_error()`
- [AWS DynamoDB Developer Guide - Key Condition Expressions](https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/Query.KeyConditionExpressions.html) - begins_with, sort key operators

### Tertiary (LOW confidence)
- None -- all findings verified against official AWS documentation and SDK docs.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- aws-sdk-dynamodb is the official SDK, version verified on crates.io and docs.rs
- Architecture: HIGH -- patterns derived from existing StorageBackend trait contract and DynamoDB best practices documented by AWS
- Pitfalls: HIGH -- common DynamoDB gotchas (Scan vs Query, Number formatting, TTL epoch) verified against AWS documentation
- Error handling: MEDIUM -- exact error matching pattern verified via docs.rs PutItemError page, but full end-to-end pattern with `SdkError` wrapping needs validation during implementation

**Research date:** 2026-02-23
**Valid until:** 2026-03-23 (stable domain; AWS SDK versions may advance but APIs are backward-compatible)
