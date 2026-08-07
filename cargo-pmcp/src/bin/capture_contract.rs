//! `capture_contract` — shared introspect + extract-subset tool for the
//! pmcp.run package-capture GraphQL contract (Task 2 of the package-capture
//! contract seam).
//!
//! Two modes:
//! - `capture_contract emit` — introspects the live **source data API**
//!   (the amplifyData API behind `api_url`, NOT the merged
//!   `PMCP_RUN_GRAPHQL_URL`/`DEFAULT_GRAPHQL_URL`), extracts just the
//!   capture-subset (`submitPackageCapture` + `getPackageCaptureStatus` and
//!   their return types), and prints the resulting SDL to stdout.
//! - `capture_contract check <path>` — does the same live introspection,
//!   then diffs the extracted SDL against the vendored contract file at
//!   `<path>` (after stripping its provenance header comment), exiting
//!   non-zero on drift.
//!
//! Deliberately kept as a `cargo-pmcp`-subcommand-free internal binary
//! (`src/bin/capture_contract.rs`) rather than a public CLI subcommand. It is
//! a MANUAL dev/introspection aid, run by someone with source-API access
//! (M2M `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` credentials) — it is NOT
//! invoked by SDK CI, which cannot reach the source `amplifyData` API (see
//! `docs/superpowers/plans/2026-07-20-package-capture-contract-seam.md` Task 4:
//! the online drift check is platform-owned). Because end users installing
//! `cargo-pmcp` have no use for this tool, it requires the non-default
//! `capture-contract-tool` feature (see the `[[bin]]` entry in `Cargo.toml`),
//! so a default `cargo install cargo-pmcp` does not build or install it.
//!
//! ## Why this does NOT call `auth::get_credentials()` directly
//!
//! The brief for this tool describes reusing
//! `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs`'s `get_credentials()`
//! M2M (`client_credentials`) path. In practice this isn't reachable from a
//! `src/bin/*.rs` binary: those link only the `cargo_pmcp` **library** crate
//! (see `src/lib.rs`), and `auth.rs` lives in the bin-only module tree
//! declared in `src/main.rs`. Worse, `get_credentials()`'s config-discovery
//! fallback (`configured_api_base_url()`) transitively needs
//! `commands::configure::resolver`, which the Phase 77 HIGH-1 fix
//! (`.planning/phases/77-cargo-pmcp-configure-commands/77-03-PLAN.md`,
//! `commands/configure/name_validation.rs` doc comment) deliberately keeps
//! bin-only — `commands::*` is never mounted into the lib surface, and
//! integration tests that need real CLI behavior invoke the built binary as
//! a subprocess instead. Pulling that whole tree into the lib crate for this
//! one M2M call would violate that documented boundary.
//!
//! So this file reimplements just the M2M branch directly against
//! `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` (mirroring
//! `auth::get_credentials_via_client_credentials`'s discovery + token
//! exchange), self-contained, with no dependency on the bin-only `deployment`
//! module tree. It DOES depend on the `cargo_pmcp` lib crate's
//! `#[doc(hidden)] pmcp_run_graphql` seam for the pure SDL-extraction helpers
//! (`extract_capture_sdl`, `assert_capture_ops_present`,
//! `strip_provenance_header`), which live there so their unit tests run
//! under the default `cargo test -p cargo-pmcp`.

use anyhow::{Context, Result};
use cargo_pmcp::pmcp_run_graphql::{
    assert_capture_ops_present, extract_capture_sdl, strip_provenance_header,
};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

/// `capture_contract <command>`
#[derive(Debug, Parser)]
#[command(name = "capture_contract")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Introspect the live source data API and print the capture-subset SDL.
    Emit,
    /// Introspect live, extract the capture-subset SDL, and diff it against
    /// the SDL file at `<path>`. Exits non-zero on drift.
    Check {
        /// Path to the vendored contract SDL (e.g. `contracts/pmcp-run/capture-v1.graphql`).
        path: PathBuf,
    },
}

/// The standard full `IntrospectionQuery` document (introspects `__schema`:
/// query/mutation root type names, all `types[]` with fields/args/enum
/// values, and directives). Sent as-is to the source data API.
const INTROSPECTION_QUERY: &str = r#"
query IntrospectionQuery {
  __schema {
    queryType { name }
    mutationType { name }
    subscriptionType { name }
    types {
      ...FullType
    }
    directives {
      name
      description
      locations
      args {
        ...InputValue
      }
    }
  }
}

fragment FullType on __Type {
  kind
  name
  description
  fields(includeDeprecated: true) {
    name
    description
    args {
      ...InputValue
    }
    type {
      ...TypeRef
    }
    isDeprecated
    deprecationReason
  }
  inputFields {
    ...InputValue
  }
  interfaces {
    ...TypeRef
  }
  enumValues(includeDeprecated: true) {
    name
    description
    isDeprecated
    deprecationReason
  }
  possibleTypes {
    ...TypeRef
  }
}

fragment InputValue on __InputValue {
  name
  description
  type { ...TypeRef }
  defaultValue
}

fragment TypeRef on __Type {
  kind
  name
  ofType {
    kind
    name
    ofType {
      kind
      name
      ofType {
        kind
        name
        ofType {
          kind
          name
          ofType {
            kind
            name
            ofType {
              kind
              name
              ofType {
                kind
                name
              }
            }
          }
        }
      }
    }
  }
}
"#;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Emit => {
            let sdl = introspect_and_extract().await?;
            print!("{sdl}");
        },
        Command::Check { path } => {
            let live_sdl = introspect_and_extract().await?;
            let vendored_raw = std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            let vendored_body = strip_provenance_header(&vendored_raw);

            if normalize_body(&vendored_body) == normalize_body(&live_sdl) {
                println!("OK: {} matches the live source schema", path.display());
            } else {
                eprintln!(
                    "DRIFT DETECTED: {} no longer matches the live source schema",
                    path.display()
                );
                print_line_diff(&vendored_body, &live_sdl);
                std::process::exit(1);
            }
        },
    }
    Ok(())
}

// ============================================================================
// Live path: auth + introspection + endpoint guard + extraction
// ============================================================================

/// Full live flow: resolve the source endpoint, get an M2M token, introspect,
/// assert it's the right endpoint, and extract the capture-subset SDL.
async fn introspect_and_extract() -> Result<String> {
    let source_url = resolve_source_url()?;
    let token = get_m2m_token().await?;
    let schema_data = introspect_source_schema(&source_url, &token).await?;
    assert_capture_ops_present(&schema_data, &source_url)?;
    extract_capture_sdl(&schema_data)
}

/// Read `PMCP_SOURCE_GRAPHQL_URL` — the source data API (amplifyData) behind
/// `api_url`, distinct from the merged `PMCP_RUN_GRAPHQL_URL`/`DEFAULT_GRAPHQL_URL`.
fn resolve_source_url() -> Result<String> {
    std::env::var("PMCP_SOURCE_GRAPHQL_URL").context(
        "PMCP_SOURCE_GRAPHQL_URL is not set — required: the source data API (amplifyData) \
         endpoint behind api_url, distinct from the merged DEFAULT_GRAPHQL_URL",
    )
}

/// Fetch an M2M access token via the OAuth2 `client_credentials` grant.
/// Mirrors `auth::get_credentials_via_client_credentials` (see module docs
/// above for why this isn't a direct call).
async fn get_m2m_token() -> Result<String> {
    let client_id = std::env::var("PMCP_CLIENT_ID")
        .context("PMCP_CLIENT_ID is not set — required for M2M auth (client_credentials)")?;
    let client_secret = std::env::var("PMCP_CLIENT_SECRET")
        .context("PMCP_CLIENT_SECRET is not set — required for M2M auth (client_credentials)")?;

    let cognito_domain = resolve_cognito_domain().await?;
    let token_url = format!("https://{cognito_domain}/oauth2/token");

    let client = reqwest::Client::new();
    let response = client
        .post(&token_url)
        .basic_auth(&client_id, Some(&client_secret))
        .form(&[("grant_type", "client_credentials")])
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .context("failed to request access token via client_credentials")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("client_credentials token request failed: {status}\n{body}");
    }

    #[derive(serde::Deserialize)]
    struct TokenResponse {
        access_token: String,
    }

    let token: TokenResponse = response
        .json()
        .await
        .context("failed to parse client_credentials token response")?;
    Ok(token.access_token)
}

/// Resolve the Cognito auth domain: `PMCP_RUN_COGNITO_DOMAIN` override, else
/// discovery via `.well-known/pmcp-config` at `PMCP_API_URL`/`PMCP_RUN_API_URL`
/// (default `https://api.pmcp.run`).
async fn resolve_cognito_domain() -> Result<String> {
    if let Ok(domain) = std::env::var("PMCP_RUN_COGNITO_DOMAIN") {
        let trimmed = domain.trim();
        if !trimmed.is_empty() {
            return Ok(strip_scheme(trimmed));
        }
    }

    let api_url = std::env::var("PMCP_API_URL")
        .or_else(|_| std::env::var("PMCP_RUN_API_URL"))
        .unwrap_or_else(|_| "https://api.pmcp.run".to_string());
    let discovery_url = format!("{}/.well-known/pmcp-config", api_url.trim_end_matches('/'));

    let client = reqwest::Client::new();
    let response = client
        .get(&discovery_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("failed to reach discovery endpoint {discovery_url}"))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "discovery endpoint {discovery_url} returned status {}",
            response.status()
        );
    }

    #[derive(serde::Deserialize)]
    struct Discovery {
        cognito_domain: String,
    }

    let discovered: Discovery = response
        .json()
        .await
        .context("failed to parse pmcp-config discovery response")?;
    Ok(strip_scheme(&discovered.cognito_domain))
}

/// Strip a leading `https://`/`http://` scheme and trailing `/`.
fn strip_scheme(s: &str) -> String {
    s.strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s)
        .trim_end_matches('/')
        .to_string()
}

/// POST the standard introspection query to the source data API with the M2M
/// bearer token. Returns the `data` object of the GraphQL response (i.e. the
/// value carrying the `__schema` key), NOT `data.__schema` directly — matches
/// what [`extract_capture_sdl`] expects.
async fn introspect_source_schema(source_url: &str, token: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "query": INTROSPECTION_QUERY,
        "variables": {},
    });

    let response = client
        .post(source_url)
        .bearer_auth(token)
        .json(&body)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .with_context(|| format!("failed to POST introspection query to {source_url}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        anyhow::bail!("introspection request to {source_url} failed: {status}\n{text}");
    }

    let raw: Value = response
        .json()
        .await
        .context("failed to parse introspection response as JSON")?;

    if let Some(errors) = raw.get("errors") {
        anyhow::bail!("GraphQL errors introspecting {source_url}: {errors}");
    }

    raw.get("data")
        .cloned()
        .with_context(|| format!("introspection response from {source_url} has no `data` field"))
}

// ============================================================================
// `check` mode helpers
// ============================================================================
//
// NOTE: `assert_capture_ops_present`, `extract_capture_sdl`, and
// `strip_provenance_header` moved to
// `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` (the
// already-dual-mounted shared leaf) along with their pure helpers
// (`find_type`, `find_field`, `render_type_ref`, `unwrap_named_type`,
// `render_args`, `render_object_type`) and their unit tests, so those tests
// run under the default `cargo test -p cargo-pmcp` even though this binary
// is now feature-gated behind `capture-contract-tool`. Imported above via
// `cargo_pmcp::pmcp_run_graphql::*`.

/// Normalize an SDL body for comparison: trim trailing whitespace per line,
/// trim leading/trailing blank lines. Still a plain string/line comparison —
/// no GraphQL parsing.
fn normalize_body(s: &str) -> String {
    s.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Print a line-level unified-ish diff between the vendored and live SDL
/// bodies (`-` vendored-only/changed, `+` live-only/changed).
fn print_line_diff(vendored: &str, live: &str) {
    let vendored_norm = normalize_body(vendored);
    let live_norm = normalize_body(live);
    let vendored_lines: Vec<&str> = vendored_norm.lines().collect();
    let live_lines: Vec<&str> = live_norm.lines().collect();
    let max = vendored_lines.len().max(live_lines.len());
    for i in 0..max {
        let v = vendored_lines.get(i).copied();
        let l = live_lines.get(i).copied();
        match (v, l) {
            (Some(a), Some(b)) if a == b => {},
            (Some(a), Some(b)) => {
                println!("- {a}");
                println!("+ {b}");
            },
            (Some(a), None) => println!("- {a}"),
            (None, Some(b)) => println!("+ {b}"),
            (None, None) => {},
        }
    }
}

// ============================================================================
// Tests
// ============================================================================
//
// The unit tests that used to live here (covering `extract_capture_sdl` /
// `assert_capture_ops_present` / `strip_provenance_header`) moved along with
// those functions to
// `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` — see the
// note above `check`-mode helpers. This keeps that coverage running under
// the default `cargo test -p cargo-pmcp` (lib target) even though this
// binary itself now requires the non-default `capture-contract-tool`
// feature.
