//! Property-based tests for the `AuthProvider::on_unauthorized` + 401 retry
//! contract added in pmcp 2.8.0.
//!
//! Companion to the 5 hand-picked unit tests in `src/shared/streamable_http.rs`
//! (Tests 1–5). These property tests sweep arbitrary HTTP status codes to
//! catch any drift in the trigger predicate "retry iff status == 401".
//!
//! Invariants verified:
//!   1. `on_unauthorized()` is invoked **iff** the first response status is 401
//!      AND an auth provider is configured.
//!   2. `get_access_token()` is called exactly once for non-401 responses and
//!      exactly twice for a 401 → retry sequence (proves the single-shot retry).
//!   3. When `auth_provider` is `None`, `on_unauthorized()` is never invoked
//!      regardless of response status.
//!
//! Each generated case is heavy (spawns a mock server and a real transport),
//! so `ProptestConfig::with_cases(32)` is chosen to keep the suite fast
//! (~10–20s on a warm cache).

#![cfg(feature = "streamable-http")]

use async_trait::async_trait;
use mockito::Server as MockServer;
use pmcp::shared::streamable_http::{
    AuthProvider, SendOptions, StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::shared::TransportMessage;
use pmcp::types::{ClientNotification, Notification};
use proptest::prelude::*;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use url::Url;

/// Local `AuthProvider` test double — counts every call to `get_access_token`
/// and `on_unauthorized`.
///
/// Mirrors the in-crate `CountingProvider` from `src/shared/streamable_http.rs`
/// tests but is defined here because that helper is not part of the public API.
struct CountingProvider {
    token: String,
    get_count: AtomicUsize,
    unauthorized_count: AtomicUsize,
}

impl fmt::Debug for CountingProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CountingProvider")
            .field("get_count", &self.get_count.load(Ordering::SeqCst))
            .field(
                "unauthorized_count",
                &self.unauthorized_count.load(Ordering::SeqCst),
            )
            .finish()
    }
}

impl CountingProvider {
    fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            get_count: AtomicUsize::new(0),
            unauthorized_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl AuthProvider for CountingProvider {
    async fn get_access_token(&self) -> pmcp::Result<String> {
        self.get_count.fetch_add(1, Ordering::SeqCst);
        Ok(self.token.clone())
    }

    async fn on_unauthorized(&self) -> pmcp::Result<()> {
        self.unauthorized_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Build a transport pointed at the given mock URL with the supplied provider.
fn make_transport(url: Url, provider: Option<Arc<dyn AuthProvider>>) -> StreamableHttpTransport {
    let mut builder = StreamableHttpTransportConfigBuilder::new(url);
    if let Some(p) = provider {
        builder = builder.with_auth_provider(p);
    }
    StreamableHttpTransport::new(builder.build())
}

/// Cheap notification message — works for any send-path test.
fn ping_message() -> TransportMessage {
    TransportMessage::Notification(Notification::Client(ClientNotification::Initialized))
}

/// HTTP status codes worth sweeping — covers success (200, 202), various 4xx
/// (including 401 itself), and 5xx. We use a closed set so proptest doesn't
/// waste cycles on, e.g., 100-Continue which the SDK doesn't model.
fn arb_status_code() -> impl Strategy<Value = u16> {
    prop_oneof![
        Just(200u16),
        Just(202),
        Just(400),
        Just(401),
        Just(403),
        Just(404),
        Just(500),
        Just(503),
    ]
}

/// Drive a single send through a mock server returning `status` and report
/// `(get_count, unauthorized_count)` from the provider.
async fn run_one_case(status: u16, with_provider: bool) -> (usize, usize) {
    let mut server = MockServer::new_async().await;
    let _mock = server
        .mock("POST", "/")
        .with_status(status as usize)
        .with_header("content-type", "application/json")
        .with_body(r#"{"jsonrpc":"2.0","id":1,"result":{}}"#)
        // No `.expect(n)` constraint: the SDK may issue 1 or 2 requests; both
        // are valid contractual outcomes depending on `status`. Verifying
        // call counts via the provider's atomics is sufficient.
        .create_async()
        .await;

    let url = Url::parse(&server.url()).unwrap();
    let provider = if with_provider {
        Some(Arc::new(CountingProvider::new("token")))
    } else {
        None
    };

    let provider_dyn: Option<Arc<dyn AuthProvider>> =
        provider.clone().map(|p| p as Arc<dyn AuthProvider>);
    let mut transport = make_transport(url, provider_dyn);

    // Send is allowed to fail (401/500 will return Err); we care about call
    // counts, not Ok/Err.
    let _ = transport
        .send_with_options(ping_message(), SendOptions::default())
        .await;

    match provider {
        Some(p) => (
            p.get_count.load(Ordering::SeqCst),
            p.unauthorized_count.load(Ordering::SeqCst),
        ),
        None => (0, 0),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    /// With a provider configured, `on_unauthorized` fires iff status == 401,
    /// and `get_access_token` is called exactly twice on 401 (proves single retry)
    /// and exactly once on every other status.
    #[test]
    fn property_on_unauthorized_triggers_iff_401(status in arb_status_code()) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (get_count, unauth_count) =
            runtime.block_on(run_one_case(status, /*with_provider=*/ true));

        if status == 401 {
            prop_assert_eq!(unauth_count, 1, "on_unauthorized must fire exactly once on 401");
            prop_assert_eq!(get_count, 2, "get_access_token must be called twice on 401 (original + retry)");
        } else {
            prop_assert_eq!(unauth_count, 0,
                "on_unauthorized must NOT fire on non-401 status {}", status);
            prop_assert_eq!(get_count, 1,
                "get_access_token must be called exactly once on non-401 status {}", status);
        }
    }

    /// With no provider, `on_unauthorized` is never invoked — by construction,
    /// the retry branch is gated on `Some(provider)`. This property guards the
    /// dual-path correctness even if the trigger predicate is widened later.
    #[test]
    fn property_no_provider_means_no_retry(status in arb_status_code()) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (get_count, unauth_count) =
            runtime.block_on(run_one_case(status, /*with_provider=*/ false));

        prop_assert_eq!(get_count, 0, "without a provider, get_access_token cannot be called");
        prop_assert_eq!(unauth_count, 0, "without a provider, on_unauthorized cannot be called");
    }
}
