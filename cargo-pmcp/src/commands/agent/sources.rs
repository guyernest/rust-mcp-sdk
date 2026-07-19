//! Shared openai-compat completion-source construction (CLI-02/CLI-03).
//!
//! Both `agent dev --source openai-compat` and `team dev --llm` build the SAME
//! [`OpenAiCompatSource`] from an endpoint + model + env-backed key, mapping the
//! construction contract to actionable errors. This module is the single home
//! for that logic so the two command arms cannot drift.

use anyhow::{bail, Result};

use pmcp_agent::sources::{HttpSourceOptions, OpenAiCompatSource, SecretString};
use pmcp_agent::CompletionError;

/// Resolve the API key from `--api-key-env <VAR>` (env-backed, never argv);
/// default a placeholder for local unauthenticated Ollama. Never logged.
pub fn resolve_api_key(api_key_env: Option<&str>) -> SecretString {
    match api_key_env {
        Some(var) => SecretString::new(std::env::var(var).unwrap_or_default()),
        None => SecretString::new("ollama"),
    }
}

/// Build the openai-compat source, mapping the construction contract to
/// actionable errors: a remote plain-http endpoint returns
/// [`CompletionError::Decode`] (→ `--allow-insecure-http` guidance); any other
/// error is a client-build failure. `flag_hint` names the caller's endpoint flag
/// (`--endpoint` / `--llm`) so the fallback message stays context-specific.
pub fn build_openai_compat_source(
    endpoint: &str,
    model: &str,
    key: SecretString,
    allow_insecure_http: bool,
    flag_hint: &str,
) -> Result<OpenAiCompatSource> {
    let options = HttpSourceOptions {
        allow_insecure_http,
        ..Default::default()
    };
    match OpenAiCompatSource::with_options(endpoint, model, key, options) {
        Ok(source) => Ok(source),
        Err(CompletionError::Decode(_)) => bail!(
            "remote non-HTTPS endpoint {endpoint} is blocked by default — use an https:// URL \
             or pass --allow-insecure-http"
        ),
        Err(err) => {
            bail!("failed to build the completion source for {endpoint} — check {flag_hint}: {err}")
        },
    }
}
