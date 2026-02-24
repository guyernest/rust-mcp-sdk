//! Redis storage backend for task persistence.
//!
//! [`RedisBackend`] implements [`StorageBackend`] using Redis as the underlying
//! key-value store. It maps the 6 trait methods to Redis operations: `HGETALL`
//! for reads, and Lua scripts (`redis::Script`) for atomic writes that update
//! hash fields, sorted set indexes, and TTL in a single round-trip.
//!
//! # Key Schema
//!
//! | Key Pattern | Type | Purpose |
//! |-------------|------|---------|
//! | `{prefix}:tasks:{owner_id}:{task_id}` | Hash | Task record storage |
//! | `{prefix}:idx:{owner_id}` | Sorted Set | Owner-scoped task index |
//!
//! Each task is stored as a Redis hash with separate fields:
//!
//! | Field | Type | Description |
//! |-------|------|-------------|
//! | `version` | String (u64) | Monotonic CAS version, starts at 1 |
//! | `data` | String (JSON) | Serialized `TaskRecord` JSON blob |
//! | `expires_at` | String (i64) | Unix epoch seconds (only present with TTL) |
//!
//! # Relationship to GenericTaskStore
//!
//! This backend is a **dumb KV adapter**. It stores and retrieves opaque byte
//! blobs (serialized JSON). All domain logic -- state machine validation, owner
//! checking, variable merge, TTL policy -- lives in
//! [`GenericTaskStore`](crate::store::generic::GenericTaskStore). The backend
//! never interprets the data it stores, except for extracting the `expiresAt`
//! field to set the Redis TTL and `task.createdAt` for sorted set scoring.
//!
//! # Usage
//!
//! ```rust,no_run
//! use pmcp_tasks::store::redis::RedisBackend;
//! use pmcp_tasks::store::generic::GenericTaskStore;
//!
//! # async fn example() {
//! let backend = RedisBackend::new("redis://127.0.0.1:6379").await.unwrap();
//! let store = GenericTaskStore::new(backend);
//! # }
//! ```

use std::collections::HashMap;

use ::redis::aio::MultiplexedConnection;
use ::redis::{AsyncCommands, Script};
use async_trait::async_trait;

use crate::store::backend::{StorageBackend, StorageError, VersionedRecord};

// ---------------------------------------------------------------------------
// Lua script constants
// ---------------------------------------------------------------------------

/// Unconditional put: update hash, maintain sorted set index, set TTL.
///
/// KEYS[1] = task hash key, KEYS[2] = owner index sorted set key.
/// ARGV[1] = data JSON, ARGV[2] = expires_at epoch (or "" if no TTL),
/// ARGV[3] = task_id, ARGV[4] = creation timestamp score.
/// Returns: new version number.
const LUA_PUT: &str = r#"
local current_version = redis.call('HGET', KEYS[1], 'version')
local new_version
if current_version then
    new_version = tonumber(current_version) + 1
else
    new_version = 1
end

redis.call('HSET', KEYS[1], 'version', tostring(new_version), 'data', ARGV[1])

if ARGV[2] ~= '' then
    redis.call('HSET', KEYS[1], 'expires_at', ARGV[2])
    redis.call('EXPIREAT', KEYS[1], tonumber(ARGV[2]))
else
    redis.call('HDEL', KEYS[1], 'expires_at')
    redis.call('PERSIST', KEYS[1])
end

redis.call('ZADD', KEYS[2], 'NX', tonumber(ARGV[4]), ARGV[3])

return new_version
"#;

/// Conditional put (CAS): check version, then update or reject.
///
/// KEYS[1] = task hash key, KEYS[2] = owner index sorted set key.
/// ARGV[1] = data, ARGV[2] = expected_version, ARGV[3] = expires_at epoch
/// (or ""), ARGV[4] = task_id, ARGV[5] = creation timestamp score.
/// Returns: {status, value} where status 1=success, 0=mismatch, -1=missing.
const LUA_PUT_IF_VERSION: &str = r#"
local current_version = redis.call('HGET', KEYS[1], 'version')
if not current_version then
    return {-1, 0}
end

local expected = tonumber(ARGV[2])
local actual = tonumber(current_version)
if actual ~= expected then
    return {0, actual}
end

local new_version = actual + 1
redis.call('HSET', KEYS[1], 'version', tostring(new_version), 'data', ARGV[1])

if ARGV[3] ~= '' then
    redis.call('HSET', KEYS[1], 'expires_at', ARGV[3])
    redis.call('EXPIREAT', KEYS[1], tonumber(ARGV[3]))
else
    redis.call('HDEL', KEYS[1], 'expires_at')
    redis.call('PERSIST', KEYS[1])
end

redis.call('ZADD', KEYS[2], 'NX', tonumber(ARGV[5]), ARGV[4])

return {1, new_version}
"#;

/// Delete: remove hash and sorted set entry.
///
/// KEYS[1] = task hash key, KEYS[2] = owner index sorted set key.
/// ARGV[1] = task_id (sorted set member).
/// Returns: 1 if key existed and was deleted, 0 otherwise.
const LUA_DELETE: &str = r#"
local existed = redis.call('EXISTS', KEYS[1])
if existed == 1 then
    redis.call('DEL', KEYS[1])
    redis.call('ZREM', KEYS[2], ARGV[1])
    return 1
end
return 0
"#;

// ---------------------------------------------------------------------------
// RedisBackend struct
// ---------------------------------------------------------------------------

/// Redis storage backend for task persistence.
///
/// Stores task records as Redis hashes with field-level mapping (version, data,
/// expires_at). Owner-scoped listing uses per-owner sorted set indexes. All
/// write operations are atomic via Lua scripts that update hash + index + TTL
/// in a single round-trip.
///
/// This backend is a thin adapter -- it contains **no domain logic**. All
/// intelligence (state machine validation, owner isolation, variable merge,
/// TTL enforcement) lives in
/// [`GenericTaskStore`](crate::store::generic::GenericTaskStore).
///
/// # Connection Model
///
/// `RedisBackend` holds a [`MultiplexedConnection`] which is designed to be
/// cloned cheaply -- all clones share the same underlying TCP connection.
/// Each method clones the connection for concurrent safety.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp_tasks::store::redis::RedisBackend;
/// use pmcp_tasks::store::generic::GenericTaskStore;
///
/// # async fn example() {
/// // Connect to local Redis:
/// let backend = RedisBackend::new("redis://127.0.0.1:6379").await.unwrap();
/// let store = GenericTaskStore::new(backend);
///
/// // With custom prefix for isolation:
/// let backend = RedisBackend::new("redis://127.0.0.1:6379")
///     .await
///     .unwrap()
///     .with_prefix("my-app");
/// let store = GenericTaskStore::new(backend);
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct RedisBackend {
    conn: MultiplexedConnection,
    key_prefix: String,
}

impl RedisBackend {
    /// Creates a backend by connecting to Redis at the given URL.
    ///
    /// The URL format is `redis://[:<password>@]<host>:<port>[/<db>]`.
    /// Uses the default key prefix `"pmcp"`. Fails fast if the connection
    /// cannot be established.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Backend`] if the Redis client cannot be created
    /// or the connection cannot be established.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp_tasks::store::redis::RedisBackend;
    ///
    /// # async fn example() {
    /// let backend = RedisBackend::new("redis://127.0.0.1:6379").await.unwrap();
    /// # }
    /// ```
    pub async fn new(url: &str) -> Result<Self, StorageError> {
        let client = ::redis::Client::open(url).map_err(|e| StorageError::Backend {
            message: format!("failed to create Redis client: {e}"),
            source: Some(Box::new(e)),
        })?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| StorageError::Backend {
                message: format!("failed to connect to Redis: {e}"),
                source: Some(Box::new(e)),
            })?;
        Ok(Self {
            conn,
            key_prefix: "pmcp".to_string(),
        })
    }

    /// Creates a backend with a pre-built multiplexed connection.
    ///
    /// Useful when the caller manages connection lifecycle or needs custom
    /// connection configuration. Uses the default key prefix `"pmcp"`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp_tasks::store::redis::RedisBackend;
    ///
    /// # async fn example() {
    /// let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    /// let conn = client.get_multiplexed_async_connection().await.unwrap();
    /// let backend = RedisBackend::with_connection(conn);
    /// # }
    /// ```
    pub fn with_connection(conn: MultiplexedConnection) -> Self {
        Self {
            conn,
            key_prefix: "pmcp".to_string(),
        }
    }

    /// Sets a custom key prefix (builder pattern).
    ///
    /// Useful for test isolation: each test run can use a unique prefix to
    /// avoid key collisions. The prefix is used in all Redis keys:
    /// `{prefix}:tasks:{owner_id}:{task_id}` and `{prefix}:idx:{owner_id}`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp_tasks::store::redis::RedisBackend;
    ///
    /// # async fn example() {
    /// let backend = RedisBackend::new("redis://127.0.0.1:6379")
    ///     .await
    ///     .unwrap()
    ///     .with_prefix("test-abc123");
    /// # }
    /// ```
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = prefix.into();
        self
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl RedisBackend {
    /// Constructs the Redis hash key for a task.
    fn task_key(&self, owner_id: &str, task_id: &str) -> String {
        format!("{}:tasks:{}:{}", self.key_prefix, owner_id, task_id)
    }

    /// Constructs the Redis sorted set key for an owner's task index.
    fn index_key(&self, owner_id: &str) -> String {
        format!("{}:idx:{}", self.key_prefix, owner_id)
    }
}

/// Splits a composite `{owner_id}:{task_id}` key into `(owner_id, task_id)`.
fn split_key(key: &str) -> Result<(&str, &str), StorageError> {
    key.split_once(':').ok_or_else(|| StorageError::Backend {
        message: format!("invalid key format (missing ':'): {key}"),
        source: None,
    })
}

/// Splits a composite prefix `{owner_id}:` into the owner_id by stripping
/// the trailing colon.
fn split_prefix(prefix: &str) -> Result<&str, StorageError> {
    prefix
        .strip_suffix(':')
        .ok_or_else(|| StorageError::Backend {
            message: format!("invalid prefix format (missing trailing ':'): {prefix}"),
            source: None,
        })
}

/// Extracts the TTL epoch seconds from serialized task record JSON.
///
/// Parses the `expiresAt` field (an RFC 3339 datetime string) from the
/// serialized `TaskRecord` JSON and converts it to Unix epoch seconds.
/// Returns `None` if the field is absent or cannot be parsed.
fn extract_ttl_epoch(data: &[u8]) -> Option<i64> {
    let value: serde_json::Value = serde_json::from_slice(data).ok()?;
    let expires_at_str = value.get("expiresAt")?.as_str()?;
    let dt = chrono::DateTime::parse_from_rfc3339(expires_at_str).ok()?;
    Some(dt.timestamp())
}

/// Extracts the creation timestamp in milliseconds from serialized JSON.
///
/// Parses the `task.createdAt` field (RFC 3339 datetime) and converts it to
/// epoch milliseconds. Used as the score for sorted set indexing (preserves
/// creation ordering).
fn extract_created_at_ms(data: &[u8]) -> Option<i64> {
    let value: serde_json::Value = serde_json::from_slice(data).ok()?;
    let created_at_str = value.get("task")?.get("createdAt")?.as_str()?;
    let dt = chrono::DateTime::parse_from_rfc3339(created_at_str).ok()?;
    Some(dt.timestamp_millis())
}

/// Maps a Redis error to a [`StorageError::Backend`].
fn map_redis_error(err: ::redis::RedisError, key: &str) -> StorageError {
    StorageError::Backend {
        message: format!("Redis error for key {key}: {err}"),
        source: Some(Box::new(err)),
    }
}

/// Checks whether a task hash has expired based on its `expires_at` field.
///
/// Returns `true` if the `expires_at` field is present and its epoch value
/// is less than or equal to the current time.
fn is_expired(fields: &HashMap<String, String>) -> bool {
    if let Some(expires_at_str) = fields.get("expires_at") {
        if let Ok(epoch) = expires_at_str.parse::<i64>() {
            return epoch <= chrono::Utc::now().timestamp();
        }
    }
    false
}

// ---------------------------------------------------------------------------
// StorageBackend implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl StorageBackend for RedisBackend {
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError> {
        let (owner_id, task_id) = split_key(key)?;
        let hash_key = self.task_key(owner_id, task_id);
        let mut conn = self.conn.clone();

        let result: HashMap<String, String> = conn
            .hgetall(&hash_key)
            .await
            .map_err(|e| map_redis_error(e, key))?;

        if result.is_empty() {
            return Err(StorageError::NotFound {
                key: key.to_string(),
            });
        }

        // Application-level expiry check
        if is_expired(&result) {
            return Err(StorageError::NotFound {
                key: key.to_string(),
            });
        }

        let version: u64 = result
            .get("version")
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| StorageError::Backend {
                message: format!("missing or invalid version field for key {key}"),
                source: None,
            })?;

        let data = result.get("data").ok_or_else(|| StorageError::Backend {
            message: format!("missing data field for key {key}"),
            source: None,
        })?;

        Ok(VersionedRecord {
            data: data.as_bytes().to_vec(),
            version,
        })
    }

    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError> {
        let (owner_id, task_id) = split_key(key)?;
        let hash_key = self.task_key(owner_id, task_id);
        let idx_key = self.index_key(owner_id);

        let data_str = std::str::from_utf8(data).map_err(|e| StorageError::Backend {
            message: format!("data is not valid UTF-8: {e}"),
            source: Some(Box::new(e)),
        })?;

        let expires_at_str = extract_ttl_epoch(data)
            .map(|e| e.to_string())
            .unwrap_or_default();
        let creation_ts = extract_created_at_ms(data).unwrap_or(0);

        let script = Script::new(LUA_PUT);
        let new_version: u64 = script
            .key(&hash_key)
            .key(&idx_key)
            .arg(data_str)
            .arg(&expires_at_str)
            .arg(task_id)
            .arg(creation_ts)
            .invoke_async(&mut self.conn.clone())
            .await
            .map_err(|e| map_redis_error(e, key))?;

        Ok(new_version)
    }

    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError> {
        let (owner_id, task_id) = split_key(key)?;
        let hash_key = self.task_key(owner_id, task_id);
        let idx_key = self.index_key(owner_id);

        let data_str = std::str::from_utf8(data).map_err(|e| StorageError::Backend {
            message: format!("data is not valid UTF-8: {e}"),
            source: Some(Box::new(e)),
        })?;

        let expires_at_str = extract_ttl_epoch(data)
            .map(|e| e.to_string())
            .unwrap_or_default();
        let creation_ts = extract_created_at_ms(data).unwrap_or(0);

        let script = Script::new(LUA_PUT_IF_VERSION);
        let result: (i64, i64) = script
            .key(&hash_key)
            .key(&idx_key)
            .arg(data_str)
            .arg(expected_version)
            .arg(&expires_at_str)
            .arg(task_id)
            .arg(creation_ts)
            .invoke_async(&mut self.conn.clone())
            .await
            .map_err(|e| map_redis_error(e, key))?;

        match result.0 {
            1 => Ok(result.1 as u64),
            0 => Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected: expected_version,
                actual: result.1 as u64,
            }),
            // -1: key does not exist -- return VersionConflict per DynamoDB
            // backend pattern (missing key with CAS returns VersionConflict).
            _ => Err(StorageError::VersionConflict {
                key: key.to_string(),
                expected: expected_version,
                actual: 0,
            }),
        }
    }

    async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        let (owner_id, task_id) = split_key(key)?;
        let hash_key = self.task_key(owner_id, task_id);
        let idx_key = self.index_key(owner_id);

        let script = Script::new(LUA_DELETE);
        let result: i64 = script
            .key(&hash_key)
            .key(&idx_key)
            .arg(task_id)
            .invoke_async(&mut self.conn.clone())
            .await
            .map_err(|e| map_redis_error(e, key))?;

        Ok(result == 1)
    }

    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError> {
        let owner_id = split_prefix(prefix)?;
        let idx_key = self.index_key(owner_id);

        // Get all task_ids from the owner's sorted set (sorted by creation time)
        let task_ids: Vec<String> = self
            .conn
            .clone()
            .zrange(&idx_key, 0, -1)
            .await
            .map_err(|e| map_redis_error(e, prefix))?;

        let mut results = Vec::with_capacity(task_ids.len());
        let mut orphaned_ids: Vec<String> = Vec::new();

        for task_id in &task_ids {
            let hash_key = self.task_key(owner_id, task_id);
            let fields: HashMap<String, String> = self
                .conn
                .clone()
                .hgetall(&hash_key)
                .await
                .map_err(|e| map_redis_error(e, prefix))?;

            if fields.is_empty() {
                // Orphaned index entry: hash expired but sorted set entry
                // remains.
                orphaned_ids.push(task_id.clone());
                continue;
            }

            // Application-level expiry filtering
            if is_expired(&fields) {
                orphaned_ids.push(task_id.clone());
                continue;
            }

            let version: u64 = fields
                .get("version")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0);
            let data = fields.get("data").cloned().unwrap_or_default();
            let composite_key = format!("{owner_id}:{task_id}");

            results.push((
                composite_key,
                VersionedRecord {
                    data: data.into_bytes(),
                    version,
                },
            ));
        }

        // Lazy cleanup of orphaned sorted set entries
        if !orphaned_ids.is_empty() {
            let _: Result<(), ::redis::RedisError> =
                self.conn.clone().zrem(&idx_key, &orphaned_ids).await;
            // Ignore cleanup errors -- best-effort
        }

        Ok(results)
    }

    /// No-op for Redis: `EXPIRE`/`EXPIREAT` handles expired key cleanup
    /// automatically.
    ///
    /// Orphaned sorted set entries (pointing to expired hashes) are cleaned
    /// lazily during [`list_by_prefix`](StorageBackend::list_by_prefix).
    async fn cleanup_expired(&self) -> Result<usize, StorageError> {
        Ok(0)
    }
}

// ---------------------------------------------------------------------------
// Integration tests -- Redis backend contract tests
// ---------------------------------------------------------------------------

/// Integration tests for [`RedisBackend`] against a real Redis instance.
///
/// These tests require:
/// - A running Redis instance (default: `redis://127.0.0.1:6379`)
/// - Set `REDIS_URL` environment variable to override the connection URL
///
/// Run with:
/// ```bash
/// cargo test -p pmcp-tasks --features redis-tests -- redis_ --test-threads=1
/// ```
///
/// Each test uses a unique UUID-based key prefix for isolation, so tests
/// do not interfere with each other and no cleanup is needed.
#[cfg(all(test, feature = "redis-tests"))]
mod integration_tests {
    use super::*;
    use crate::domain::TaskRecord;

    /// Creates a test backend with a unique key prefix for isolation.
    async fn test_backend() -> (RedisBackend, String) {
        let url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
        let backend = RedisBackend::new(&url)
            .await
            .expect("Redis connection failed -- is Redis running?");
        let test_prefix = format!("test-{}", uuid::Uuid::new_v4());
        let backend = backend.with_prefix(test_prefix.clone());
        (backend, test_prefix)
    }

    // ---- get tests ----

    #[tokio::test]
    async fn redis_get_missing_key_returns_not_found() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:nonexistent-task";
        let result = backend.get(key).await;
        assert!(
            matches!(&result, Err(StorageError::NotFound { key: k }) if k == key),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn redis_get_returns_stored_data() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        let data = b"hello world";
        let version = backend.put(key, data).await.unwrap();

        let record = backend.get(key).await.unwrap();
        assert_eq!(record.data, data);
        assert_eq!(record.version, version);
    }

    // ---- put tests ----

    #[tokio::test]
    async fn redis_put_new_key_returns_version_1() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        let version = backend.put(key, b"data").await.unwrap();
        assert_eq!(version, 1);
    }

    #[tokio::test]
    async fn redis_put_existing_key_increments_version() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        let v1 = backend.put(key, b"first").await.unwrap();
        let v2 = backend.put(key, b"second").await.unwrap();
        assert_eq!(v1, 1);
        assert_eq!(v2, 2);

        let record = backend.get(key).await.unwrap();
        assert_eq!(record.data, b"second");
        assert_eq!(record.version, 2);
    }

    #[tokio::test]
    async fn redis_put_overwrites_data() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        backend.put(key, b"original").await.unwrap();
        backend.put(key, b"updated").await.unwrap();

        let record = backend.get(key).await.unwrap();
        assert_eq!(record.data, b"updated");
    }

    // ---- put_if_version tests ----

    #[tokio::test]
    async fn redis_put_if_version_succeeds_on_match() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        let v1 = backend.put(key, b"data-v1").await.unwrap();
        let v2 = backend.put_if_version(key, b"data-v2", v1).await.unwrap();
        assert_eq!(v2, v1 + 1);
    }

    #[tokio::test]
    async fn redis_put_if_version_fails_on_mismatch() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        backend.put(key, b"data").await.unwrap();

        let result = backend.put_if_version(key, b"new-data", 999).await;
        match result {
            Err(StorageError::VersionConflict {
                key: k, expected, ..
            }) => {
                assert_eq!(k, key);
                assert_eq!(expected, 999);
            },
            other => panic!("expected VersionConflict, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn redis_put_if_version_fails_on_missing_key() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:nonexistent";
        let result = backend.put_if_version(key, b"data", 1).await;
        assert!(
            matches!(&result, Err(StorageError::VersionConflict { .. })),
            "expected VersionConflict for missing key with condition, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn redis_put_if_version_updates_data() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        let v1 = backend.put(key, b"original").await.unwrap();
        backend
            .put_if_version(key, b"cas-updated", v1)
            .await
            .unwrap();

        let record = backend.get(key).await.unwrap();
        assert_eq!(record.data, b"cas-updated");
    }

    // ---- delete tests ----

    #[tokio::test]
    async fn redis_delete_existing_returns_true() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        backend.put(key, b"data").await.unwrap();
        let deleted = backend.delete(key).await.unwrap();
        assert!(deleted);
    }

    #[tokio::test]
    async fn redis_delete_missing_returns_false() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:nonexistent";
        let deleted = backend.delete(key).await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn redis_delete_then_get_returns_not_found() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-1";
        backend.put(key, b"data").await.unwrap();
        backend.delete(key).await.unwrap();

        let result = backend.get(key).await;
        assert!(matches!(result, Err(StorageError::NotFound { .. })));
    }

    // ---- list_by_prefix tests ----

    #[tokio::test]
    async fn redis_list_by_prefix_returns_matching() {
        let (backend, _prefix) = test_backend().await;
        let owner_a = "owner-a";
        let owner_b = "owner-b";

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
    async fn redis_list_by_prefix_empty_on_no_match() {
        let (backend, _prefix) = test_backend().await;
        let owner = "owner-nomatch";
        let results = backend.list_by_prefix(&format!("{owner}:")).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn redis_list_by_prefix_returns_correct_data_and_versions() {
        let (backend, _prefix) = test_backend().await;
        let owner = "owner";

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

        let results = backend.list_by_prefix(&format!("{owner}:")).await.unwrap();
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
    async fn redis_cleanup_expired_returns_zero() {
        let (backend, _prefix) = test_backend().await;
        let removed = backend.cleanup_expired().await.unwrap();
        assert_eq!(removed, 0);
    }

    // ---- TTL verification tests ----

    #[tokio::test]
    async fn redis_put_sets_ttl_when_expires_at_present() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-ttl";

        // Create a TaskRecord with a 1-hour TTL
        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            Some(3_600_000),
        );
        let data = serde_json::to_vec(&record).unwrap();
        backend.put(key, &data).await.unwrap();

        // Verify the hash has an expires_at field
        let (owner_id, task_id) = split_key(key).unwrap();
        let hash_key = backend.task_key(owner_id, task_id);
        let fields: HashMap<String, String> =
            backend.conn.clone().hgetall(&hash_key).await.unwrap();

        let expires_at_str = fields
            .get("expires_at")
            .expect("expires_at field should exist");
        let epoch: i64 = expires_at_str
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
    async fn redis_put_omits_expires_at_when_no_ttl() {
        let (backend, _prefix) = test_backend().await;
        let key = "owner:task-no-ttl";

        // Create a TaskRecord without TTL
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let data = serde_json::to_vec(&record).unwrap();
        backend.put(key, &data).await.unwrap();

        // Verify the hash does not have an expires_at field
        let (owner_id, task_id) = split_key(key).unwrap();
        let hash_key = backend.task_key(owner_id, task_id);
        let fields: HashMap<String, String> =
            backend.conn.clone().hgetall(&hash_key).await.unwrap();

        assert!(
            fields.get("expires_at").is_none(),
            "expires_at field should be absent when TTL is None"
        );
    }
}
