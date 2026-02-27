//! Low-level key-value storage backend trait and supporting types.
//!
//! The [`StorageBackend`] trait defines the contract that all storage engines
//! implement. It exposes 6 KV operations: [`get`](StorageBackend::get),
//! [`put`](StorageBackend::put), [`put_if_version`](StorageBackend::put_if_version),
//! [`delete`](StorageBackend::delete), [`list_by_prefix`](StorageBackend::list_by_prefix),
//! and [`cleanup_expired`](StorageBackend::cleanup_expired).
//!
//! Domain logic (state machine validation, owner isolation, variable merge,
//! TTL enforcement, serialization) does **not** belong here. Backends are
//! dumb KV stores; domain logic lives in `GenericTaskStore`.
//!
//! # Key Structure
//!
//! Keys are composite strings in the format `{owner_id}:{task_id}`. The
//! colon separator is safe because `owner_id` comes from OAuth/session
//! tokens (no colons) and `task_id` is a `UUIDv4` (no colons). Prefix
//! queries use `{owner_id}:` to scope listings to an owner.
//!
//! # Versioning
//!
//! Each stored record carries a monotonic `u64` version number starting at
//! 1, incremented on every successful write. The [`put_if_version`](StorageBackend::put_if_version)
//! method provides compare-and-swap (CAS) semantics for optimistic concurrency.

use std::fmt;

use async_trait::async_trait;

/// A stored record paired with its monotonic version number.
///
/// The `data` field holds the serialized task record bytes (canonical JSON,
/// produced by `GenericTaskStore`). The `version` field is a monotonic
/// counter starting at 1 that increments on every successful write, enabling
/// optimistic concurrency via [`StorageBackend::put_if_version`].
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::backend::VersionedRecord;
///
/// let record = VersionedRecord {
///     data: b"{}".to_vec(),
///     version: 1,
/// };
/// assert_eq!(record.version, 1);
/// assert_eq!(record.data, b"{}");
/// ```
#[derive(Debug, Clone)]
pub struct VersionedRecord {
    /// The serialized task record bytes (canonical JSON).
    pub data: Vec<u8>,

    /// Monotonic version number. Starts at 1, increments on each
    /// successful write. Used by [`StorageBackend::put_if_version`]
    /// for optimistic concurrency control.
    pub version: u64,
}

/// Errors that can occur during raw storage operations.
///
/// These are low-level errors from the storage backend. `GenericTaskStore`
/// maps them to domain-aware [`TaskError`](crate::error::TaskError) variants
/// before surfacing to callers.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::backend::StorageError;
///
/// let err = StorageError::NotFound { key: "owner:task-123".to_string() };
/// assert!(err.to_string().contains("owner:task-123"));
///
/// let err = StorageError::VersionConflict {
///     key: "k".to_string(),
///     expected: 2,
///     actual: 3,
/// };
/// assert!(err.to_string().contains("expected 2"));
/// ```
#[derive(Debug)]
pub enum StorageError {
    /// The requested key was not found in storage.
    NotFound {
        /// The key that was not found.
        key: String,
    },

    /// A [`put_if_version`](StorageBackend::put_if_version) call failed
    /// because the stored version does not match the expected version.
    VersionConflict {
        /// The key where the conflict occurred.
        key: String,
        /// The version the caller expected.
        expected: u64,
        /// The actual version found in storage.
        actual: u64,
    },

    /// The backend has reached a capacity limit (e.g., max records,
    /// max storage size).
    CapacityExceeded {
        /// Human-readable description of the capacity issue.
        message: String,
    },

    /// An I/O or backend-specific error occurred (e.g., network failure,
    /// database timeout, file system error).
    Backend {
        /// Human-readable description of the error.
        message: String,
        /// The underlying error, if available. Accessible via
        /// [`std::error::Error::source()`].
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { key } => write!(f, "key not found: {key}"),
            Self::VersionConflict {
                key,
                expected,
                actual,
            } => write!(
                f,
                "version conflict on key {key}: expected {expected}, found {actual}"
            ),
            Self::CapacityExceeded { message } => {
                write!(f, "capacity exceeded: {message}")
            },
            Self::Backend { message, .. } => write!(f, "backend error: {message}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Backend {
                source: Some(src), ..
            } => Some(src.as_ref()),
            _ => None,
        }
    }
}

/// Key-value storage backend for task persistence.
///
/// Implementations provide raw storage primitives. All domain logic
/// (state machine validation, owner isolation, variable merge, TTL
/// enforcement, serialization) lives in `GenericTaskStore`, **not** in
/// the backend.
///
/// # Key Format
///
/// Keys are composite strings: `{owner_id}:{task_id}`. Prefix queries
/// use `{owner_id}:` to scope listings to a single owner. Backends
/// must store and return keys verbatim.
///
/// # Thread Safety
///
/// Implementations must be `Send + Sync` to support concurrent access
/// from multiple request handlers.
///
/// # Versioning
///
/// Every stored record has a monotonic `u64` version starting at 1.
/// The version increments on each successful write. The
/// [`put_if_version`](StorageBackend::put_if_version) method provides
/// compare-and-swap (CAS) semantics for optimistic concurrency.
///
/// # No Domain Logic
///
/// Backends must **never** implement state machine validation, owner
/// checking, variable merging, or TTL policy logic. They are dumb KV
/// stores. Domain logic belongs in `GenericTaskStore`.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Retrieves a record by key.
    ///
    /// Returns the serialized record bytes and current version number.
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotFound`] if no record exists for the given key.
    /// - [`StorageError::Backend`] on I/O or backend-specific failures.
    async fn get(&self, key: &str) -> Result<VersionedRecord, StorageError>;

    /// Stores a record unconditionally (create or overwrite).
    ///
    /// For new keys, the backend assigns version 1. For existing keys,
    /// the backend increments the current version. Returns the assigned
    /// version number.
    ///
    /// # Errors
    ///
    /// - [`StorageError::CapacityExceeded`] if the backend is at capacity.
    /// - [`StorageError::Backend`] on I/O or backend-specific failures.
    async fn put(&self, key: &str, data: &[u8]) -> Result<u64, StorageError>;

    /// Stores a record only if the current version matches `expected_version`.
    ///
    /// This is the compare-and-swap (CAS) primitive. On success, the
    /// version is incremented and the new version is returned. On failure
    /// (version mismatch), returns [`StorageError::VersionConflict`] with
    /// both the expected and actual versions.
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotFound`] if no record exists for the given key.
    /// - [`StorageError::VersionConflict`] if the stored version does not
    ///   match `expected_version`.
    /// - [`StorageError::CapacityExceeded`] if the backend is at capacity.
    /// - [`StorageError::Backend`] on I/O or backend-specific failures.
    async fn put_if_version(
        &self,
        key: &str,
        data: &[u8],
        expected_version: u64,
    ) -> Result<u64, StorageError>;

    /// Deletes a record by key.
    ///
    /// Returns `true` if the key existed and was deleted, `false` if the
    /// key did not exist (idempotent delete).
    ///
    /// # Errors
    ///
    /// - [`StorageError::Backend`] on I/O or backend-specific failures.
    async fn delete(&self, key: &str) -> Result<bool, StorageError>;

    /// Lists all records whose key starts with the given prefix.
    ///
    /// Returns `(key, VersionedRecord)` tuples for all matching records.
    /// Used for owner-scoped listing with prefix `{owner_id}:`.
    ///
    /// # Errors
    ///
    /// - [`StorageError::Backend`] on I/O or backend-specific failures.
    async fn list_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, VersionedRecord)>, StorageError>;

    /// Removes records that have expired, using backend-specific criteria.
    ///
    /// This is a best-effort operation. Different backends handle TTL
    /// differently:
    /// - In-memory: scans all records, checks TTL, removes expired ones.
    /// - `DynamoDB`: no-op (native TTL handles cleanup automatically).
    /// - Redis: scans and removes expired records.
    ///
    /// Returns the count of records actually removed. Callers should not
    /// depend on this for correctness; expiry is also checked at read time.
    ///
    /// # Errors
    ///
    /// - [`StorageError::Backend`] on I/O or backend-specific failures.
    async fn cleanup_expired(&self) -> Result<usize, StorageError>;
}

/// Constructs a storage key from owner and task identifiers.
///
/// The key format is `{owner_id}:{task_id}`. The colon separator is safe
/// because `owner_id` comes from OAuth/session tokens (no colons) and
/// `task_id` is a `UUIDv4` (no colons).
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::backend::make_key;
///
/// assert_eq!(make_key("session-abc", "task-123"), "session-abc:task-123");
/// assert_eq!(make_key("", "task-123"), ":task-123");
/// assert_eq!(make_key("owner", ""), "owner:");
/// ```
pub fn make_key(owner_id: &str, task_id: &str) -> String {
    format!("{owner_id}:{task_id}")
}

/// Parses a storage key into `(owner_id, task_id)` components.
///
/// Splits on the first colon. Returns `None` if the key does not
/// contain a colon.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::backend::parse_key;
///
/// assert_eq!(parse_key("session-abc:task-123"), Some(("session-abc", "task-123")));
/// assert_eq!(parse_key(":task-123"), Some(("", "task-123")));
/// assert_eq!(parse_key("owner:"), Some(("owner", "")));
/// assert_eq!(parse_key("no-colon"), None);
/// ```
pub fn parse_key(key: &str) -> Option<(&str, &str)> {
    key.split_once(':')
}

/// Constructs a prefix for listing all records owned by a given owner.
///
/// The prefix format is `{owner_id}:`, suitable for use with
/// [`StorageBackend::list_by_prefix`].
///
/// # Examples
///
/// ```
/// use pmcp_tasks::store::backend::make_prefix;
///
/// assert_eq!(make_prefix("session-abc"), "session-abc:");
/// assert_eq!(make_prefix(""), ":");
/// ```
pub fn make_prefix(owner_id: &str) -> String {
    format!("{owner_id}:")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- StorageError Display tests ----

    #[test]
    fn storage_error_display_not_found() {
        let err = StorageError::NotFound {
            key: "owner:task-1".to_string(),
        };
        assert_eq!(err.to_string(), "key not found: owner:task-1");
    }

    #[test]
    fn storage_error_display_version_conflict() {
        let err = StorageError::VersionConflict {
            key: "owner:task-2".to_string(),
            expected: 3,
            actual: 5,
        };
        let msg = err.to_string();
        assert!(msg.contains("owner:task-2"));
        assert!(msg.contains("expected 3"));
        assert!(msg.contains("found 5"));
    }

    #[test]
    fn storage_error_display_capacity_exceeded() {
        let err = StorageError::CapacityExceeded {
            message: "max 1000 records".to_string(),
        };
        assert_eq!(err.to_string(), "capacity exceeded: max 1000 records");
    }

    #[test]
    fn storage_error_display_backend() {
        let err = StorageError::Backend {
            message: "connection timeout".to_string(),
            source: None,
        };
        assert_eq!(err.to_string(), "backend error: connection timeout");
    }

    // ---- StorageError source() tests ----

    #[test]
    fn storage_error_source_backend_with_source() {
        let inner = std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out");
        let err = StorageError::Backend {
            message: "db failed".to_string(),
            source: Some(Box::new(inner)),
        };
        let source = std::error::Error::source(&err);
        assert!(source.is_some());
        assert!(source.unwrap().to_string().contains("timed out"));
    }

    #[test]
    fn storage_error_source_backend_without_source() {
        let err = StorageError::Backend {
            message: "unknown".to_string(),
            source: None,
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn storage_error_source_not_found_returns_none() {
        let err = StorageError::NotFound {
            key: "k".to_string(),
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn storage_error_source_version_conflict_returns_none() {
        let err = StorageError::VersionConflict {
            key: "k".to_string(),
            expected: 1,
            actual: 2,
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn storage_error_source_capacity_exceeded_returns_none() {
        let err = StorageError::CapacityExceeded {
            message: "full".to_string(),
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    // ---- Key helper tests ----

    #[test]
    fn make_key_normal() {
        assert_eq!(make_key("session-abc", "task-123"), "session-abc:task-123");
    }

    #[test]
    fn make_key_empty_owner() {
        assert_eq!(make_key("", "task-123"), ":task-123");
    }

    #[test]
    fn make_key_empty_task_id() {
        assert_eq!(make_key("owner", ""), "owner:");
    }

    #[test]
    fn make_key_both_empty() {
        assert_eq!(make_key("", ""), ":");
    }

    #[test]
    fn parse_key_normal() {
        assert_eq!(
            parse_key("session-abc:task-123"),
            Some(("session-abc", "task-123"))
        );
    }

    #[test]
    fn parse_key_empty_owner() {
        assert_eq!(parse_key(":task-123"), Some(("", "task-123")));
    }

    #[test]
    fn parse_key_empty_task_id() {
        assert_eq!(parse_key("owner:"), Some(("owner", "")));
    }

    #[test]
    fn parse_key_no_colon() {
        assert_eq!(parse_key("no-colon"), None);
    }

    #[test]
    fn parse_key_multiple_colons_splits_on_first() {
        assert_eq!(parse_key("owner:task:extra"), Some(("owner", "task:extra")));
    }

    #[test]
    fn make_prefix_normal() {
        assert_eq!(make_prefix("session-abc"), "session-abc:");
    }

    #[test]
    fn make_prefix_empty() {
        assert_eq!(make_prefix(""), ":");
    }

    // ---- VersionedRecord tests ----

    #[test]
    fn versioned_record_construction() {
        let record = VersionedRecord {
            data: b"hello".to_vec(),
            version: 42,
        };
        assert_eq!(record.data, b"hello");
        assert_eq!(record.version, 42);
    }

    #[test]
    fn versioned_record_clone() {
        let original = VersionedRecord {
            data: b"data".to_vec(),
            version: 7,
        };
        let cloned = original.clone();
        assert_eq!(cloned.data, original.data);
        assert_eq!(cloned.version, original.version);
    }

    #[test]
    fn versioned_record_debug() {
        let record = VersionedRecord {
            data: vec![],
            version: 1,
        };
        let debug = format!("{record:?}");
        assert!(debug.contains("VersionedRecord"));
        assert!(debug.contains("version: 1"));
    }

    // ---- Key round-trip tests ----

    #[test]
    fn make_key_then_parse_key_round_trip() {
        let key = make_key("owner-1", "task-abc");
        let parsed = parse_key(&key);
        assert_eq!(parsed, Some(("owner-1", "task-abc")));
    }

    #[test]
    fn make_prefix_matches_key_prefix() {
        let prefix = make_prefix("owner-1");
        let key = make_key("owner-1", "task-abc");
        assert!(key.starts_with(&prefix));
    }
}
