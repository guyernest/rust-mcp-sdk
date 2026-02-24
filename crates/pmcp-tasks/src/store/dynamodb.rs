//! DynamoDB storage backend for task persistence.
//!
//! [`DynamoDbBackend`] implements [`StorageBackend`] using Amazon DynamoDB as
//! the underlying key-value store. It maps the 6 trait methods to DynamoDB API
//! calls: `GetItem`, `PutItem` (with `ConditionExpression` for CAS),
//! `DeleteItem`, and `Query` (for `list_by_prefix`).
//!
//! # Single-Table Design
//!
//! All task records live in a single DynamoDB table using composite primary
//! keys:
//!
//! | Attribute    | Type   | Description                           |
//! |-------------|--------|---------------------------------------|
//! | `PK`        | String | Partition key: `OWNER#<owner_id>`     |
//! | `SK`        | String | Sort key: `TASK#<task_id>`            |
//! | `version`   | Number | Monotonic CAS version (starts at 1)   |
//! | `data`      | String | Serialized `TaskRecord` JSON          |
//! | `expires_at`| Number | Epoch seconds for DynamoDB TTL (optional) |
//!
//! Owner isolation is structural: a `Query` on `PK = OWNER#owner-a` physically
//! cannot return items belonging to `OWNER#owner-b`.
//!
//! # Relationship to GenericTaskStore
//!
//! This backend is a **dumb KV adapter**. It stores and retrieves opaque
//! byte blobs (serialized JSON). All domain logic -- state machine validation,
//! owner checking, variable merge, TTL policy -- lives in
//! [`GenericTaskStore`](crate::store::generic::GenericTaskStore). The backend
//! never interprets the data it stores, except for extracting the `expiresAt`
//! field to set the DynamoDB TTL attribute.
//!
//! # Size Limits
//!
//! DynamoDB has a 400 KB item size limit. The `GenericTaskStore` enforces a
//! configurable `StoreConfig::max_variable_size_bytes` cap (default 1 MB,
//! recommended 350 KB for DynamoDB) **before** data reaches this backend.
//! The backend itself does not enforce size limits.
//!
//! # Usage
//!
//! ```rust,no_run
//! use pmcp_tasks::store::dynamodb::DynamoDbBackend;
//! use pmcp_tasks::store::generic::GenericTaskStore;
//!
//! # async fn example() {
//! // From environment (standard AWS config chain):
//! let backend = DynamoDbBackend::from_env().await;
//! let store = GenericTaskStore::new(backend);
//!
//! // With pre-built client:
//! let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
//! let client = aws_sdk_dynamodb::Client::new(&config);
//! let backend = DynamoDbBackend::new(client, "my_tasks_table");
//! let store = GenericTaskStore::new(backend);
//! # }
//! ```

use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;

use crate::store::backend::{StorageBackend, StorageError, VersionedRecord};

/// DynamoDB storage backend for task persistence.
///
/// Stores task records in a single DynamoDB table using composite primary
/// keys: `PK = OWNER#<owner_id>`, `SK = TASK#<task_id>`. The `version`
/// attribute enables CAS via `ConditionExpression`. The optional `expires_at`
/// attribute stores epoch seconds for DynamoDB's native TTL.
///
/// This backend is a thin adapter -- it contains **no domain logic**. All
/// intelligence (state machine validation, owner isolation, variable merge,
/// TTL enforcement) lives in
/// [`GenericTaskStore`](crate::store::generic::GenericTaskStore).
///
/// # Size Limits
///
/// DynamoDB has a 400 KB item size limit. The `GenericTaskStore` enforces a
/// configurable variable size cap via `StoreConfig::max_variable_size_bytes`
/// before data reaches this backend, so oversized items never arrive here.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp_tasks::store::dynamodb::DynamoDbBackend;
/// use pmcp_tasks::store::generic::GenericTaskStore;
///
/// # async fn example() {
/// let backend = DynamoDbBackend::from_env().await;
/// let store = GenericTaskStore::new(backend);
/// # }
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
    /// - `PK` (String, partition key)
    /// - `SK` (String, sort key)
    /// - `version` (Number)
    /// - `data` (String)
    /// - `expires_at` (Number, optional, TTL attribute)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp_tasks::store::dynamodb::DynamoDbBackend;
    ///
    /// # async fn example() {
    /// let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    /// let client = aws_sdk_dynamodb::Client::new(&config);
    /// let backend = DynamoDbBackend::new(client, "my_tasks_table");
    /// # }
    /// ```
    pub fn new(client: Client, table_name: impl Into<String>) -> Self {
        Self {
            client,
            table_name: table_name.into(),
        }
    }

    /// Creates a backend using the standard AWS SDK config chain.
    ///
    /// Loads credentials and region from environment variables, AWS profiles,
    /// or IMDS (for EC2/Lambda). Uses `"pmcp_tasks"` as the default table
    /// name.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp_tasks::store::dynamodb::DynamoDbBackend;
    ///
    /// # async fn example() {
    /// let backend = DynamoDbBackend::from_env().await;
    /// # }
    /// ```
    pub async fn from_env() -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Self::new(client, "pmcp_tasks")
    }

    /// Creates a backend from the standard AWS SDK config chain with a
    /// custom table name.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp_tasks::store::dynamodb::DynamoDbBackend;
    ///
    /// # async fn example() {
    /// let backend = DynamoDbBackend::from_env_with_table("my_tasks").await;
    /// # }
    /// ```
    pub async fn from_env_with_table(table_name: impl Into<String>) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Self::new(client, table_name)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Constructs the DynamoDB partition key from an owner_id.
fn make_pk(owner_id: &str) -> String {
    format!("OWNER#{owner_id}")
}

/// Constructs the DynamoDB sort key from a task_id.
fn make_sk(task_id: &str) -> String {
    format!("TASK#{task_id}")
}

/// Extracts the owner_id from a DynamoDB partition key value.
fn parse_pk(pk: &str) -> Option<&str> {
    pk.strip_prefix("OWNER#")
}

/// Extracts the task_id from a DynamoDB sort key value.
fn parse_sk(sk: &str) -> Option<&str> {
    sk.strip_prefix("TASK#")
}

/// Splits a composite `{owner_id}:{task_id}` key into `(PK, SK)` values
/// suitable for DynamoDB operations.
fn split_key(key: &str) -> Result<(String, String), StorageError> {
    let (owner_id, task_id) = key.split_once(':').ok_or_else(|| StorageError::Backend {
        message: format!("invalid key format (missing ':'): {key}"),
        source: None,
    })?;
    Ok((make_pk(owner_id), make_sk(task_id)))
}

/// Splits a composite prefix `{owner_id}:` into a PK value suitable for
/// DynamoDB `Query` operations. The trailing colon is stripped.
fn split_prefix(prefix: &str) -> Result<String, StorageError> {
    let owner_id = prefix
        .strip_suffix(':')
        .ok_or_else(|| StorageError::Backend {
            message: format!("invalid prefix format (missing trailing ':'): {prefix}"),
            source: None,
        })?;
    Ok(make_pk(owner_id))
}

/// Extracts the TTL epoch seconds from serialized task record JSON.
///
/// Parses the `expiresAt` field (an RFC 3339 datetime string) from the
/// serialized `TaskRecord` JSON and converts it to Unix epoch seconds.
/// Returns `None` if the field is absent or cannot be parsed, in which
/// case the `expires_at` DynamoDB attribute should be omitted entirely
/// (per locked decision: no dummy far-future values).
fn extract_ttl_epoch(data: &[u8]) -> Option<i64> {
    let value: serde_json::Value = serde_json::from_slice(data).ok()?;
    let expires_at_str = value.get("expiresAt")?.as_str()?;
    let dt = chrono::DateTime::parse_from_rfc3339(expires_at_str).ok()?;
    Some(dt.timestamp())
}

/// Maps an AWS SDK error to a [`StorageError::Backend`].
fn map_sdk_error(err: impl std::error::Error + Send + Sync + 'static, key: &str) -> StorageError {
    StorageError::Backend {
        message: format!("DynamoDB error for key {key}: {err}"),
        source: Some(Box::new(err)),
    }
}

// ---------------------------------------------------------------------------
// StorageBackend implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl StorageBackend for DynamoDbBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        let (pk, sk) = split_key(key)?;

        let result = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("PK", AttributeValue::S(pk))
            .key("SK", AttributeValue::S(sk))
            .send()
            .await
            .map_err(|e| map_sdk_error(e, key))?;

        let item = result.item().ok_or_else(|| StorageError::NotFound {
            key: key.to_string(),
        })?;

        let version = item
            .get("version")
            .and_then(|v| v.as_n().ok())
            .and_then(|n| n.parse::<u64>().ok())
            .ok_or_else(|| StorageError::Backend {
                message: format!("missing or invalid version attribute for key {key}"),
                source: None,
            })?;

        let data = item
            .get("data")
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

    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        let (pk, sk) = split_key(key)?;
        let data_str = std::str::from_utf8(data).map_err(|e| StorageError::Backend {
            message: format!("data is not valid UTF-8: {e}"),
            source: Some(Box::new(e)),
        })?;

        // Read current version (if exists) to determine next version.
        let current_version = match self.get(key).await {
            Ok(record) => record.version,
            Err(StorageError::NotFound { .. }) => 0,
            Err(e) => return Err(e),
        };
        let new_version = current_version + 1;

        let mut builder = self
            .client
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

    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        let (pk, sk) = split_key(key)?;
        let data_str = std::str::from_utf8(data).map_err(|e| StorageError::Backend {
            message: format!("data is not valid UTF-8: {e}"),
            source: Some(Box::new(e)),
        })?;

        let new_version = expected_version + 1;

        let mut builder = self
            .client
            .put_item()
            .table_name(&self.table_name)
            .item("PK", AttributeValue::S(pk))
            .item("SK", AttributeValue::S(sk))
            .item("version", AttributeValue::N(new_version.to_string()))
            .item("data", AttributeValue::S(data_str.to_string()))
            .condition_expression("#v = :expected")
            .expression_attribute_names("#v", "version")
            .expression_attribute_values(
                ":expected",
                AttributeValue::N(expected_version.to_string()),
            );

        if let Some(epoch) = extract_ttl_epoch(data) {
            builder = builder.item("expires_at", AttributeValue::N(epoch.to_string()));
        }

        match builder.send().await {
            Ok(_) => Ok(new_version),
            Err(sdk_err) => {
                if let Some(service_err) = sdk_err.as_service_error() {
                    if service_err.is_conditional_check_failed_exception() {
                        return Err(StorageError::VersionConflict {
                            key: key.to_string(),
                            expected: expected_version,
                            // We don't know the actual version without an
                            // extra read; report expected per discretion
                            // decision in research.
                            actual: expected_version,
                        });
                    }
                }
                Err(map_sdk_error(sdk_err, key))
            },
        }
    }

    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        let (pk, sk) = split_key(key)?;

        let result = self
            .client
            .delete_item()
            .table_name(&self.table_name)
            .key("PK", AttributeValue::S(pk))
            .key("SK", AttributeValue::S(sk))
            .return_values(aws_sdk_dynamodb::types::ReturnValue::AllOld)
            .send()
            .await
            .map_err(|e| map_sdk_error(e, key))?;

        Ok(result.attributes().is_some_and(|attrs| !attrs.is_empty()))
    }

    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        let pk = split_prefix(prefix)?;

        let mut results = Vec::new();
        let mut exclusive_start_key = None;

        loop {
            let mut query = self
                .client
                .query()
                .table_name(&self.table_name)
                .key_condition_expression("PK = :pk")
                .expression_attribute_values(":pk", AttributeValue::S(pk.clone()));

            if let Some(start_key) = exclusive_start_key.take() {
                query = query.set_exclusive_start_key(Some(start_key));
            }

            let output = query.send().await.map_err(|e| map_sdk_error(e, prefix))?;

            for item in output.items() {
                let item_pk = item.get("PK").and_then(|v: &AttributeValue| v.as_s().ok());
                let item_sk = item.get("SK").and_then(|v: &AttributeValue| v.as_s().ok());
                let version_str = item
                    .get("version")
                    .and_then(|v: &AttributeValue| v.as_n().ok());
                let data_str = item
                    .get("data")
                    .and_then(|v: &AttributeValue| v.as_s().ok());

                if let (Some(pk_val), Some(sk_val), Some(ver), Some(data)) =
                    (item_pk, item_sk, version_str, data_str)
                {
                    if let (Some(owner), Some(task_id)) = (parse_pk(pk_val), parse_sk(sk_val)) {
                        if let Ok(version) = ver.parse::<u64>() {
                            let composite_key = format!("{owner}:{task_id}");
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

            match output.last_evaluated_key() {
                Some(last_key) if !last_key.is_empty() => {
                    exclusive_start_key = Some(last_key.clone());
                },
                _ => break,
            }
        }

        Ok(results)
    }

    /// No-op for DynamoDB: native TTL handles expired item cleanup
    /// automatically.
    ///
    /// DynamoDB's built-in TTL mechanism deletes items whose `expires_at`
    /// attribute is in the past (within approximately 48 hours). There is
    /// no need for application-level cleanup.
    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// Test-only accessors
// ---------------------------------------------------------------------------

#[cfg(test)]
impl DynamoDbBackend {
    /// Returns a reference to the underlying DynamoDB client.
    ///
    /// Test-only: used by integration tests to inspect raw DynamoDB items
    /// (e.g., verifying `expires_at` TTL attribute presence).
    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    /// Returns the table name this backend operates on.
    ///
    /// Test-only: used by integration tests to issue raw `GetItem` calls
    /// for TTL attribute verification.
    pub(crate) fn table_name(&self) -> &str {
        &self.table_name
    }
}

// ---------------------------------------------------------------------------
// Integration tests -- DynamoDB backend contract tests
// ---------------------------------------------------------------------------

/// Integration tests for [`DynamoDbBackend`] against real AWS DynamoDB.
///
/// These tests require:
/// - Valid AWS credentials (via environment variables, profile, or IMDS)
/// - A DynamoDB table named `pmcp_tasks` with schema:
///   - Partition key: `PK` (String)
///   - Sort key: `SK` (String)
///   - TTL attribute: `expires_at` (enabled via Table settings)
///
/// Run with:
/// ```bash
/// cargo test -p pmcp-tasks --features dynamodb-tests -- ddb_ --test-threads=1
/// ```
///
/// Each test uses a unique UUID-based owner prefix for isolation, so tests
/// do not interfere with each other and no cleanup is needed.
#[cfg(all(test, feature = "dynamodb-tests"))]
mod integration_tests {
    use super::*;
    use crate::domain::TaskRecord;

    /// Creates a test backend from environment and a unique owner prefix.
    ///
    /// The prefix is a random UUID, ensuring each test run operates on
    /// isolated keys with zero chance of collision.
    async fn test_backend() -> (DynamoDbBackend, String) {
        let backend = DynamoDbBackend::from_env().await;
        let test_prefix = format!("test-{}", uuid::Uuid::new_v4());
        (backend, test_prefix)
    }

    // ---- get tests ----

    #[tokio::test]
    async fn ddb_get_missing_key_returns_not_found() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:nonexistent-task");
        let result = backend.get(&key).await;
        assert!(
            matches!(&result, Err(StorageError::NotFound { key: k }) if k == &key),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn ddb_get_returns_stored_data() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        let data = b"hello world";
        let version = backend.put(&key, data).await.unwrap();

        let record = backend.get(&key).await.unwrap();
        assert_eq!(record.data, data);
        assert_eq!(record.version, version);
    }

    // ---- put tests ----

    #[tokio::test]
    async fn ddb_put_new_key_returns_version_1() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        let version = backend.put(&key, b"data").await.unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn ddb_put_existing_key_increments_version() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        let v1 = backend.put(&key, b"first").await.unwrap();
        let v2 = backend.put(&key, b"second").await.unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);

        let record = backend.get(&key).await.unwrap();
        assert_eq!(record.data, b"second");
        assert_eq!(record.version, 2);
    }

    #[tokio::test]
    async fn ddb_put_overwrites_data() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        backend.put(&key, b"original").await.unwrap();
        backend.put(&key, b"updated").await.unwrap();

        let record = backend.get(&key).await.unwrap();
        assert_eq!(record.data, b"updated");
    }

    // ---- put_if_version tests ----

    #[tokio::test]
    async fn ddb_put_if_version_succeeds_on_match() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        let v1 = backend.put(&key, b"data-v1").await.unwrap();
        let v2 = backend
            .put_if_version(&key, b"data-v2", v1)
            .await
            .unwrap();
        assert_eq!(v2, v1 + 1);
    }

    #[tokio::test]
    async fn ddb_put_if_version_fails_on_mismatch() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        backend.put(&key, b"data").await.unwrap();

        let result = backend.put_if_version(&key, b"new-data", 999).await;
        match result {
            Err(StorageError::VersionConflict {
                key: k,
                expected,
                ..
            }) => {
                assert_eq!(k, key);
                assert_eq!(expected, 999);
            },
            other => panic!("expected VersionConflict, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn ddb_put_if_version_fails_on_missing_key() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:nonexistent");
        let result = backend.put_if_version(&key, b"data", 1).await;
        // DynamoDB returns ConditionalCheckFailedException for missing items
        // with a condition expression, which maps to VersionConflict.
        assert!(
            matches!(
                &result,
                Err(StorageError::VersionConflict { .. })
            ),
            "expected VersionConflict for missing key with condition, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn ddb_put_if_version_updates_data() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        let v1 = backend.put(&key, b"original").await.unwrap();
        backend
            .put_if_version(&key, b"cas-updated", v1)
            .await
            .unwrap();

        let record = backend.get(&key).await.unwrap();
        assert_eq!(record.data, b"cas-updated");
    }

    // ---- delete tests ----

    #[tokio::test]
    async fn ddb_delete_existing_returns_true() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        backend.put(&key, b"data").await.unwrap();
        let deleted = backend.delete(&key).await.unwrap();
        assert!(deleted);
    }

    #[tokio::test]
    async fn ddb_delete_missing_returns_false() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:nonexistent");
        let deleted = backend.delete(&key).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn ddb_delete_then_get_returns_not_found() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-1");
        backend.put(&key, b"data").await.unwrap();
        backend.delete(&key).await.unwrap();

        let result = backend.get(&key).await;
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
    }

    // ---- list_by_prefix tests ----

    #[tokio::test]
    async fn ddb_list_by_prefix_returns_matching() {
        let (backend, prefix) = test_backend().await;
        let owner_a = format!("{prefix}-a");
        let owner_b = format!("{prefix}-b");

        backend
            .put(&format!("{owner_a}:task-1"), b"data-a1")
            .await
            .unwrap();
        backend
            .put(&format!("{owner_a}:task-2"), b"data-a2")
            .await
            .unwrap();
        backend
            .put(&format!("{owner_b}:task-3"), b"data-b1")
            .await
            .unwrap();

        let results = backend
            .list_by_prefix(&format!("{owner_a}:"))
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        let keys: Vec<&str> = results.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&format!("{owner_a}:task-1").as_str()));
        assert!(keys.contains(&format!("{owner_a}:task-2").as_str()));
    }

    #[tokio::test]
    async fn ddb_list_by_prefix_empty_on_no_match() {
        let (backend, prefix) = test_backend().await;
        let owner = format!("{prefix}-nomatch");
        let results = backend
            .list_by_prefix(&format!("{owner}:"))
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn ddb_list_by_prefix_returns_correct_data_and_versions() {
        let (backend, prefix) = test_backend().await;
        let owner = format!("{prefix}-owner");

        backend
            .put(&format!("{owner}:task-1"), b"data-1")
            .await
            .unwrap();
        backend
            .put(&format!("{owner}:task-2"), b"data-2")
            .await
            .unwrap();
        // Update task-2 to get version 2
        backend
            .put(&format!("{owner}:task-2"), b"data-2-v2")
            .await
            .unwrap();

        let results = backend
            .list_by_prefix(&format!("{owner}:"))
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        for (key, record) in &results {
            if key.ends_with(":task-1") {
                assert_eq!(record.data, b"data-1");
                assert_eq!(record.version, 1);
            } else if key.ends_with(":task-2") {
                assert_eq!(record.data, b"data-2-v2");
                assert_eq!(record.version, 2);
            } else {
                panic!("unexpected key: {key}");
            }
        }
    }

    // ---- cleanup_expired tests ----

    #[tokio::test]
    async fn ddb_cleanup_expired_returns_zero() {
        let (backend, _prefix) = test_backend().await;
        let removed = backend.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
    }

    // ---- TTL verification tests ----

    #[tokio::test]
    async fn ddb_put_sets_expires_at_attribute_when_ttl_present() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-ttl");

        // Create a TaskRecord with a 1-hour TTL
        let record = TaskRecord::new(prefix.clone(), "tools/call".to_string(), Some(3_600_000));
        let data = serde_json::to_vec(&record).unwrap();
        backend.put(&key, &data).await.unwrap();

        // Directly inspect the raw DynamoDB item
        let (pk, sk) = split_key(&key).unwrap();
        let raw = backend
            .client()
            .get_item()
            .table_name(backend.table_name())
            .key("PK", AttributeValue::S(pk))
            .key("SK", AttributeValue::S(sk))
            .send()
            .await
            .expect("GetItem should succeed");

        let item = raw.item().expect("item should exist");

        // Verify expires_at attribute is present and is a Number
        let expires_at_attr = item
            .get("expires_at")
            .expect("expires_at attribute should exist");
        let epoch_str = expires_at_attr
            .as_n()
            .expect("expires_at should be a Number");
        let epoch: i64 = epoch_str
            .parse()
            .expect("expires_at should be parseable as i64");

        // Verify the epoch is reasonable (within ~2 hours of now)
        let now_epoch = chrono::Utc::now().timestamp();
        assert!(
            epoch > now_epoch && epoch < now_epoch + 7200,
            "expires_at epoch {epoch} should be within 2 hours of now ({now_epoch})"
        );
    }

    #[tokio::test]
    async fn ddb_put_omits_expires_at_when_no_ttl() {
        let (backend, prefix) = test_backend().await;
        let key = format!("{prefix}:task-no-ttl");

        // Create a TaskRecord without TTL
        let record = TaskRecord::new(prefix.clone(), "tools/call".to_string(), None);
        let data = serde_json::to_vec(&record).unwrap();
        backend.put(&key, &data).await.unwrap();

        // Directly inspect the raw DynamoDB item
        let (pk, sk) = split_key(&key).unwrap();
        let raw = backend
            .client()
            .get_item()
            .table_name(backend.table_name())
            .key("PK", AttributeValue::S(pk))
            .key("SK", AttributeValue::S(sk))
            .send()
            .await
            .expect("GetItem should succeed");

        let item = raw.item().expect("item should exist");

        // Verify expires_at attribute is absent
        assert!(
            item.get("expires_at").is_none(),
            "expires_at attribute should be absent when TTL is None"
        );
    }
}
