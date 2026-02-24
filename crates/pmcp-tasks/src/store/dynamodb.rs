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
