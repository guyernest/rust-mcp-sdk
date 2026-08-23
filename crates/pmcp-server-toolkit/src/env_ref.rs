//! The ONE place the `${VAR}` / `env:VAR` reference grammar is defined for the
//! whole toolkit.
//!
//! Every path that reads an operator-supplied value which MAY be an environment
//! reference routes through [`parse_env_ref`]:
//!
//! - outgoing credentials (`[backend.auth]` api_key / bearer token / basic
//!   password / oauth2 client_secret) — `crate::http::auth`;
//! - `[code_mode] token_secret` — `crate::code_mode`;
//! - `[backend] base_url` — [`crate::config::BackendSection::resolved_base_url`].
//!
//! # Why this module is NOT feature-gated
//!
//! The function previously lived as a private helper inside `crate::http::auth`,
//! and the whole `http` module is `#[cfg(feature = "http")]`. It compiled
//! (`BackendSection` is itself http-gated), but the accompanying claim — "the
//! single chokepoint every env-reference path shares" — was reachable only in
//! `http` builds, so it was architecturally false. Packaging tooling is about to
//! build a cross-crate grammar-parity claim on top of that statement, so the
//! statement has to be structurally true rather than aspirational: this module
//! carries NO `#[cfg(feature = ...)]` gate and compiles in every feature
//! configuration.
//!
//! # Grammar
//!
//! | Input | Result | Meaning |
//! |---|---|---|
//! | `env:VAR` | `Some("VAR")` | reference |
//! | `${VAR}` | `Some("VAR")` | reference |
//! | `${}` | `Some("")` | MALFORMED reference — a reference to an empty name |
//! | `${VAR` | `None` | unterminated → a plain literal |
//! | `plain` | `None` | plain literal |
//!
//! The `${}`-means-empty rule is deliberate: the caller resolves it to the
//! empty string (omission) rather than shipping the literal `${}` to the wire.
//!
//! # Callers diverge on RESOLUTION, never on PARSING
//!
//! What a caller does with `Some(name)` is caller policy and it legitimately
//! differs — a credential resolves an unset reference to the empty string so an
//! optional credential is OMITTED, while an endpoint errors, because an empty
//! endpoint is not a degraded request but a broken one. What must NOT differ is
//! the PARSE: a second `${}` parser with slightly different edge cases is a
//! latent security bug (one parser treating `${VAR` as a reference and another
//! as a literal is exactly how a placeholder reaches the wire).
//!
//! Note that `pmcp-package` deliberately DUPLICATES this grammar rather than
//! depending on the toolkit — that crate is the workspace-excluded leaf and a
//! dependency in either direction inverts the layering. The duplication is kept
//! honest by a package-side grammar-parity table, not by a shared type.

/// The single brace/env-ref parse core shared by EVERY credential-resolution
/// path (api_key, bearer token, basic password, oauth2 client_secret).
///
/// Returns `Some(var_name)` when `raw` is a secret REFERENCE — either the
/// `"env:VAR"` or the `"${VAR}"` form — and `None` for a plain literal (which the
/// caller uses verbatim). A malformed brace reference (e.g. `"${}"`) is treated
/// as a reference to an empty name, i.e. `Some("")`, so the caller resolves it to
/// the empty string (omission) rather than shipping the literal `${}`.
///
/// This consolidates the two brace parsers that previously existed (the inline
/// `${`-strip in the old api_key resolver in `crate::http::auth` and
/// `expand_braced_var` in `crate::code_mode`): all env-reference resolution now
/// flows through this one chokepoint so the discipline cannot drift per-variant.
// Why: in a `--no-default-features` build neither `http` (credentials,
// `base_url`) nor `code-mode` (`token_secret`) is compiled, so no caller exists
// and rustc reports the function as dead. Gating the module on
// `any(feature = "http", feature = "code-mode")` is exactly the mistake this
// relocation undoes — the grammar must be defined in ONE ungated place, and its
// own unit tests still exercise it in every configuration.
#[allow(dead_code)]
pub(crate) fn parse_env_ref(raw: &str) -> Option<&str> {
    if let Some(v) = raw.strip_prefix("env:") {
        Some(v)
    } else {
        // `${...}` → the inner name (possibly empty for the malformed `${}` form).
        raw.strip_prefix("${").and_then(|s| s.strip_suffix('}'))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_env_ref;

    #[test]
    fn test_parse_env_ref_distinguishes_literal_from_reference() {
        assert_eq!(parse_env_ref("env:FOO"), Some("FOO"));
        assert_eq!(parse_env_ref("${FOO}"), Some("FOO"));
        assert_eq!(parse_env_ref("${}"), Some("")); // malformed-but-a-reference
        assert_eq!(parse_env_ref("plain"), None);
        assert_eq!(parse_env_ref("${FOO"), None); // unterminated → literal
    }
}
