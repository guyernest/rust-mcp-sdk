//! D-13 header banner emitter for target-consuming commands.
//!
//! Idempotent within a process via `OnceLock`. Field order is FIXED at api_url /
//! aws_profile / region / source — operators learn to scan known positions.
//!
//! Suppressible with `--quiet` BUT the `PMCP_TARGET` override note (Pitfall §5)
//! always fires as a safety signal.

use std::io::Write;
use std::sync::OnceLock;

use crate::commands::configure::resolver::{
    ResolvedField, ResolvedTarget, TargetSource, BANNER_FIELD_ENV_BINDINGS,
};

static BANNER_EMITTED: OnceLock<()> = OnceLock::new();

/// Emits the D-13 header banner to stderr. Idempotent within a process —
/// subsequent calls in the same process are no-ops.
///
/// `quiet=true` suppresses the banner BUT NOT the PMCP_TARGET override note.
///
/// Returns `Ok(())` regardless of whether output was actually emitted.
pub fn emit_resolved_banner_once(resolved: &ResolvedTarget, quiet: bool) -> std::io::Result<()> {
    emit_with_writer(resolved, quiet, &mut std::io::stderr())
}

/// Test seam: emit to a custom writer (used by unit tests to capture output).
///
/// The OnceLock guard still applies — call `emit_body_to_writer` instead for
/// pure body-emission tests that need to bypass the once-per-process gate.
pub fn emit_with_writer<W: Write>(
    resolved: &ResolvedTarget,
    quiet: bool,
    w: &mut W,
) -> std::io::Result<()> {
    // D-03 / Pitfall §5: PMCP_TARGET override note fires even when quiet.
    // **MED-4 fix per 77-REVIEWS.md**: format MUST match D-03 verbatim:
    //   `note: PMCP_TARGET=<env-name> overriding workspace marker (<file-name>)`
    if let Some(TargetSource::Env) = resolved.name_source {
        let marker_path = crate::commands::configure::workspace::find_workspace_root()
            .ok()
            .map(|root| {
                root.join(".pmcp")
                    .join("active-target")
                    .display()
                    .to_string()
            })
            .unwrap_or_else(|| ".pmcp/active-target".to_string());
        writeln!(
            w,
            "note: PMCP_TARGET={} overriding workspace marker ({})",
            resolved.name.as_deref().unwrap_or("<unknown>"),
            marker_path
        )?;
    }

    if quiet {
        return Ok(());
    }

    // Idempotency: only emit the body once per process.
    if BANNER_EMITTED.set(()).is_err() {
        return Ok(());
    }

    emit_body_inner(resolved, w)
}

/// Emit the banner body unconditionally (no OnceLock, no quiet check).
/// Used by tests to verify field ordering deterministically without contending
/// with the process-wide `BANNER_EMITTED` static.
pub fn emit_body_to_writer<W: Write>(resolved: &ResolvedTarget, w: &mut W) -> std::io::Result<()> {
    emit_body_inner(resolved, w)
}

fn emit_body_inner<W: Write>(resolved: &ResolvedTarget, w: &mut W) -> std::io::Result<()> {
    let name = resolved.name.as_deref().unwrap_or("<unset>");
    let kind = resolved.kind.as_deref().unwrap_or("<unset>");
    writeln!(w, "→ Using target: {} ({})", name, kind)?;
    // FIXED ORDER per D-13 — do not alphabetize.
    writeln!(w, "  api_url     = {}", display_field(resolved.api_url()))?;
    writeln!(
        w,
        "  aws_profile = {}",
        display_field(resolved.aws_profile())
    )?;
    writeln!(w, "  region      = {}", display_field(resolved.region()))?;
    // MED-4: when source is Env, look up the marker name for D-13's "(active marker = X)" text.
    let marker_name: Option<String> = if matches!(resolved.name_source, Some(TargetSource::Env)) {
        crate::commands::configure::workspace::find_workspace_root()
            .ok()
            .and_then(|root| {
                crate::commands::configure::use_cmd::read_active_marker(&root)
                    .ok()
                    .flatten()
            })
    } else {
        None
    };
    writeln!(
        w,
        "  source      = {}",
        source_description_exact(resolved.name_source, marker_name.as_deref())
    )?;
    emit_env_override_warnings(resolved, w)?;
    Ok(())
}

/// Per-field warning when an env var overrode the target's value. The aggregate
/// `source` line only describes how the target *name* was picked, so without
/// these warnings a stale env var silently misroutes deploys with no
/// operator-visible signal. Suppresses warnings for benign same-value shadows.
fn emit_env_override_warnings<W: Write>(
    resolved: &ResolvedTarget,
    w: &mut W,
) -> std::io::Result<()> {
    let target_name = resolved.name.as_deref().unwrap_or("<unset>");
    for (field, env_var) in BANNER_FIELD_ENV_BINDINGS {
        let Some(f) = resolved.fields.get(*field) else {
            continue;
        };
        if !matches!(f.source, TargetSource::Env) {
            continue;
        }
        let suffix = match (
            f.shadowing_target_value(),
            f.shadowed_target_value.as_deref(),
        ) {
            (Some(tv), _) => format!("target '{target_name}' says {tv}"),
            (None, None) => format!("target '{target_name}' has no {field} field"),
            (None, Some(_)) => continue, // benign same-value shadow
        };
        writeln!(
            w,
            "  ⚠ ENV override: {field} = {} (from ${env_var}; {suffix})",
            f.value
        )?;
    }
    Ok(())
}

fn display_field(field: Option<&ResolvedField>) -> String {
    match field {
        Some(f) => f.value.clone(),
        None => "<unset>".to_string(),
    }
}

/// D-13 exact source-line text per resolution path (MED-4 fix per 77-REVIEWS.md).
/// These strings are PRODUCT BEHAVIOR — operator-visible; copy verbatim from CONTEXT.md D-13.
/// `marker_name` indicates whether `.pmcp/active-target` is set (used to disambiguate the
/// two PMCP_TARGET-env paths: "(active marker = <name>)" vs "(no active marker)").
pub fn source_description_exact(
    name_source: Option<TargetSource>,
    marker_name: Option<&str>,
) -> String {
    match name_source {
        Some(TargetSource::Env) => match marker_name {
            Some(name) => format!("PMCP_TARGET env (active marker = {})", name),
            None => "PMCP_TARGET env (no active marker)".to_string(),
        },
        Some(TargetSource::Flag) => "--target flag".to_string(),
        Some(TargetSource::WorkspaceMarker) => {
            "~/.pmcp/config.toml + .pmcp/active-target".to_string()
        },
        // D-13 says deploy-toml-only path emits NO banner — this string would never be printed.
        // We keep it as the documented fallback for completeness.
        Some(TargetSource::Target) | Some(TargetSource::DeployToml) | None => {
            ".pmcp/deploy.toml only (no targets configured)".to_string()
        },
    }
}

/// Backwards-compat shim used by the unit tests in this module that call
/// `source_description(name_source)` without marker_name context.
/// Defers to `source_description_exact(name_source, None)`.
pub fn source_description(name_source: Option<TargetSource>) -> String {
    source_description_exact(name_source, None)
}

// =============================
// Unit tests
// =============================
#[cfg(test)]
mod tests {
    use super::*;

    fn make_resolved(name_source: Option<TargetSource>) -> ResolvedTarget {
        // HIGH-3: ResolvedTarget uses BTreeMap<String, ResolvedField> for fields.
        let mut fields = std::collections::BTreeMap::new();
        fields.insert(
            "api_url".into(),
            ResolvedField {
                value: "https://x".into(),
                source: TargetSource::Target,
                shadowed_target_value: None,
            },
        );
        fields.insert(
            "aws_profile".into(),
            ResolvedField {
                value: "p".into(),
                source: TargetSource::Target,
                shadowed_target_value: None,
            },
        );
        fields.insert(
            "region".into(),
            ResolvedField {
                value: "us-west-2".into(),
                source: TargetSource::Target,
                shadowed_target_value: None,
            },
        );
        ResolvedTarget {
            name: Some("dev".into()),
            kind: Some("pmcp-run".into()),
            fields,
            name_source,
        }
    }

    #[test]
    fn banner_field_order_fixed() {
        let resolved = make_resolved(Some(TargetSource::WorkspaceMarker));
        let mut buf: Vec<u8> = Vec::new();
        // Use emit_body_to_writer to bypass OnceLock (parallel tests share state).
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let api_pos = s.find("api_url").expect("api_url");
        let aws_pos = s.find("aws_profile").expect("aws_profile");
        let region_pos = s.find("region").expect("region");
        let source_pos = s.find("source").expect("source");
        assert!(api_pos < aws_pos, "api_url must come before aws_profile");
        assert!(aws_pos < region_pos, "aws_profile must come before region");
        assert!(region_pos < source_pos, "region must come before source");
    }

    #[test]
    fn pmcp_target_note_fires_when_source_is_env_and_quiet_true() {
        let resolved = make_resolved(Some(TargetSource::Env));
        let mut buf: Vec<u8> = Vec::new();
        // quiet=true: still emit the override note via the unconditional path
        emit_with_writer(&resolved, true, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("PMCP_TARGET=dev"), "got: {s}");
        assert!(s.contains("overriding workspace marker"), "got: {s}");
        // But NO body when quiet
        assert!(
            !s.contains("→ Using target"),
            "body must be suppressed under quiet; got: {s}"
        );
    }

    #[test]
    fn quiet_suppresses_banner_body_when_source_is_not_env() {
        let resolved = make_resolved(Some(TargetSource::WorkspaceMarker));
        let mut buf: Vec<u8> = Vec::new();
        emit_with_writer(&resolved, true, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.is_empty(),
            "quiet+marker should produce no output; got: {s:?}"
        );
    }

    #[test]
    fn source_description_workspace_marker_text() {
        let s = source_description(Some(TargetSource::WorkspaceMarker));
        assert_eq!(s, "~/.pmcp/config.toml + .pmcp/active-target");
    }

    #[test]
    fn source_description_env_text() {
        let s = source_description(Some(TargetSource::Env));
        assert!(s.contains("PMCP_TARGET"), "got: {s}");
    }

    #[test]
    fn source_description_flag_text() {
        let s = source_description(Some(TargetSource::Flag));
        assert_eq!(s, "--target flag");
    }

    // MED-4 fix per 77-REVIEWS.md: exact-format snapshot tests for D-03 / D-13 strings.
    // Drift from these strings is a UX regression — operators learn to grep for these
    // exact substrings.

    #[test]
    fn med4_source_description_env_with_marker_exact_text() {
        // D-13: "PMCP_TARGET env (active marker = <name>)"
        let s = source_description_exact(Some(TargetSource::Env), Some("dev"));
        assert_eq!(
            s, "PMCP_TARGET env (active marker = dev)",
            "MED-4: env+marker source string MUST match D-13 verbatim"
        );
    }

    #[test]
    fn med4_source_description_env_no_marker_exact_text() {
        // D-13: "PMCP_TARGET env (no active marker)"
        let s = source_description_exact(Some(TargetSource::Env), None);
        assert_eq!(
            s, "PMCP_TARGET env (no active marker)",
            "MED-4: env+no-marker source string MUST match D-13 verbatim"
        );
    }

    #[test]
    fn med4_source_description_flag_exact_text() {
        // D-13: "--target flag"
        assert_eq!(
            source_description_exact(Some(TargetSource::Flag), None),
            "--target flag"
        );
    }

    #[test]
    fn med4_source_description_workspace_marker_exact_text() {
        // D-13: "~/.pmcp/config.toml + .pmcp/active-target"
        assert_eq!(
            source_description_exact(Some(TargetSource::WorkspaceMarker), None),
            "~/.pmcp/config.toml + .pmcp/active-target"
        );
    }

    /// Replaces a field with an env-sourced value, capturing the prior (target)
    /// value as `shadowed_target_value` so the banner can compare and decide
    /// whether the override is a real conflict or a benign no-op.
    fn make_resolved_with_env_override(field: &str, env_value: &str) -> ResolvedTarget {
        let mut r = make_resolved(Some(TargetSource::WorkspaceMarker));
        let prior_target_value = r.fields.get(field).map(|f| f.value.clone());
        r.fields.insert(
            field.into(),
            ResolvedField {
                value: env_value.into(),
                source: TargetSource::Env,
                shadowed_target_value: prior_target_value,
            },
        );
        r
    }

    #[test]
    fn env_override_warning_fires_for_api_url() {
        // make_resolved fixture's target api_url = "https://x"; env value differs.
        let resolved = make_resolved_with_env_override("api_url", "https://stale.example.com");
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.contains("⚠ ENV override: api_url = https://stale.example.com (from $PMCP_API_URL"),
            "expected ENV-override warning for api_url; got: {s}"
        );
        assert!(
            s.contains("target 'dev' says https://x"),
            "warning must include shadowed target value; got: {s}"
        );
    }

    #[test]
    fn env_override_warning_fires_for_region_with_aws_region_envvar() {
        // target region = "us-west-2"; env value = "us-east-1" → real conflict.
        let resolved = make_resolved_with_env_override("region", "us-east-1");
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.contains("⚠ ENV override: region = us-east-1 (from $AWS_REGION"),
            "expected AWS_REGION warning; got: {s}"
        );
        assert!(
            s.contains("target 'dev' says us-west-2"),
            "warning must include shadowed value; got: {s}"
        );
    }

    #[test]
    fn env_override_warning_fires_for_aws_profile() {
        let resolved = make_resolved_with_env_override("aws_profile", "stale-profile");
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.contains("⚠ ENV override: aws_profile = stale-profile (from $AWS_PROFILE"),
            "expected AWS_PROFILE warning; got: {s}"
        );
    }

    #[test]
    fn env_override_warning_silent_when_no_fields_overridden() {
        let resolved = make_resolved(Some(TargetSource::WorkspaceMarker));
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            !s.contains("ENV override"),
            "no warnings expected when all fields come from target; got: {s}"
        );
    }

    #[test]
    fn env_override_warning_silent_when_env_matches_target_value() {
        // Env winner with the same value as the target — benign shadow, NOT a conflict.
        // make_resolved's region = "us-west-2"; env also says "us-west-2".
        let resolved = make_resolved_with_env_override("region", "us-west-2");
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            !s.contains("ENV override"),
            "warning must be suppressed when env value matches target value; got: {s}"
        );
    }

    #[test]
    fn env_override_warning_when_target_has_no_field_uses_no_field_phrasing() {
        // Env value with no shadowed target value — env-only override.
        let mut r = make_resolved(Some(TargetSource::WorkspaceMarker));
        r.fields.insert(
            "api_url".into(),
            ResolvedField {
                value: "https://from-env".into(),
                source: TargetSource::Env,
                shadowed_target_value: None,
            },
        );
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&r, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.contains("⚠ ENV override: api_url = https://from-env (from $PMCP_API_URL"),
            "warning should fire when target has no value; got: {s}"
        );
        assert!(
            s.contains("target 'dev' has no api_url field"),
            "warning should use 'has no <field> field' phrasing when shadowed value is None; got: {s}"
        );
    }

    #[test]
    fn env_override_warning_appears_after_source_line() {
        // Use a value that differs from target's "https://x" so the warning fires.
        let resolved = make_resolved_with_env_override("api_url", "https://different");
        let mut buf: Vec<u8> = Vec::new();
        emit_body_to_writer(&resolved, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let source_pos = s.find("source").expect("source line");
        let warn_pos = s.find("ENV override").expect("warning line");
        assert!(
            source_pos < warn_pos,
            "warning must appear AFTER the source line so it reads as an addendum"
        );
    }

    #[test]
    fn med4_d03_override_note_format() {
        // D-03 verbatim: `note: PMCP_TARGET=<env-name> overriding workspace marker (<file-name>)`
        let resolved = make_resolved(Some(TargetSource::Env));
        let mut buf: Vec<u8> = Vec::new();
        emit_with_writer(&resolved, /*quiet=*/ true, &mut buf).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(
            s.starts_with("note: PMCP_TARGET=dev overriding workspace marker ("),
            "MED-4: override note prefix must match D-03 verbatim; got: {s}"
        );
        assert!(
            s.contains(")"),
            "MED-4: override note must close the parenthesized file path"
        );
        let first_line = s.split('\n').next().unwrap();
        assert!(
            first_line.ends_with(")"),
            "MED-4: override note line must end with `)`; got: {s:?}"
        );
    }
}
