//! `cargo pmcp package capture <path>` — upload a local `.pmcp` package to a
//! configured platform target (CLI-04).
//!
//! A thin authenticated client (D-04a — reuses `cargo pmcp configure`/`auth`,
//! invents NO new config): it resolves a platform target via the capture-local
//! `--target` (Codex MEDIUM — NOT a top-level `GlobalFlags` flag, so `package`
//! never clobbers `PMCP_TARGET`/AWS env), reads a cached, non-expired token from
//! the `configure`/`auth` cache (the CORRECT `TokenCacheV1.entries.get` API +
//! `is_near_expiry` check — an expired token is NEVER uploaded, Codex HIGH), and
//! POSTs the packed package with a `Bearer` header + timeout + non-2xx handling
//! via the lib-safe [`super::capture_upload`] seam. When unconfigured or the
//! token is expired it fails with actionable guidance — never a panic.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::Args;
use colored::Colorize;

use super::capture_upload::capture_upload;
use crate::commands::auth_cmd::cache::{
    default_multi_cache_path, is_near_expiry, normalize_cache_key, TokenCacheV1,
    REFRESH_WINDOW_SECS,
};
use crate::commands::GlobalFlags;

/// Arguments for `cargo pmcp package capture`.
#[derive(Debug, Args)]
pub struct CaptureArgs {
    /// Path to the AI-Package (OCI image-layout directory) to capture.
    pub path: PathBuf,
    /// Capture-local platform target selector (resolved via `resolve_target`).
    #[arg(long)]
    pub target: Option<String>,
}

/// Capture a package for a platform target.
pub async fn execute(args: CaptureArgs, global_flags: &GlobalFlags) -> Result<()> {
    // Resolve the platform target (reuse existing config — D-04a).
    let project_root = crate::commands::configure::workspace::find_workspace_root()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let target = crate::commands::configure::resolver::resolve_target(
        args.target.as_deref(),
        None,
        &project_root,
        None,
    )?
    .filter(|t| !t.is_none())
    .ok_or_else(|| {
        anyhow!("no platform target configured — run `cargo pmcp configure add <name>` first")
    })?;

    let api_url = target
        .api_url()
        .ok_or_else(|| anyhow!("target has no api_url — set it via `cargo pmcp configure`"))?;
    let api_url_str = &api_url.value;

    // Read the cached token (CORRECT `entries.get` API, Codex HIGH — NOT `cache.get`).
    let cache = TokenCacheV1::read(&default_multi_cache_path())?;
    let key = normalize_cache_key(api_url_str)?;
    let entry = cache.entries.get(&key).ok_or_else(|| {
        anyhow!("not authenticated for {api_url_str} — run `cargo pmcp auth login {api_url_str}`")
    })?;

    // Never upload an expired/near-expiry token (Codex HIGH). Transparent refresh
    // wiring is a documented follow-on.
    if is_near_expiry(entry, REFRESH_WINDOW_SECS) {
        bail!(
            "cached token for {api_url_str} is expired or about to expire — \
             run `cargo pmcp auth login {api_url_str}`"
        );
    }

    // Pack the local layout, then POST it (timeout + non-2xx handling live in the
    // lib-safe `capture_upload` seam). `entry.access_token` is interpolated ONLY
    // into the Bearer header inside `capture_upload` — never printed here.
    let package_bytes = pack_layout_to_zip(&args.path)?;
    // The per-request timeout + non-2xx handling live in the `capture_upload` seam.
    let client = reqwest::Client::new();
    capture_upload(&client, api_url_str, &entry.access_token, package_bytes).await?;

    if global_flags.should_output() {
        println!("{} captured package to {}", "✓".green().bold(), api_url_str);
    }
    Ok(())
}

/// Archive the OCI image-layout directory at `path` into an in-memory zip.
/// Validates the path is a real `.pmcp` package (an OCI layout has `index.json`)
/// before archiving — the exact upload payload format is platform-owned
/// (A1/Open-Q2, threat register `accept`).
fn pack_layout_to_zip(path: &Path) -> Result<Vec<u8>> {
    if !path.join("index.json").exists() {
        bail!(
            "{} is not an OCI image layout (.pmcp package) — missing index.json",
            path.display()
        );
    }

    let mut cursor = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut cursor);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let rel = entry
                .path()
                .strip_prefix(path)
                .context("strip package root prefix")?;
            let zip_path = rel
                .to_str()
                .ok_or_else(|| anyhow!("non-UTF-8 path in package: {}", rel.display()))?
                .replace('\\', "/");
            zip.start_file(zip_path, options)?;
            let bytes = std::fs::read(entry.path())
                .with_context(|| format!("read package file {}", entry.path().display()))?;
            zip.write_all(&bytes)?;
        }
        zip.finish().context("finalize package archive")?;
    }
    Ok(cursor.into_inner())
}
