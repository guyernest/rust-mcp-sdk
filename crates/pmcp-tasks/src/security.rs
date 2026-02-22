//! Security configuration and owner resolution for MCP Tasks.
//!
//! This module provides [`TaskSecurityConfig`] for configuring owner-specific
//! security limits (max tasks per owner, anonymous access control), and the
//! [`resolve_owner_id`] function that determines the owner identity from
//! available authentication sources.
//!
//! # Owner Resolution Priority
//!
//! The [`resolve_owner_id`] function uses this priority chain:
//! 1. OAuth subject (highest priority)
//! 2. Client ID
//! 3. Session ID
//! 4. [`DEFAULT_LOCAL_OWNER`] ("local") -- used for single-user servers
//!
//! # Security Model
//!
//! Owner isolation is structural: every task operation receives an `owner_id`
//! parameter, and the store enforces that callers can only access their own
//! tasks. On owner mismatch, the store returns `NotFound` (never revealing
//! that a task exists but belongs to someone else).

/// Default owner ID used when no authentication is configured.
///
/// All tasks for local single-user servers (no OAuth) belong to this owner,
/// keeping code paths consistent regardless of auth configuration.
///
/// # Examples
///
/// ```
/// use pmcp_tasks::security::DEFAULT_LOCAL_OWNER;
///
/// assert_eq!(DEFAULT_LOCAL_OWNER, "local");
/// ```
pub const DEFAULT_LOCAL_OWNER: &str = "local";

/// Security configuration for owner-specific task limits.
///
/// This struct holds security-related configuration that applies per-owner
/// (as opposed to [`StoreConfig`](crate::store::StoreConfig) which holds
/// storage-level limits like variable size and TTL).
///
/// # Defaults
///
/// | Setting               | Default | Description                              |
/// |-----------------------|---------|------------------------------------------|
/// | `max_tasks_per_owner` | 100     | Maximum active tasks per owner           |
/// | `allow_anonymous`     | false   | Whether anonymous/local access is allowed|
///
/// # Examples
///
/// ```
/// use pmcp_tasks::security::TaskSecurityConfig;
///
/// // Use defaults
/// let config = TaskSecurityConfig::default();
/// assert_eq!(config.max_tasks_per_owner, 100);
/// assert!(!config.allow_anonymous);
///
/// // Use builder methods
/// let config = TaskSecurityConfig::default()
///     .with_max_tasks_per_owner(50)
///     .with_allow_anonymous(true);
/// assert_eq!(config.max_tasks_per_owner, 50);
/// assert!(config.allow_anonymous);
/// ```
#[derive(Debug, Clone)]
pub struct TaskSecurityConfig {
    /// Maximum number of active tasks a single owner can have.
    ///
    /// When this limit is reached, `create()` returns
    /// [`TaskError::ResourceExhausted`](crate::error::TaskError::ResourceExhausted)
    /// with a suggestion to cancel or wait for existing tasks to expire.
    /// There is no auto-eviction -- the limit is a hard reject.
    pub max_tasks_per_owner: usize,

    /// Whether anonymous (unauthenticated) access is allowed.
    ///
    /// When `false`, task operations with an empty owner ID or the
    /// [`DEFAULT_LOCAL_OWNER`] value are rejected. Set to `true` for
    /// local single-user servers that operate without OAuth.
    pub allow_anonymous: bool,
}

impl Default for TaskSecurityConfig {
    fn default() -> Self {
        Self {
            max_tasks_per_owner: 100,
            allow_anonymous: false,
        }
    }
}

impl TaskSecurityConfig {
    /// Sets the maximum number of tasks per owner.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::security::TaskSecurityConfig;
    ///
    /// let config = TaskSecurityConfig::default()
    ///     .with_max_tasks_per_owner(50);
    /// assert_eq!(config.max_tasks_per_owner, 50);
    /// ```
    pub fn with_max_tasks_per_owner(mut self, max: usize) -> Self {
        self.max_tasks_per_owner = max;
        self
    }

    /// Sets whether anonymous access is allowed.
    ///
    /// # Examples
    ///
    /// ```
    /// use pmcp_tasks::security::TaskSecurityConfig;
    ///
    /// let config = TaskSecurityConfig::default()
    ///     .with_allow_anonymous(true);
    /// assert!(config.allow_anonymous);
    /// ```
    pub fn with_allow_anonymous(mut self, allow: bool) -> Self {
        self.allow_anonymous = allow;
        self
    }
}

/// Resolves the owner ID from available identity sources.
///
/// Uses a priority chain to determine the owner identity:
/// 1. `auth_subject` -- OAuth subject claim (highest priority)
/// 2. `client_id` -- OAuth client ID
/// 3. `session_id` -- Transport session ID
/// 4. [`DEFAULT_LOCAL_OWNER`] -- fallback for unauthenticated access
///
/// Empty strings are treated as absent and skipped in the priority chain.
///
/// # Arguments
///
/// * `auth_subject` - The OAuth subject claim (`sub`), if available.
/// * `client_id` - The OAuth client ID, if available.
/// * `session_id` - The transport session ID, if available.
///
/// # Design Note
///
/// This function takes `&str` slices rather than an `AuthContext` reference
/// to avoid coupling `pmcp-tasks` to the `pmcp` crate's auth types. Phase 3
/// middleware will bridge the two:
/// ```text
/// resolve_owner_id(Some(&auth.subject), auth.client_id.as_deref(), session_id)
/// ```
///
/// # Examples
///
/// ```
/// use pmcp_tasks::security::{resolve_owner_id, DEFAULT_LOCAL_OWNER};
///
/// // OAuth subject takes highest priority
/// let owner = resolve_owner_id(Some("user-123"), Some("client-abc"), Some("sess-1"));
/// assert_eq!(owner, "user-123");
///
/// // Falls through to client_id when subject is absent
/// let owner = resolve_owner_id(None, Some("client-abc"), Some("sess-1"));
/// assert_eq!(owner, "client-abc");
///
/// // Falls through to session_id when both are absent
/// let owner = resolve_owner_id(None, None, Some("sess-1"));
/// assert_eq!(owner, "sess-1");
///
/// // Falls back to DEFAULT_LOCAL_OWNER when all are absent
/// let owner = resolve_owner_id(None, None, None);
/// assert_eq!(owner, DEFAULT_LOCAL_OWNER);
/// ```
pub fn resolve_owner_id(
    auth_subject: Option<&str>,
    client_id: Option<&str>,
    session_id: Option<&str>,
) -> String {
    if let Some(subject) = auth_subject {
        if !subject.is_empty() {
            return subject.to_string();
        }
    }

    if let Some(cid) = client_id {
        if !cid.is_empty() {
            return cid.to_string();
        }
    }

    if let Some(sid) = session_id {
        if !sid.is_empty() {
            return sid.to_string();
        }
    }

    DEFAULT_LOCAL_OWNER.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- TaskSecurityConfig tests ---

    #[test]
    fn default_config_has_expected_values() {
        let config = TaskSecurityConfig::default();
        assert_eq!(config.max_tasks_per_owner, 100);
        assert!(!config.allow_anonymous);
    }

    #[test]
    fn builder_sets_max_tasks_per_owner() {
        let config = TaskSecurityConfig::default().with_max_tasks_per_owner(50);
        assert_eq!(config.max_tasks_per_owner, 50);
        assert!(!config.allow_anonymous); // unchanged
    }

    #[test]
    fn builder_sets_allow_anonymous() {
        let config = TaskSecurityConfig::default().with_allow_anonymous(true);
        assert!(config.allow_anonymous);
        assert_eq!(config.max_tasks_per_owner, 100); // unchanged
    }

    #[test]
    fn builder_chains_both_settings() {
        let config = TaskSecurityConfig::default()
            .with_max_tasks_per_owner(25)
            .with_allow_anonymous(true);
        assert_eq!(config.max_tasks_per_owner, 25);
        assert!(config.allow_anonymous);
    }

    #[test]
    fn config_clone() {
        let config = TaskSecurityConfig::default().with_max_tasks_per_owner(42);
        let cloned = config.clone();
        assert_eq!(cloned.max_tasks_per_owner, 42);
    }

    #[test]
    fn config_debug() {
        let config = TaskSecurityConfig::default();
        let debug = format!("{config:?}");
        assert!(debug.contains("TaskSecurityConfig"));
        assert!(debug.contains("100"));
    }

    // --- DEFAULT_LOCAL_OWNER tests ---

    #[test]
    fn default_local_owner_is_local() {
        assert_eq!(DEFAULT_LOCAL_OWNER, "local");
    }

    // --- resolve_owner_id tests ---

    #[test]
    fn resolve_prefers_auth_subject() {
        let owner = resolve_owner_id(Some("user-123"), Some("client-abc"), Some("sess-1"));
        assert_eq!(owner, "user-123");
    }

    #[test]
    fn resolve_falls_to_client_id_when_subject_absent() {
        let owner = resolve_owner_id(None, Some("client-abc"), Some("sess-1"));
        assert_eq!(owner, "client-abc");
    }

    #[test]
    fn resolve_falls_to_client_id_when_subject_empty() {
        let owner = resolve_owner_id(Some(""), Some("client-abc"), Some("sess-1"));
        assert_eq!(owner, "client-abc");
    }

    #[test]
    fn resolve_falls_to_session_id_when_subject_and_client_absent() {
        let owner = resolve_owner_id(None, None, Some("sess-1"));
        assert_eq!(owner, "sess-1");
    }

    #[test]
    fn resolve_falls_to_session_id_when_subject_and_client_empty() {
        let owner = resolve_owner_id(Some(""), Some(""), Some("sess-1"));
        assert_eq!(owner, "sess-1");
    }

    #[test]
    fn resolve_falls_to_default_when_all_absent() {
        let owner = resolve_owner_id(None, None, None);
        assert_eq!(owner, DEFAULT_LOCAL_OWNER);
    }

    #[test]
    fn resolve_falls_to_default_when_all_empty() {
        let owner = resolve_owner_id(Some(""), Some(""), Some(""));
        assert_eq!(owner, DEFAULT_LOCAL_OWNER);
    }

    #[test]
    fn resolve_ignores_later_priorities_when_subject_present() {
        // Even with empty client_id and session_id, subject wins
        let owner = resolve_owner_id(Some("user-x"), None, None);
        assert_eq!(owner, "user-x");
    }

    #[test]
    fn resolve_ignores_later_priorities_when_client_id_present() {
        let owner = resolve_owner_id(None, Some("client-y"), None);
        assert_eq!(owner, "client-y");
    }
}
