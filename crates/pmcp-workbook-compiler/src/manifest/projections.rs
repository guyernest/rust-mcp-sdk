//! BA-string metadata limits + DV inline-literal resolution (the info-flow
//! boundary for strings reaching the agent LLM — Codex MEDIUM; T-93-04-INJ).
//!
//! BA-authored cell metadata (a cell's `meaning`, `unit`, and enum labels) reaches
//! the agent LLM through the synthesized manifest → served tool schema. Before any
//! such string enters the [`Manifest`](super::model::Manifest) it is SANITIZED:
//! control characters are stripped, runs of whitespace collapse to one space, and
//! the result is truncated at a documented cap with an ellipsis. An overflow emits
//! a WARNING (never a hard block — keep the BA in Excel; D-04). These caps are the
//! prompt-injection info-flow boundary.
//!
//! This module ALSO carries the DV → enum resolution (WBCO-06/D-06): inline quoted
//! literal lists of ≤10 distinct values freeze to a closed JSON-Schema enum; every
//! other DV source (range / named range / formula / governed / too-many) stays a
//! DYNAMIC input with a PRECISE reason code so the served tool never over-promises
//! a schema.

use std::collections::HashSet;

/// Max chars for a cell `meaning` string before it enters the manifest.
pub const MAX_MEANING_LEN: usize = 280;
/// Max chars for a `unit` string before it enters the manifest.
pub const MAX_UNIT_LEN: usize = 32;
/// Max chars for a single enum label before it enters the manifest.
pub const MAX_ENUM_LABEL_LEN: usize = 64;
/// Max number of exposed INPUT cells the manifest may carry.
pub const MAX_INPUT_COUNT: usize = 64;
/// Max number of exposed OUTPUT cells the manifest may carry.
pub const MAX_OUTPUT_COUNT: usize = 64;
/// Max chars for a sheet name surfaced into a manifest string.
pub const MAX_SHEET_NAME_LEN: usize = 64;

/// The D-03/D-06 ceiling on DISTINCT inline-literal values eligible to freeze.
pub const MAX_FROZEN_VALUES: usize = 10;

/// The ellipsis appended when a sanitized string is truncated at its cap.
const ELLIPSIS: char = '…';

/// Sanitize a BA-authored metadata string and cap it at `max` chars.
///
/// The pipeline is: strip control characters (`char::is_control`), collapse every
/// run of whitespace to a single ASCII space, trim the ends, then — if the
/// remaining char count exceeds `max` — truncate to `max - 1` chars and append a
/// single ellipsis so the result is exactly `max` chars. Returns `(sanitized,
/// truncated)`; `truncated == true` signals the caller to emit a WARNING (never a
/// hard block). Capping by `char` count (not bytes) keeps multibyte text intact.
#[must_use]
pub fn sanitize_capped(raw: &str, max: usize) -> (String, bool) {
    // Strip control chars; collapse whitespace runs to one space.
    let mut cleaned = String::with_capacity(raw.len());
    let mut prev_ws = false;
    for ch in raw.chars() {
        if ch.is_whitespace() {
            // Whitespace (incl. control whitespace like tab/newline) collapses to
            // a single space — it MUST be handled before the control-char strip so
            // a tab/newline does not silently fuse two words together.
            if !prev_ws {
                cleaned.push(' ');
            }
            prev_ws = true;
        } else if ch.is_control() {
            // A non-whitespace control char (e.g. BEL) is stripped entirely; it is
            // NOT a word boundary, so `prev_ws` is left unchanged.
            continue;
        } else {
            cleaned.push(ch);
            prev_ws = false;
        }
    }
    let trimmed = cleaned.trim();

    let count = trimmed.chars().count();
    if max == 0 {
        return (String::new(), count > 0);
    }
    if count <= max {
        return (trimmed.to_string(), false);
    }
    // Truncate to (max - 1) chars + an ellipsis so the capped string is `max`.
    let kept: String = trimmed.chars().take(max.saturating_sub(1)).collect();
    let mut out = kept;
    out.push(ELLIPSIS);
    (out, true)
}

/// Sanitize + cap an OPTIONAL metadata string in place: `None` passes through
/// (returns `false` — nothing to truncate); `Some(raw)` is sanitized to `max`.
/// Returns whether a truncation occurred (the caller emits the WARNING).
#[must_use]
pub fn sanitize_opt(value: &mut Option<String>, max: usize) -> bool {
    match value {
        None => false,
        Some(raw) => {
            let (capped, truncated) = sanitize_capped(raw, max);
            *value = Some(capped);
            truncated
        },
    }
}

/// Tokenize an INLINE LITERAL quoted list (`"a,b,c"` WITH the literal surrounding
/// double quotes) into its trimmed, deduped tokens (first occurrence wins, order
/// preserved — D-07, NO sort). Each surviving token is sanitized + capped at
/// [`MAX_ENUM_LABEL_LEN`]. `None` when `formula1` is not a surrounding-quoted
/// literal (a range / NAMED RANGE / bare ref — resolution of those is DEFERRED).
fn inline_literal_tokens(formula1: &str) -> Option<Vec<String>> {
    let inner = formula1
        .trim()
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))?;
    let mut seen = HashSet::new();
    Some(
        inner
            .split(',')
            .map(|s| {
                let (capped, _truncated) = sanitize_capped(s, MAX_ENUM_LABEL_LEN);
                capped
            })
            .filter(|s| !s.is_empty() && seen.insert(s.clone()))
            .collect(),
    )
}

/// Resolve an INLINE LITERAL quoted list to its frozen token set: trim + dedup
/// preserving the FIRST occurrence (workbook order, NO sort — D-07), each token
/// enum-label-capped, accepting only a non-empty set of ≤10 DISTINCT values (D-06).
///
/// `None` for an empty list, >10 distinct values, or any non-literal source
/// (`=$Z$1:$Z$2`, `=SomeName`, a bare ref) — range/named-range static resolution is
/// a documented DEFERRED extension; this phase freezes inline quoted literals ONLY.
#[must_use]
pub fn resolve_inline_list(formula1: &str) -> Option<Vec<String>> {
    let vals = inline_literal_tokens(formula1)?;
    (!vals.is_empty() && vals.len() <= MAX_FROZEN_VALUES).then_some(vals)
}

/// Whether `formula1` is an inline quoted literal at all (used to distinguish the
/// `too_many_values` reason from `not_inline_literal` in the synth fork).
#[must_use]
pub fn is_inline_literal(formula1: &str) -> bool {
    inline_literal_tokens(formula1).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_passes_a_clean_short_string_through() {
        let (out, truncated) = sanitize_capped("Total floor area", MAX_MEANING_LEN);
        assert_eq!(out, "Total floor area");
        assert!(!truncated);
    }

    #[test]
    fn sanitize_strips_control_chars_and_collapses_whitespace() {
        let raw = "Total\u{0007}  floor\t\tarea\n";
        let (out, truncated) = sanitize_capped(raw, MAX_MEANING_LEN);
        assert_eq!(out, "Total floor area", "control stripped, ws collapsed");
        assert!(!truncated);
    }

    #[test]
    fn sanitize_truncates_with_an_ellipsis_at_the_cap() {
        let raw = "a".repeat(50);
        let (out, truncated) = sanitize_capped(&raw, MAX_UNIT_LEN); // 32
        assert!(truncated, "an over-long unit must signal truncation");
        assert_eq!(out.chars().count(), MAX_UNIT_LEN, "capped to exactly max");
        assert!(out.ends_with('…'), "truncation appends an ellipsis");
    }

    #[test]
    fn sanitize_caps_a_multibyte_string_by_char_not_byte() {
        // 40 multibyte chars capped at 32: the result is 32 CHARS (not bytes).
        let raw = "é".repeat(40);
        let (out, truncated) = sanitize_capped(&raw, MAX_UNIT_LEN);
        assert!(truncated);
        assert_eq!(out.chars().count(), MAX_UNIT_LEN);
    }

    #[test]
    fn sanitize_opt_handles_none_and_some() {
        let mut none: Option<String> = None;
        assert!(!sanitize_opt(&mut none, MAX_MEANING_LEN));
        assert_eq!(none, None);

        let mut some = Some("a".repeat(300));
        let truncated = sanitize_opt(&mut some, MAX_MEANING_LEN);
        assert!(truncated);
        assert_eq!(some.as_deref().map(|s| s.chars().count()), Some(280));
    }

    #[test]
    fn inline_list_parses_and_caps_enum_labels() {
        assert_eq!(
            resolve_inline_list("\"single,married\""),
            Some(vec!["single".to_string(), "married".to_string()]),
        );
    }

    #[test]
    fn inline_list_trims_dedups_preserving_first_occurrence_no_sort() {
        assert_eq!(
            resolve_inline_list("\" b , a , b , c \""),
            Some(vec!["b".to_string(), "a".to_string(), "c".to_string()]),
        );
    }

    #[test]
    fn inline_list_rejects_more_than_ten_distinct() {
        let eleven = "\"v1,v2,v3,v4,v5,v6,v7,v8,v9,v10,v11\"";
        assert_eq!(resolve_inline_list(eleven), None);
        // 11 raw tokens deduping to 10 distinct still freezes.
        let dup = "\"v1,v2,v3,v4,v5,v6,v7,v8,v9,v10,v1\"";
        assert_eq!(resolve_inline_list(dup).map(|v| v.len()), Some(10));
    }

    #[test]
    fn inline_list_rejects_non_literal_sources() {
        assert_eq!(resolve_inline_list("=$Z$1:$Z$2"), None);
        assert_eq!(resolve_inline_list("=SomeName"), None);
        assert_eq!(resolve_inline_list("SomeRef"), None);
        assert!(!is_inline_literal("=SomeName"));
        assert!(is_inline_literal("\"a,b\""));
    }

    #[test]
    fn enum_label_overflow_is_capped_not_dropped() {
        // A 100-char label freezes but is capped to MAX_ENUM_LABEL_LEN chars.
        let long = "x".repeat(100);
        let formula = format!("\"{long},short\"");
        let resolved = resolve_inline_list(&formula).expect("freezes");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].chars().count(), MAX_ENUM_LABEL_LEN);
        assert_eq!(resolved[1], "short");
    }
}
