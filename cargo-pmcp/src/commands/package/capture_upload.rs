//! Lib-safe HTTP-upload seam for `cargo pmcp package capture` (CLI-04).
//!
//! This is the **lib-safe leaf** (mirrors `agent_run`/`kind`): it references
//! only `reqwest` + `anyhow` + std — NO `clap`/`GlobalFlags`/`OciLayout` — so it
//! compiles in the lib target and its `mockito`-driven success/failure tests run
//! under `cargo test --lib capture_upload`, NOT only in the bin target. The bin
//! handler (`commands::package::capture`) resolves the target + token and packs
//! the package bytes, then calls [`capture_upload`].
//!
//! # Platform-coordination flag (A1 / Open-Q2)
//!
//! [`CAPTURE_PATH`] is the `{api_url}`-relative capture endpoint. The exact path
//! and payload are platform-owned (out-of-repo) — dispositioned `accept` in the
//! threat register (T-110-05-06). It ships here as a named constant with a
//! request timeout + non-2xx handling so the config/auth/HTTP contract is
//! modelled correctly; the concrete endpoint is a documented platform follow-on.

use std::time::Duration;

use anyhow::{bail, Context, Result};

/// The `{api_url}`-relative platform capture endpoint (platform-coordination
/// item, A1/Open-Q2).
pub const CAPTURE_PATH: &str = "/v1/packages/capture";

/// Per-request upload timeout (seconds).
pub const CAPTURE_TIMEOUT_SECS: u64 = 30;

/// Max bytes of a non-2xx response body echoed into the error (keeps a hostile
/// or huge error body bounded).
const MAX_ERROR_BODY_BYTES: usize = 512;

/// POST `package_bytes` to `{api_url}{CAPTURE_PATH}` with a `Bearer` header and a
/// request timeout, returning `Ok(())` on a 2xx and an actionable `Err` on any
/// non-2xx (status + a bounded slice of the response body). Never logs `token`.
pub async fn capture_upload(
    client: &reqwest::Client,
    api_url: &str,
    token: &str,
    package_bytes: Vec<u8>,
) -> Result<()> {
    let url = format!("{}{}", api_url.trim_end_matches('/'), CAPTURE_PATH);
    let resp = client
        .post(&url)
        .timeout(Duration::from_secs(CAPTURE_TIMEOUT_SECS))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/octet-stream")
        .body(package_bytes)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let bounded: String = body.chars().take(MAX_ERROR_BODY_BYTES).collect();
        bail!("package capture failed: HTTP {status} — {bounded}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2xx response → `Ok`, and the mock proves the request carried the exact
    /// `Bearer` header, the `CAPTURE_PATH`, and the package bytes.
    #[tokio::test]
    async fn capture_upload_posts_bearer_path_and_bytes_then_ok_on_2xx() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", CAPTURE_PATH)
            .match_header("authorization", "Bearer tok")
            .match_body("zip-package-bytes")
            .with_status(200)
            .with_body("{}")
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let result =
            capture_upload(&client, &server.url(), "tok", b"zip-package-bytes".to_vec()).await;

        assert!(result.is_ok(), "2xx must map to Ok: {result:?}");
        mock.assert_async().await;
    }

    /// A non-2xx response → an actionable `Err` naming the status.
    #[tokio::test]
    async fn capture_upload_errors_actionably_on_non_2xx() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", CAPTURE_PATH)
            .with_status(500)
            .with_body("upstream boom")
            .create_async()
            .await;

        let client = reqwest::Client::new();
        let err = capture_upload(&client, &server.url(), "tok", b"x".to_vec())
            .await
            .expect_err("500 must map to Err");
        let msg = err.to_string();
        assert!(
            msg.contains("500") && msg.contains("capture failed"),
            "error must name the status + be actionable: {msg}"
        );
    }
}
