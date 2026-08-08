//! The expected-difference baseline between MCP 2025-11-25 (v1) and
//! MCP 2026-07-28 (v2): its data model, its reader, and nothing else.
//!
//! # What this module is for
//!
//! v2 legitimately differs from v1 BY DESIGN — no `initialize`, no `tasks/list`,
//! `resultType` added, caching hints REQUIRED rather than optional — so a naive
//! dual-run diff of two `TestReport`s is pure noise. Encoding the KNOWN deltas
//! is what turns "the two runs differ" into "the two runs differ IN A WAY WE DID
//! NOT EXPECT", which is the only interesting signal. The encoding lives in
//! `crates/mcp-tester/baselines/era-deltas.yaml`, is reviewable as a spec
//! artifact by someone who does not read Rust, and is a direct input to
//! Phase 118's conformance work.
//!
//! # Why a NEW TOP-LEVEL MODULE (the A-D11 rule)
//!
//! This module follows the precedent set by [`crate::post_deploy_report`], whose
//! own header argues the case verbatim ("Why a new struct (vs. extending
//! `TestReport`)") and states the additivity rule: additive field changes on a
//! NEW type do not disturb existing consumers.
//!
//! Here that rule is not a style preference, it is a build constraint, and it is
//! ABSOLUTE. `cargo-pmcp` links `mcp-tester` as a LIBRARY, not as a JSON
//! producer:
//!
//! * `cargo-pmcp/src/commands/test/apps.rs:874-880` builds a
//!   [`crate::TestResult`] as an EXHAUSTIVE POSITIONAL STRUCT LITERAL, so a new
//!   field on `TestResult` is a hard compile break;
//! * `cargo-pmcp/src/commands/test/conformance.rs:276-289` matches
//!   [`crate::TestCategory`] with NO `_` arm, so a new variant is a hard compile
//!   break;
//! * `ServerTester::new` has five call sites in `cargo-pmcp`, so widening its
//!   arity is a hard compile break.
//!
//! Therefore NOTHING in this module adds a field to `TestResult`, a variant to
//! `TestCategory` or `TestStatus`, or a positional argument to
//! `ServerTester::new`. It defines its own types and reads its own file.
//!
//! # Scope
//!
//! Data model plus reader ONLY. The dual-run comparison itself (and the report
//! type that carries its verdict) is deliberately NOT here — it is owned by a
//! later plan, which joins its observed differences against
//! [`EraDelta::observation_id`].

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// One expected v1-vs-v2 difference: a difference that is CORRECT BY DESIGN.
///
/// Field semantics are documented in the baseline file's own header, which is
/// the reviewer-facing copy of this contract.
///
/// `note` and `provisional` carry `#[serde(default)]` so the schema stays
/// forward-compatible: a future optional field can be added without invalidating
/// every checked-in baseline, exactly as
/// [`crate::post_deploy_report::PostDeployReport`] does for its optional fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraDelta {
    /// Stable human-facing label, `ERA-NN`. Unique across the baseline.
    pub id: String,

    /// The STABLE, MACHINE-FACING name of the wire fact this entry is about —
    /// namespaced, lowercase, dot-separated (`method.initialize`,
    /// `header.mcp_session_id`, `result.cache_scope`, …).
    ///
    /// This is the JOIN KEY a dual-run comparison diffs on. It is REQUIRED (no
    /// `serde(default)`) and must be unique: a missing or duplicated value
    /// silently merges two distinct wire facts. It is NOT a human-facing test
    /// name and must never be renamed for readability.
    ///
    /// It exists because [`crate::TestResult`] carries only
    /// `{name, category, status, duration, error, details}` — no header, no
    /// session id, no result-envelope key, no HTTP status — so a comparison
    /// keyed on test names could not observe most of the baseline's entries.
    pub observation_id: String,

    /// Human-readable wire surface the entry concerns.
    pub subject: String,

    /// What MCP 2025-11-25 does.
    pub v1: String,

    /// What MCP 2026-07-28 does.
    pub v2: String,

    /// Difference class, for grouping in a report.
    pub kind: String,

    /// Citation a reviewer can check without reading Rust.
    pub source: String,

    /// Optional prose. Required in practice on provisional entries, where it
    /// names the phase that owns the entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,

    /// `true` when the owning phase is not signed off, so a change there
    /// produces a legible baseline edit rather than a mystery test failure.
    #[serde(default)]
    pub provisional: bool,
}

/// The whole checked-in baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EraBaseline {
    /// Wire-format version of THIS file's schema. Currently `1`.
    pub schema_version: u32,

    /// The v1 protocol version the baseline was written against.
    pub v1_protocol: String,

    /// The v2 protocol version the baseline was written against.
    pub v2_protocol: String,

    /// The expected differences.
    pub deltas: Vec<EraDelta>,
}

impl EraBaseline {
    /// Look an entry up by its stable [`EraDelta::observation_id`].
    pub fn find_by_observation_id(&self, observation_id: &str) -> Option<&EraDelta> {
        self.deltas
            .iter()
            .find(|d| d.observation_id == observation_id)
    }

    /// Every [`EraDelta::observation_id`] in file order.
    pub fn observation_ids(&self) -> Vec<&str> {
        self.deltas
            .iter()
            .map(|d| d.observation_id.as_str())
            .collect()
    }
}

/// Path of the baseline shipped with this crate.
///
/// Derived from `CARGO_MANIFEST_DIR` so no absolute path is ever baked in and
/// the file resolves the same from a test, from the binary and from a fuzz
/// target.
pub fn default_baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("baselines")
        .join("era-deltas.yaml")
}

/// Parse a baseline from text. The PURE seam — no file I/O, no environment.
///
/// This is what the fuzz target drives, so it MUST NOT PANIC on any input:
/// every rejection below is an `Err`.
///
/// # Rejections (the parser's contract)
///
/// A successfully returned [`EraBaseline`] is GUARANTEED to satisfy all four of
/// the following, because each is rejected here:
///
/// 1. the text is not valid YAML for the schema — `Err`;
/// 2. some delta's `id` is empty after trimming — `Err`;
/// 3. some delta's `observation_id` is empty after trimming — `Err`;
/// 4. two deltas share an `id`, or two deltas share an `observation_id` — `Err`.
///
/// Validation lives HERE rather than only in a test so that "a parsed baseline
/// has non-empty unique ids" is a PARSER CONTRACT. A downstream consumer that
/// keys on [`EraDelta::observation_id`] would otherwise silently merge two wire
/// facts, and a fuzz target asserting the property would crash on well-formed
/// input the parser had legitimately accepted.
///
/// Deliberately NOT rejected here: the lexical SHAPE of an `observation_id`
/// (lowercase, dot-separated), the length of a `source`, and whether a
/// provisional entry names its owning phase. Those are baseline-content rules,
/// gated by `crates/mcp-tester/tests/era_baseline.rs` against the checked-in
/// file — not properties of arbitrary input.
///
/// # Errors
///
/// Returns an error for each of the four cases above.
pub fn parse_baseline(text: &str) -> Result<EraBaseline> {
    let baseline: EraBaseline =
        serde_yaml::from_str(text).context("Failed to parse era-delta baseline YAML")?;

    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_observation_ids: HashSet<&str> = HashSet::new();

    for delta in &baseline.deltas {
        if delta.id.trim().is_empty() {
            bail!("era-delta baseline: an entry has an empty `id`");
        }
        if delta.observation_id.trim().is_empty() {
            bail!(
                "era-delta baseline: entry `{}` has an empty `observation_id`",
                delta.id
            );
        }
        if !seen_ids.insert(delta.id.as_str()) {
            bail!("era-delta baseline: duplicate `id` `{}`", delta.id);
        }
        if !seen_observation_ids.insert(delta.observation_id.as_str()) {
            bail!(
                "era-delta baseline: duplicate `observation_id` `{}` (on entry `{}`)",
                delta.observation_id,
                delta.id
            );
        }
    }

    Ok(baseline)
}

/// Read and parse a baseline from disk.
///
/// # Errors
///
/// Returns an error when the file cannot be read, or for any reason
/// [`parse_baseline`] rejects its contents.
pub fn load_baseline<P: AsRef<Path>>(path: P) -> Result<EraBaseline> {
    let path = path.as_ref();
    let text = fs::read_to_string(path)
        .with_context(|| format!("Failed to read era-delta baseline: {}", path.display()))?;
    parse_baseline(&text)
        .with_context(|| format!("Failed to parse era-delta baseline: {}", path.display()))
}

/// Read and parse the baseline shipped with this crate.
///
/// # Errors
///
/// Same conditions as [`load_baseline`].
pub fn load_default_baseline() -> Result<EraBaseline> {
    load_baseline(default_baseline_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal well-formed baseline, used as the mutation base for the negative
    /// cases below.
    fn valid_yaml() -> String {
        r#"
schema_version: 1
v1_protocol: "2025-11-25"
v2_protocol: "2026-07-28"
deltas:
  - id: ERA-01
    observation_id: method.initialize
    subject: "method:initialize"
    v1: served
    v2: absent
    kind: method-removed
    source: "REQUIREMENTS.md:911 (CLNT-01)"
  - id: ERA-02
    observation_id: method.server_discover
    subject: "method:server/discover"
    v1: "error:-32601"
    v2: served
    kind: method-added
    source: "src/client/mod.rs:887"
    provisional: true
    note: "Phase 112 owns this."
"#
        .to_string()
    }

    #[test]
    fn era_diff_parses_a_well_formed_baseline() {
        let baseline = parse_baseline(&valid_yaml()).expect("valid baseline must parse");
        assert_eq!(baseline.schema_version, 1);
        assert_eq!(baseline.deltas.len(), 2);
        assert!(!baseline.deltas[0].provisional, "default is false");
        assert!(baseline.deltas[0].note.is_none(), "default is None");
        assert!(baseline.deltas[1].provisional);
    }

    #[test]
    fn era_diff_rejects_an_empty_id() {
        let yaml = valid_yaml().replace("id: ERA-01", "id: \"   \"");
        let err = parse_baseline(&yaml).expect_err("an empty `id` must be rejected");
        assert!(
            err.to_string().contains("empty `id`"),
            "error must name the failure: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_an_empty_observation_id() {
        let yaml =
            valid_yaml().replace("observation_id: method.initialize", "observation_id: \"\"");
        let err = parse_baseline(&yaml).expect_err("an empty `observation_id` must be rejected");
        assert!(
            err.to_string().contains("empty `observation_id`"),
            "error must name the failure: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_a_duplicate_id() {
        let yaml = valid_yaml().replace("id: ERA-02", "id: ERA-01");
        let err = parse_baseline(&yaml).expect_err("a duplicate `id` must be rejected");
        assert!(
            err.to_string().contains("duplicate `id` `ERA-01`"),
            "error must name the duplicate: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_a_duplicate_observation_id() {
        let yaml = valid_yaml().replace(
            "observation_id: method.server_discover",
            "observation_id: method.initialize",
        );
        let err = parse_baseline(&yaml).expect_err("a duplicate `observation_id` must be rejected");
        assert!(
            err.to_string()
                .contains("duplicate `observation_id` `method.initialize`"),
            "error must name the duplicate: {err}"
        );
    }

    #[test]
    fn era_diff_rejects_garbage_without_panicking() {
        for garbage in [
            "",
            "\u{0}\u{1}\u{2}",
            "not: a: baseline",
            "deltas: []",
            "schema_version: \"one\"",
            "[1, 2, 3]",
        ] {
            assert!(
                parse_baseline(garbage).is_err(),
                "garbage input must be an Err, not a panic: {garbage:?}"
            );
        }
    }

    #[test]
    fn era_diff_loads_the_checked_in_baseline() {
        let baseline = load_default_baseline().expect("the shipped baseline must load");
        assert!(
            baseline.deltas.len() >= 14,
            "the shipped baseline must carry the seeded entries, found {}",
            baseline.deltas.len()
        );
        assert!(baseline
            .find_by_observation_id("method.initialize")
            .is_some());
        assert_eq!(baseline.observation_ids().len(), baseline.deltas.len());
    }

    // CLAUDE.md ALWAYS / PROPERTY testing: the parser's total-function property.
    proptest::proptest! {
        /// `parse_baseline` is TOTAL over arbitrary text: it returns, never
        /// unwinds. Complements the fuzz target, which drives arbitrary BYTES.
        #[test]
        fn era_diff_parse_baseline_never_panics_on_arbitrary_text(text in ".*") {
            let _ = parse_baseline(&text);
        }

        /// Whenever the parser ACCEPTS, its documented contract holds: every
        /// `id` and `observation_id` is non-empty and unique.
        #[test]
        fn era_diff_accepted_baselines_have_unique_nonempty_ids(suffix in "[a-z]{1,8}") {
            let yaml = valid_yaml().replace("method.server_discover", &format!("method.{suffix}"));
            if let Ok(baseline) = parse_baseline(&yaml) {
                let ids: std::collections::HashSet<&str> =
                    baseline.deltas.iter().map(|d| d.id.as_str()).collect();
                let observation_ids: std::collections::HashSet<&str> =
                    baseline.deltas.iter().map(|d| d.observation_id.as_str()).collect();
                proptest::prop_assert_eq!(ids.len(), baseline.deltas.len());
                proptest::prop_assert_eq!(observation_ids.len(), baseline.deltas.len());
                proptest::prop_assert!(baseline.deltas.iter().all(|d| !d.id.trim().is_empty()));
                proptest::prop_assert!(
                    baseline.deltas.iter().all(|d| !d.observation_id.trim().is_empty())
                );
            }
        }
    }
}
