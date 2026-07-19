//! Approval-notification channels — the outbound *notify-only* path an ask
//! takes to reach a human.
//!
//! Two channels ship here, both **notify-only** (D-10/D-11): they announce a
//! pending approval, they NEVER resolve it. Resolution ALWAYS happens
//! out-of-band via the `resolve_approval` tool from any connected client, so
//! there is exactly one resolution path and CI stays deterministic — no TTY,
//! no stdin prompting.
//!
//! - [`ConsoleChannel`] (default): prints the ask (question, options, approval
//!   id, target role) to the server log via `tracing`. No stdin, no TTY, no
//!   resolution.
//! - [`WebhookChannel`] (feature `webhook`): issues a single outgoing POST of
//!   the ask payload + approval id to an operator-configured URL, with an
//!   OPTIONAL shared-secret header. Built on a `reqwest::Client` with a BOUNDED
//!   connect + request timeout so an offline receiver can never hang the server
//!   task; a webhook failure is NON-BLOCKING (warn + proceed) because the
//!   approval is still resolvable out-of-band.
//!
//! # Security
//!
//! - **V7 / T-109-04-01 (secret non-leak):** the webhook shared secret is
//!   placed ONLY in the outgoing request header — it is NEVER passed to a
//!   `tracing`/`println` field. The warn path logs the approval id and the
//!   error kind, never the secret and never the full URL (which could embed
//!   credentials).
//! - **T-109-04-02 (egress hang):** the client's bounded timeout guarantees a
//!   notify call returns within the configured window even against an
//!   unresponsive endpoint; the returned error is non-fatal to the caller.
//! - **T-109-04-04 (SSRF):** the webhook URL is operator-configured trusted
//!   input (opt-in `webhook` feature + `--webhook-url`); SSRF hardening of the
//!   egress path is out of scope for this dev/CI-only channel.

use async_trait::async_trait;
use thiserror::Error;

/// A pending approval, as handed to a notification channel.
///
/// Carries everything a human needs to act plus the deterministic approval id
/// they will pass back to `resolve_approval`, and the OPTIONAL subject
/// reference (`subject_task_id`/`subject_ref`, D-12) linking the approval to
/// the task/component it gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalAsk {
    /// The deterministic approval id (what `resolve_approval` takes).
    pub approval_id: String,
    /// The human-facing question.
    pub question: String,
    /// The closed set of acceptable decisions.
    pub options: Vec<String>,
    /// The human role this ask targets.
    pub target_role: String,
    /// Optional linked task id (D-12), echoed verbatim.
    pub subject_task_id: Option<String>,
    /// Optional linked component/ref (D-12), echoed verbatim.
    pub subject_ref: Option<String>,
}

/// A non-fatal failure to deliver a notification.
///
/// Channels are notify-only, so the caller treats every `ChannelError` as
/// non-blocking: the approval remains resolvable via `resolve_approval`.
#[derive(Debug, Error)]
pub enum ChannelError {
    /// The outbound notification timed out (bounded by the channel's timeout).
    #[error("approval notification timed out")]
    Timeout,
    /// The outbound notification failed to send or was rejected by the receiver.
    #[error("approval notification transport failure: {0}")]
    Transport(String),
}

/// A notify-only outbound approval channel.
///
/// Implementations MUST NOT block on stdin/TTY and MUST NOT resolve the
/// approval — they only announce it.
#[async_trait]
pub trait ApprovalChannel: Send + Sync {
    /// Announce a pending approval.
    ///
    /// # Errors
    ///
    /// Returns a [`ChannelError`] when delivery fails. The caller treats this
    /// as NON-BLOCKING: the approval is still resolvable out-of-band, so a
    /// delivery failure must never make an approval unreachable.
    async fn notify(&self, ask: &ApprovalAsk) -> Result<(), ChannelError>;
}

/// The default dev channel: prints the ask to the server log.
///
/// No stdin, no TTY, no resolution (D-10). Always succeeds.
#[derive(Debug, Clone, Default)]
pub struct ConsoleChannel;

impl ConsoleChannel {
    /// Construct a console channel.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ApprovalChannel for ConsoleChannel {
    async fn notify(&self, ask: &ApprovalAsk) -> Result<(), ChannelError> {
        // Notify-only: announce and return. No stdin read, so this never blocks.
        tracing::info!(
            approval_id = %ask.approval_id,
            target_role = %ask.target_role,
            question = %ask.question,
            options = ?ask.options,
            subject_task_id = ?ask.subject_task_id,
            subject_ref = ?ask.subject_ref,
            "APPROVAL PENDING — resolve via the `resolve_approval` tool"
        );
        Ok(())
    }
}

#[cfg(feature = "webhook")]
mod webhook {
    use super::{ApprovalAsk, ApprovalChannel, ChannelError};
    use async_trait::async_trait;
    use std::time::Duration;

    /// Default bounded connect + request timeout for the webhook client.
    ///
    /// Small enough that an offline receiver cannot stall the server task, large
    /// enough for a healthy receiver on a local network.
    pub const DEFAULT_WEBHOOK_TIMEOUT: Duration = Duration::from_secs(3);

    /// The header carrying the OPTIONAL shared secret. The secret VALUE is only
    /// ever placed here — never in a log field (V7).
    pub const APPROVAL_SECRET_HEADER: &str = "x-approval-secret";

    /// A notify-only outbound webhook channel (feature `webhook`).
    ///
    /// POSTs the ask payload + approval id to an operator-configured URL with an
    /// OPTIONAL shared-secret header. The `reqwest::Client` is built with a
    /// bounded connect + request timeout; delivery failure is non-blocking.
    pub struct WebhookChannel {
        url: String,
        secret: Option<String>,
        client: reqwest::Client,
    }

    impl std::fmt::Debug for WebhookChannel {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            // Never render the secret or the raw URL (may embed credentials).
            f.debug_struct("WebhookChannel")
                .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
                .finish_non_exhaustive()
        }
    }

    impl WebhookChannel {
        /// Build a webhook channel with the default bounded timeout.
        ///
        /// # Errors
        ///
        /// Returns [`ChannelError::Transport`] if the underlying HTTP client
        /// cannot be constructed.
        pub fn new(url: impl Into<String>, secret: Option<String>) -> Result<Self, ChannelError> {
            Self::with_timeout(url, secret, DEFAULT_WEBHOOK_TIMEOUT)
        }

        /// Build a webhook channel with an explicit bounded timeout (used by
        /// tests to exercise the timeout path quickly).
        ///
        /// # Errors
        ///
        /// Returns [`ChannelError::Transport`] if the underlying HTTP client
        /// cannot be constructed.
        pub fn with_timeout(
            url: impl Into<String>,
            secret: Option<String>,
            timeout: Duration,
        ) -> Result<Self, ChannelError> {
            let client = reqwest::Client::builder()
                .connect_timeout(timeout)
                .timeout(timeout)
                .build()
                .map_err(|e| ChannelError::Transport(e.to_string()))?;
            Ok(Self {
                url: url.into(),
                secret,
                client,
            })
        }

        fn payload(ask: &ApprovalAsk) -> serde_json::Value {
            serde_json::json!({
                "approvalId": ask.approval_id,
                "question": ask.question,
                "options": ask.options,
                "targetRole": ask.target_role,
                "subjectTaskId": ask.subject_task_id,
                "subjectRef": ask.subject_ref,
            })
        }
    }

    #[async_trait]
    impl ApprovalChannel for WebhookChannel {
        async fn notify(&self, ask: &ApprovalAsk) -> Result<(), ChannelError> {
            let mut req = self.client.post(&self.url).json(&Self::payload(ask));
            if let Some(secret) = &self.secret {
                // The ONLY place the secret value is used — never logged (V7).
                req = req.header(APPROVAL_SECRET_HEADER, secret);
            }
            match req.send().await {
                Ok(resp) if resp.status().is_success() => Ok(()),
                Ok(resp) => {
                    let status = resp.status();
                    // Non-blocking: warn WITHOUT the secret or the URL, then
                    // return a non-fatal error. The approval stays resolvable.
                    tracing::warn!(
                        approval_id = %ask.approval_id,
                        status = %status,
                        "approval webhook rejected the notification (non-blocking; resolve out-of-band)"
                    );
                    Err(ChannelError::Transport(format!("webhook status {status}")))
                },
                Err(e) if e.is_timeout() => {
                    tracing::warn!(
                        approval_id = %ask.approval_id,
                        "approval webhook timed out (non-blocking; resolve out-of-band)"
                    );
                    Err(ChannelError::Timeout)
                },
                Err(e) => {
                    tracing::warn!(
                        approval_id = %ask.approval_id,
                        error = %e,
                        "approval webhook failed to send (non-blocking; resolve out-of-band)"
                    );
                    Err(ChannelError::Transport(e.to_string()))
                },
            }
        }
    }
}

#[cfg(feature = "webhook")]
pub use webhook::{WebhookChannel, APPROVAL_SECRET_HEADER, DEFAULT_WEBHOOK_TIMEOUT};

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ask() -> ApprovalAsk {
        ApprovalAsk {
            approval_id: "appr-001".to_string(),
            question: "Ship v2.4?".to_string(),
            options: vec!["approve".to_string(), "reject".to_string()],
            target_role: "release-manager".to_string(),
            subject_task_id: Some("task-42".to_string()),
            subject_ref: None,
        }
    }

    #[tokio::test]
    async fn console_notify_is_ok_and_does_not_block() {
        let channel = ConsoleChannel::new();
        // A stdin-reading impl would hang here under a non-interactive test.
        let out = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            channel.notify(&sample_ask()),
        )
        .await
        .expect("console notify must not block");
        assert!(out.is_ok());
    }

    #[cfg(feature = "webhook")]
    mod webhook_tests {
        use super::super::{ApprovalChannel, WebhookChannel, APPROVAL_SECRET_HEADER};
        use super::sample_ask;
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        /// Read one HTTP request (headers + Content-Length body) from a stream.
        async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let n = stream.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf);
                if let Some(hdr_end) = text.find("\r\n\r\n") {
                    // Determine the declared body length and read until we have it.
                    let content_len = text
                        .lines()
                        .find_map(|l| {
                            let l = l.to_ascii_lowercase();
                            l.strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                        })
                        .unwrap_or(0);
                    let have_body = buf.len() - (hdr_end + 4);
                    if have_body >= content_len {
                        break;
                    }
                }
            }
            String::from_utf8_lossy(&buf).into_owned()
        }

        #[tokio::test]
        async fn webhook_posts_payload_and_secret_header() {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let raw = read_request(&mut stream).await;
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
                raw
            });

            let channel = WebhookChannel::with_timeout(
                format!("http://{addr}/approvals"),
                Some("shhh-secret".to_string()),
                Duration::from_secs(2),
            )
            .unwrap();
            channel.notify(&sample_ask()).await.expect("notify ok");

            let raw = server.await.unwrap();
            let lower = raw.to_ascii_lowercase();
            assert!(
                lower.contains(APPROVAL_SECRET_HEADER),
                "secret header must be present"
            );
            assert!(raw.contains("shhh-secret"), "secret value sent in header");
            assert!(
                raw.contains("appr-001"),
                "payload must carry the approval id"
            );
            assert!(
                raw.contains("Ship v2.4?"),
                "payload must carry the question"
            );
        }

        #[tokio::test]
        async fn webhook_omits_header_when_no_secret() {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();

            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let raw = read_request(&mut stream).await;
                stream
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                    .await
                    .unwrap();
                raw
            });

            let channel = WebhookChannel::with_timeout(
                format!("http://{addr}/a"),
                None,
                Duration::from_secs(2),
            )
            .unwrap();
            channel.notify(&sample_ask()).await.expect("notify ok");

            let raw = server.await.unwrap().to_ascii_lowercase();
            assert!(
                !raw.contains(APPROVAL_SECRET_HEADER),
                "no secret header when unconfigured"
            );
        }

        #[tokio::test]
        async fn webhook_timeout_is_non_blocking_within_bound() {
            // Bind but never respond: the connection is accepted by the kernel
            // backlog, so the request timeout (not connect) fires.
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            // Hold the accepted socket open without replying.
            let _guard = tokio::spawn(async move {
                if let Ok((stream, _)) = listener.accept().await {
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    drop(stream);
                }
            });

            let channel = WebhookChannel::with_timeout(
                format!("http://{addr}/slow"),
                None,
                Duration::from_millis(150),
            )
            .unwrap();

            let started = std::time::Instant::now();
            let out = channel.notify(&sample_ask()).await;
            let elapsed = started.elapsed();

            assert!(out.is_err(), "an unresponsive endpoint must yield an error");
            assert!(
                elapsed < Duration::from_secs(2),
                "failure must return within the bounded window, took {elapsed:?}"
            );
        }
    }
}
