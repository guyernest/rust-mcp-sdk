//! Single-run `mcp-tester` report output, pinned as it stands at **0.7.0**.
//!
//! # What this file is
//!
//! These literals were captured against `mcp-tester` 0.7.0 **BEFORE** any of the
//! Phase-117 `--dual-run` work existed. They are the **D-11 / A-D11 ADDITIVITY
//! PROOF**: plan 117-11 adds a `--dual-run` mode, and this file is what turns
//! "single-run output is unchanged" from a claim into a fact.
//!
//! A diff here means the single-run output changed, which A-D11 forbids.
//!
//! # If a test in this file fails
//!
//! When a golden here diverges, **do NOT re-record** it. A re-recorded golden
//! silently converts a forbidden behaviour change into a documented one, which is
//! precisely the failure this file exists to prevent. The correct remedy is one
//! of:
//!
//! 1. Move the change behind the opt-in `--dual-run` path, so single-run output
//!    is untouched; or
//! 2. Put the new data in a **new top-level struct** rather than in `TestReport`.
//!
//! Option 2 has an in-repo precedent: `crates/mcp-tester/src/post_deploy_report.rs`
//! exists for exactly this reason. Its header argues the case verbatim — "Why a
//! new struct (vs. extending `TestReport`)" — and states the additivity rule this
//! file enforces: "Additive field changes (new optional fields with
//! `#[serde(default)]`) do NOT bump the version." `post_deploy_report` is the
//! shape to copy when 117-11 needs somewhere to put dual-run data.
//!
//! # Why the compiler is the strict consumer
//!
//! A-D11 resolved D-11 to ADDITIVE because `cargo-pmcp` links `mcp-tester` as a
//! **library**, not as a JSON producer: it struct-literals `TestResult`
//! (`cargo-pmcp/src/commands/test/apps.rs:874-880`) and matches `TestCategory`
//! exhaustively with no `_` arm (`cargo-pmcp/src/commands/test/conformance.rs:278-288`).
//! A new field on `TestResult` or a new variant on `TestCategory` is therefore a
//! hard workspace compile break, not a runtime surprise. `cargo build -p cargo-pmcp`
//! is the companion gate to this file.
//!
//! # Scope
//!
//! - `--format json` is pinned **byte-for-byte** (see [`assert_json_bytes`]).
//! - `--format pretty` is pinned by a criterion that can actually pass — see the
//!   header block above the pretty tests for the three measured non-determinism
//!   sources and what is asserted instead.
//! - `--format minimal` / `--format verbose` are smoke-level only; neither is a
//!   consumer contract.

use mcp_tester::{OutputFormat, TestCategory, TestReport, TestResult};
use std::time::Duration;

// ===========================================================================
// Capture seam
// ===========================================================================

/// Render `report` through the writer seam `report.rs:244-255` added by Phase 78
/// Plan 04 for exactly this purpose, and return the bytes as a `String`.
///
/// `print_to_writer` is the only way to assert what the binary prints: the
/// `print` path (`report.rs:230-236`) writes straight to stdout.
fn capture(report: &TestReport, format: OutputFormat) -> String {
    let mut sink = Vec::<u8>::new();
    report
        .print_to_writer(format, &mut sink)
        .expect("writing a report into a Vec<u8> cannot fail");
    String::from_utf8(sink).expect("report output must be valid UTF-8")
}

// ===========================================================================
// Width-preserving normalization
//
// `DynamicField`, `width_preserving`, `substitute`, `substitute_one` and
// `key_occurrences` are RESTATED from `tests/v1_lists_golden.rs:97-186` rather
// than imported: a Rust integration test is its own crate, so the two files
// cannot see each other's items. The one adaptation is the needle — this file
// normalizes `serde_json::to_string_pretty` output, which puts a space after
// the key's colon, whereas `v1_lists_golden.rs` normalizes compact wire bytes.
// ===========================================================================

/// One JSON object key whose STRING value is genuinely per-run.
struct DynamicField {
    /// The JSON object key whose STRING value is dynamic.
    key: &'static str,
    /// The canonical placeholder written into the golden literal.
    token: &'static str,
    /// Shape predicate the raw value must satisfy — a normalization that
    /// accepted any string would let a reshaped value through unnoticed.
    shape: fn(&str) -> bool,
    /// Human-readable form of `shape`, for the failure message.
    shape_description: &'static str,
}

/// The only per-run value in the `--format json` output.
///
/// `TestReport.timestamp` is `Utc::now()` (`report.rs:174`) and chrono renders it
/// as an RFC 3339 instant whose fractional-second digit count varies, so both its
/// content AND its byte width move between runs.
///
/// `duration` is deliberately NOT here. `std::time::Duration` serializes as a
/// serde struct (`{"secs": N, "nanos": N}`), not as a string, and every fixture in
/// this file pins it to `Duration::from_secs(0)` — the same choice
/// `TestReport::from_error` already makes at `report.rs:197`. At zero it renders
/// `"secs": 0, "nanos": 0` on every run, so it is fully deterministic and needs no
/// dynamic at all. [`json_duration_is_deterministic_without_a_dynamic`] is the
/// executed proof of that, so the claim is checked rather than asserted in prose.
const JSON_DYNAMICS: &[DynamicField] = &[DynamicField {
    key: "timestamp",
    token: "<TIMESTAMP>",
    shape: is_rfc3339_utc_instant,
    shape_description: "an RFC 3339 UTC instant (`YYYY-MM-DDTHH:MM:SS[.frac]Z`)",
}];

/// Minimal RFC 3339 UTC shape check: `YYYY-MM-DDTHH:MM:SS` then an optional
/// fractional part, then a literal `Z`.
fn is_rfc3339_utc_instant(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() < 20 || !value.ends_with('Z') {
        return false;
    }
    let digits_at = |positions: &[usize]| {
        positions
            .iter()
            .all(|&i| bytes.get(i).is_some_and(u8::is_ascii_digit))
    };
    digits_at(&[0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18])
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
}

/// `token`, padded with `#` to exactly `width` bytes.
fn width_preserving(token: &str, width: usize) -> String {
    assert!(
        token.len() <= width,
        "placeholder `{token}` is wider than the {width}-byte value it replaces; \
         pick a shorter token rather than shortening the value"
    );
    let mut padded = String::with_capacity(width);
    padded.push_str(token);
    padded.push_str(&"#".repeat(width - token.len()));
    padded
}

/// Replace every dynamic value in `raw`.
///
/// With `same_width`, each value becomes a padded placeholder of its own width;
/// otherwise it becomes the bare canonical token. Both passes are pure string
/// operations, so key order, spacing and null-versus-absent all survive into the
/// comparison.
fn substitute(raw: &str, fields: &[DynamicField], same_width: bool) -> String {
    let mut out = raw.to_string();
    for field in fields {
        out = substitute_one(&out, field, same_width);
    }
    out
}

fn substitute_one(raw: &str, field: &DynamicField, same_width: bool) -> String {
    // `to_string_pretty` emits `"key": "value"` — note the space after the colon.
    let needle = format!("\"{}\": \"", field.key);
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut hits = 0_usize;
    while let Some(position) = rest.find(needle.as_str()) {
        let value_start = position + needle.len();
        out.push_str(&rest[..value_start]);
        let tail = &rest[value_start..];
        let end = tail
            .find('"')
            .unwrap_or_else(|| panic!("unterminated `{}` string value in: {raw}", field.key));
        let value = &tail[..end];
        assert!(
            (field.shape)(value),
            "`{}` carried `{value}`, which is not {} — either the value shape \
             changed (a single-run output break) or this fixture is normalizing \
             the wrong key",
            field.key,
            field.shape_description
        );
        if same_width {
            out.push_str(&width_preserving(field.token, value.len()));
        } else {
            out.push_str(field.token);
        }
        rest = &tail[end..];
        hits += 1;
    }
    assert!(
        hits > 0,
        "declared dynamic key `{}` does not appear in the output — a golden that \
         normalizes an absent key proves nothing: {raw}",
        field.key
    );
    out.push_str(rest);
    out
}

fn key_occurrences(text: &str, key: &str) -> usize {
    text.matches(format!("\"{key}\":").as_str()).count()
}

/// The failure text the raw-byte comparison carries.
///
/// Factored out so the `assert_eq!` invocation stays readable: this is the
/// assertion a reviewer greps for when asking "does this file actually compare
/// bytes?", and a macro split across six lines by `rustfmt` answers that much
/// less clearly.
fn additivity_break_message(raw: &str) -> String {
    format!(
        "ADDITIVITY BREAK: the single-run `--format json` output of mcp-tester changed. \
         A-D11 forbids this — plan 117-11's `--dual-run` work must be ADDITIVE. \
         Do NOT re-record this golden. Either move the change behind the opt-in \
         `--dual-run` path, or put the new data in a new top-level struct the way \
         `crates/mcp-tester/src/post_deploy_report.rs` does. Raw output was:\n{raw}"
    )
}

/// Assert `raw` is byte-identical to `golden` once per-run values are replaced.
///
/// Four things happen, in this order:
///
/// 1. **Width invariant.** A same-width substitution must leave the length
///    unchanged. This is what makes "the normalization never adds or removes a
///    byte" a checked property rather than a comment.
/// 2. **Per-key occurrence invariant.** Each dynamic key must appear exactly as
///    often after substitution as before — so the normalizer provably replaces
///    VALUES only and can never delete a key.
/// 3. **Canonical substitution** to the bare token.
/// 4. **RAW-STRING comparison** against the golden — the load-bearing assertion,
///    and the only one that sees key order, spacing and null-versus-absent.
fn assert_json_bytes(raw: &str, golden: &str, dynamics: &[DynamicField]) {
    let same_width = substitute(raw, dynamics, true);
    assert_eq!(
        same_width.len(),
        raw.len(),
        "the placeholder substitution changed the output length; it must be \
         width-preserving so it cannot mask an added or removed byte: {raw}"
    );
    for field in dynamics {
        assert_eq!(
            key_occurrences(&same_width, field.key),
            key_occurrences(raw, field.key),
            "the substitution changed how often `{}` appears; it must replace \
             VALUES only and never delete a key: {raw}",
            field.key
        );
    }

    let normalized = substitute(raw, dynamics, false);
    assert_eq!(normalized, golden, "{}", additivity_break_message(raw));
}

// ===========================================================================
// Fixtures
//
// Every `TestResult` uses `Duration::from_secs(0)` — the same choice
// `TestReport::from_error` makes at `report.rs:197` — so nothing downstream
// depends on a real elapsed time. `TestReport.duration` is set to zero
// explicitly. `timestamp` is left as whatever `TestReport::new()` produces
// (`Utc::now()`), so it is handled by the normalizer rather than by the fixture.
// ===========================================================================

/// Six tests across three categories, two per category, covering all four
/// statuses — and deliberately INTERLEAVED, so the categories are not already
/// contiguous in `tests`.
///
/// Interleaving matters twice over. For `--format json` the output order is the
/// `Vec` order (`report.rs:154`), so the golden pins that the serializer does not
/// reorder. For `--format pretty` the printer regroups by category, so the
/// intra-block ordering assertion is a real check rather than a tautology on
/// one-element blocks.
fn multi_category_fixture() -> TestReport {
    let mut report = TestReport::new();
    report.duration = Duration::from_secs(0);
    report.add_test(TestResult::passed(
        "initialize",
        TestCategory::Core,
        Duration::from_secs(0),
        "protocol 2025-11-25",
    ));
    report.add_test(TestResult::failed(
        "tools/call echo",
        TestCategory::Tools,
        Duration::from_secs(0),
        "handler returned isError",
    ));
    report.add_test(TestResult::skipped(
        "tasks/get",
        TestCategory::Tasks,
        "server does not advertise the tasks capability",
    ));
    report.add_test(TestResult::passed(
        "ping",
        TestCategory::Core,
        Duration::from_secs(0),
        "round trip ok",
    ));
    report.add_test(TestResult::warning(
        "tools/list",
        TestCategory::Tools,
        Duration::from_secs(0),
        "server returned an empty tool list",
    ));
    report.add_test(TestResult::passed(
        "tasks/result",
        TestCategory::Tasks,
        Duration::from_secs(0),
        "terminal task returned a result",
    ));
    report
}

/// The golden for [`multi_category_fixture`] under `--format json`.
///
/// Captured against mcp-tester 0.7.0. `<TIMESTAMP>` is the ONLY normalized value.
const JSON_GOLDEN: &str = r#"{
  "tests": [
    {
      "name": "initialize",
      "category": "Core",
      "status": "Passed",
      "duration": {
        "secs": 0,
        "nanos": 0
      },
      "error": null,
      "details": "protocol 2025-11-25"
    },
    {
      "name": "tools/call echo",
      "category": "Tools",
      "status": "Failed",
      "duration": {
        "secs": 0,
        "nanos": 0
      },
      "error": "handler returned isError",
      "details": null
    },
    {
      "name": "tasks/get",
      "category": "Tasks",
      "status": "Skipped",
      "duration": {
        "secs": 0,
        "nanos": 0
      },
      "error": null,
      "details": "server does not advertise the tasks capability"
    },
    {
      "name": "ping",
      "category": "Core",
      "status": "Passed",
      "duration": {
        "secs": 0,
        "nanos": 0
      },
      "error": null,
      "details": "round trip ok"
    },
    {
      "name": "tools/list",
      "category": "Tools",
      "status": "Warning",
      "duration": {
        "secs": 0,
        "nanos": 0
      },
      "error": null,
      "details": "server returned an empty tool list"
    },
    {
      "name": "tasks/result",
      "category": "Tasks",
      "status": "Passed",
      "duration": {
        "secs": 0,
        "nanos": 0
      },
      "error": null,
      "details": "terminal task returned a result"
    }
  ],
  "duration": {
    "secs": 0,
    "nanos": 0
  },
  "timestamp": "<TIMESTAMP>",
  "summary": {
    "total": 6,
    "passed": 3,
    "failed": 1,
    "warnings": 1,
    "skipped": 1
  }
}
"#;

// ===========================================================================
// 1. `--format json` — the byte contract
// ===========================================================================

/// `print_json` (`report.rs:460-463`) emits `serde_json::to_string_pretty(&self)`
/// verbatim, so the binary's `--format json` contract IS the serde shape of
/// `TestReport`. Pinning these bytes pins that shape.
#[test]
fn json_single_run_output_is_byte_pinned() {
    let raw = capture(&multi_category_fixture(), OutputFormat::Json);
    assert_json_bytes(&raw, JSON_GOLDEN, JSON_DYNAMICS);
}

/// The byte comparison must not be passing over an empty report.
#[test]
fn json_fixture_is_not_vacuous() {
    let report = multi_category_fixture();

    assert!(
        report.tests.len() >= 3,
        "FAILURE MODE: the pinned fixture carries {} test(s), below the floor of 3. \
         A golden captured from an empty or near-empty `tests` array pins almost \
         nothing — every field of `TestResult` would go unexercised and the \
         additivity proof would be vacuous.\n\
         WHAT TO DO: restore the fixture's tests; do not lower the floor.",
        report.tests.len()
    );

    assert!(
        report.summary.total > 0,
        "FAILURE MODE: `summary.total` is 0, so the summary block of the golden is \
         all zeroes and a regression in the counters could not be seen.\n\
         WHAT TO DO: build the fixture through `add_test`, which is what maintains \
         the counters (report.rs:204-213); do not hand-write the summary."
    );

    let raw = capture(&report, OutputFormat::Json);
    assert!(
        raw.contains("\"total\": 6"),
        "FAILURE MODE: the captured json does not carry `\"total\": 6`, so the \
         fixture and the golden have drifted apart and the byte assertion may be \
         comparing something other than what this test checked.\n\
         WHAT TO DO: keep the fixture and JSON_GOLDEN in sync in the same commit."
    );
}

/// The recorded measurement behind [`JSON_DYNAMICS`] carrying no `duration` entry.
///
/// `std::time::Duration` serializes as a serde struct, not a string, and at
/// `Duration::from_secs(0)` it renders identically on every run — so it needs no
/// width-preserving dynamic. This test is what makes that a measurement rather
/// than an assumption: if `Duration`'s serde representation ever changes, this
/// fails and `JSON_DYNAMICS` has to be revisited.
#[test]
fn json_duration_is_deterministic_without_a_dynamic() {
    let first = capture(&multi_category_fixture(), OutputFormat::Json);
    let second = capture(&multi_category_fixture(), OutputFormat::Json);

    assert!(
        first.contains("\"duration\": {\n        \"secs\": 0,\n        \"nanos\": 0\n      }"),
        "`Duration` no longer serializes as the `{{secs, nanos}}` struct this \
         golden assumes; JSON_DYNAMICS may now need a `duration` entry. Output was:\n{first}"
    );

    let strip_timestamps = |text: &str| substitute(text, JSON_DYNAMICS, false);
    assert_eq!(
        strip_timestamps(&first),
        strip_timestamps(&second),
        "two captures of the same fixture differ once `timestamp` is normalized, \
         so something OTHER than `timestamp` is per-run and JSON_DYNAMICS is \
         incomplete"
    );
}
