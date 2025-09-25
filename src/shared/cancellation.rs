//! A runtime-agnostic cancellation token.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A token that can be used to signal cancellation of an operation.
///
/// This is a simple, runtime-agnostic implementation suitable for use in
/// code that needs to be compatible with both native `tokio` environments
/// and WASI.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    is_cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new `CancellationToken`.
    pub fn new() -> Self {
        Self {
            is_cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Cancels the token, signaling that the operation should be aborted.
    pub fn cancel(&self) {
        self.is_cancelled.store(true, Ordering::Relaxed);
    }

    /// Checks if the token has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.is_cancelled.load(Ordering::Relaxed)
    }
}

/// Extra context passed to request handlers.
///
/// This struct is runtime-agnostic and uses the shared `CancellationToken`.
#[derive(Clone, Debug)]
pub struct RequestHandlerExtra {
    /// Cancellation token for the request
    pub cancellation_token: CancellationToken,
    /// Request ID
    pub request_id: String,
    /// Session ID
    pub session_id: Option<String>,
    /// Authentication info
    pub auth_info: Option<crate::types::auth::AuthInfo>,
    /// Validated authentication context (if auth is enabled)
    #[cfg(not(target_arch = "wasm32"))]
    pub auth_context: Option<crate::server::auth::AuthContext>,
}

impl RequestHandlerExtra {
    /// Create new handler extra context.
    pub fn new(request_id: String, cancellation_token: CancellationToken) -> Self {
        Self {
            cancellation_token,
            request_id,
            session_id: None,
            auth_info: None,
            #[cfg(not(target_arch = "wasm32"))]
            auth_context: None,
        }
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// Set the auth info.
    pub fn with_auth_info(mut self, auth_info: Option<crate::types::auth::AuthInfo>) -> Self {
        self.auth_info = auth_info;
        self
    }

    /// Set the auth context.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_auth_context(
        mut self,
        auth_context: Option<crate::server::auth::AuthContext>,
    ) -> Self {
        self.auth_context = auth_context;
        self
    }

    /// Get the auth context if available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn auth_context(&self) -> Option<&crate::server::auth::AuthContext> {
        self.auth_context.as_ref()
    }

    /// Check if the request has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }
}
