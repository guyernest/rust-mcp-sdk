//! `dialect_version` — the workbook-declared DIALECT-version accessor (WBDL-02).
//!
//! This is a SIBLING of [`crate::version`], NOT an edit of it. The two readers
//! look alike — both scan a single-cell defined name — but they answer DIFFERENT
//! questions with OPPOSITE absence semantics:
//!
//! - [`crate::version::read_workbook_version`] reads the BUNDLE version
//!   (`version` / `wb_version`) and ERRORS on absent (Phase 94 CLI depends on it).
//! - This module reads the DIALECT version (`pmcp_dialect_version`, D-03) and
//!   treats absent as the BASELINE dialect (D-05) — never an error — while a
//!   PRESENT-but-incompatible declaration fails closed with a typed
//!   [`CompileError`] (D-04).
//!
//! The version contract (the supported / baseline `MAJOR.MINOR`) is OWNED by the
//! `pmcp-workbook-dialect` crate (parallel to its `WHITELIST`) and bound to
//! `docs/workbook-dialect-spec.md` by a drift-guard test. This module READS those
//! consts; it never redefines them.
//!
//! # Grammar
//!
//! The accepted version string is `MAJOR.MINOR` with an OPTIONAL `.PATCH`:
//! - `MAJOR.MINOR` REQUIRED; `.PATCH` tolerated.
//! - each component is base-10 digits only and parses into a `u64`; a component
//!   that overflows `u64` is MALFORMED (a typed error, never a panic).
//! - surrounding whitespace is trimmed; embedded whitespace (`1 .0`) is malformed.
//! - leading zeros are accepted (`01.0` == `1.0`); a single `0` component is legal.
//! - PATCH is IGNORED for the compatibility decision — compatibility is decided on
//!   `MAJOR.MINOR` only, so `1.0.999` is accepted when supported is `1.0`.

use pmcp_workbook_dialect::{BASELINE_DIALECT_VERSION, SUPPORTED_DIALECT_VERSION};

use crate::error::CompileError;
use crate::ingest::WorkbookMap;

/// The reserved defined name a workbook uses to declare its dialect version (D-03).
const DIALECT_VERSION_NAME: &str = "pmcp_dialect_version";

/// A parsed dialect version. `patch` is retained for round-tripping/diagnostics but
/// is IGNORED by the compatibility decision (which compares `MAJOR.MINOR` only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialectVersion {
    major: u64,
    minor: u64,
    patch: Option<u64>,
}

impl DialectVersion {
    /// The MAJOR component.
    #[must_use]
    pub fn major(self) -> u64 {
        self.major
    }

    /// The MINOR component.
    #[must_use]
    pub fn minor(self) -> u64 {
        self.minor
    }

    /// The optional PATCH component (ignored for compatibility).
    #[must_use]
    pub fn patch(self) -> Option<u64> {
        self.patch
    }

    /// Whether `self` (a DECLARED version) is compatible with `supported` under the
    /// D-04 rule: same major AND declared minor `<=` supported minor. PATCH is
    /// ignored. A different major OR a newer minor is INCOMPATIBLE.
    #[must_use]
    pub fn is_compatible_with(self, supported: DialectVersion) -> bool {
        self.major == supported.major && self.minor <= supported.minor
    }
}

/// Parse a `MAJOR.MINOR[.PATCH]` dialect-version string (the GRAMMAR above).
///
/// No `semver` crate — a hand-rolled base-10 parse (matches [`crate::version`]'s
/// crate-free posture). PATCH is parsed when present but ignored by the
/// compatibility decision. This is a PUBLIC path so the fuzz target (a separate
/// crate) and the example can call it, mirroring `formula::parse`.
///
/// # Errors
///
/// Returns [`CompileError::Lint`] for any malformed string: empty, wrong arity
/// (`1`, `1.0.0.0`), a non-digit component (`1.x`, `abc`), embedded whitespace
/// (`1 .0`), or a component overflowing `u64`. Never panics.
pub fn parse_dialect_version(raw: &str) -> Result<DialectVersion, CompileError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(malformed(raw, "the version string is empty"));
    }
    let parts: Vec<&str> = trimmed.split('.').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(malformed(
            raw,
            "expected MAJOR.MINOR with an optional .PATCH (2 or 3 dot-separated components)",
        ));
    }
    let major = parse_component(raw, "MAJOR", parts[0])?;
    let minor = parse_component(raw, "MINOR", parts[1])?;
    let patch = parts
        .get(2)
        .map(|p| parse_component(raw, "PATCH", p))
        .transpose()?;
    Ok(DialectVersion {
        major,
        minor,
        patch,
    })
}

/// Parse a single base-10 component into a `u64`. Rejects empty/non-digit/embedded
/// -whitespace components and `u64` overflow (typed error, never a panic).
fn parse_component(raw: &str, which: &str, component: &str) -> Result<u64, CompileError> {
    if component.is_empty() {
        return Err(malformed(raw, &format!("the {which} component is empty")));
    }
    // Base-10 digits ONLY: this also rejects embedded whitespace, a leading `+`,
    // and any unicode digit `from_str_radix` might otherwise have to consider.
    if !component.bytes().all(|b| b.is_ascii_digit()) {
        return Err(malformed(
            raw,
            &format!("the {which} component `{component}` is not base-10 digits"),
        ));
    }
    component.parse::<u64>().map_err(|_| {
        malformed(
            raw,
            &format!("the {which} component `{component}` overflows u64"),
        )
    })
}

/// Build the typed fail-closed error for a malformed version string.
fn malformed(raw: &str, why: &str) -> CompileError {
    CompileError::Lint(format!(
        "the `{DIALECT_VERSION_NAME}` declaration `{raw}` is malformed: {why} \
         (expected MAJOR.MINOR with an optional .PATCH, e.g. `1.0`)"
    ))
}

/// Resolve and VALIDATE the dialect version declared by `map`, honouring the D-04
/// compatibility policy and the D-05 absent→baseline rule.
///
/// - No `pmcp_dialect_version` cell (or an empty one) → the BASELINE version, no
///   error (D-05). Existing fixtures keep working with zero edits.
/// - A declared version → parsed, then checked against [`supported_version`]:
///   same major AND minor `<=` supported → accepted; otherwise a typed
///   [`CompileError::Lint`] (fail-closed, D-04), naming the offending and the
///   supported version.
///
/// # Errors
///
/// [`CompileError::Lint`] if the declared version is malformed or incompatible.
pub fn resolve_dialect_version(map: &WorkbookMap) -> Result<DialectVersion, CompileError> {
    match declared_dialect_version(map) {
        None => Ok(baseline_version()),
        Some(declared) => validate_declared(&declared),
    }
}

/// The SHARED step-(2a) fail-closed dialect-version gate BOTH compile lanes run
/// over the ingested [`WorkbookMap`]: the SEED lane (`compile_workbook_inner`) and
/// the GATED-UPDATE lane (`prepare_candidate_inner`, reached by every governed
/// re-compile through `cargo pmcp workbook compile`). Factoring the call into ONE
/// function ensures the two lanes cannot drift apart on the D-04 contract — the
/// HI-01 fail-closed gap was exactly such a drift (the check lived only in the seed
/// lane, so an author bumping `pmcp_dialect_version` to an incompatible value on an
/// already-seeded workbook was silently accepted on the gated-update path). It is a
/// thin wrapper over [`resolve_dialect_version`] that discards the resolved version:
/// both lanes need only the fail-closed REFUSAL, not the value.
///
/// Semantics are IDENTICAL on both lanes: a different major OR a newer-than-supported
/// minor → typed [`CompileError::Lint`] (fail-closed, D-04); an absent declaration →
/// the baseline with NO error (D-05, zero-churn for existing fixtures).
///
/// # Errors
///
/// [`CompileError::Lint`] if the declared version is malformed or incompatible.
pub fn validate_dialect_version_step(map: &WorkbookMap) -> Result<(), CompileError> {
    resolve_dialect_version(map).map(|_| ())
}

/// Validate a DECLARED version string against the supported version (the D-04
/// decision). Shared by [`resolve_dialect_version`] and the ALWAYS example so the
/// example exercises the SAME compat path the pipeline does.
///
/// # Errors
///
/// [`CompileError::Lint`] if `declared` is malformed or incompatible.
pub fn validate_declared(declared: &str) -> Result<DialectVersion, CompileError> {
    let version = parse_dialect_version(declared)?;
    let supported = supported_version();
    if version.is_compatible_with(supported) {
        Ok(version)
    } else {
        Err(CompileError::Lint(format!(
            "the workbook declares `{DIALECT_VERSION_NAME}` = `{declared}` \
             (parsed {}.{}), which is incompatible with the supported dialect \
             version `{SUPPORTED_DIALECT_VERSION}`: a compatible declaration has \
             the SAME major and a minor <= the supported minor (fail-closed)",
            version.major, version.minor,
        )))
    }
}

/// The compiler's supported (max) dialect version, parsed from the dialect crate's
/// const. Infallible on a well-formed const; a const that ever stops parsing is a
/// build-time programming error surfaced as a typed `Lint` (never a panic).
fn supported_version() -> DialectVersion {
    parse_const_version(SUPPORTED_DIALECT_VERSION)
}

/// The baseline dialect version (D-05 absent target), parsed from the dialect
/// crate's const.
fn baseline_version() -> DialectVersion {
    parse_const_version(BASELINE_DIALECT_VERSION)
}

/// Parse a dialect-crate version const, falling back to `0.0` on the
/// (build-time-impossible) unparseable case rather than panicking.
fn parse_const_version(s: &str) -> DialectVersion {
    parse_dialect_version(s).unwrap_or(DialectVersion {
        major: 0,
        minor: 0,
        patch: None,
    })
}

/// Resolve the declared dialect-version string from the owned [`WorkbookMap`], or
/// `None` when no `pmcp_dialect_version` single-cell defined name resolves to a
/// non-empty value. Read-only over the owned map (mirrors
/// [`crate::version`]'s scan, INVERTING the absence policy: `None` is baseline,
/// not an error).
fn declared_dialect_version(map: &WorkbookMap) -> Option<String> {
    for dn in &map.defined_names {
        if !dn.name.eq_ignore_ascii_case(DIALECT_VERSION_NAME) {
            continue;
        }
        // Single-cell target only (start == end): a range is not a scalar version.
        if dn.target.start != dn.target.end {
            continue;
        }
        if let Some(value) =
            crate::version::cell_value_for_key(map, &dn.target.sheet, &dn.target.start)
        {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        // A matched-but-empty target falls through (absent → baseline, D-05).
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::cell_map::{CellRecord, DefinedNameRecord, DefinedNameScope, FormulaKind};
    use crate::ingest::{RangeRef, SheetRecord};

    fn cell(addr: &str, value: Option<&str>) -> CellRecord {
        CellRecord {
            addr: addr.to_string(),
            formula: None,
            value: value.map(str::to_string),
            fill_argb: None,
            font_argb: None,
            number_format: None,
            is_formula: false,
            formula_kind: FormulaKind::Normal,
        }
    }

    fn single_cell_range(sheet: &str, addr: &str) -> RangeRef {
        RangeRef {
            sheet: sheet.to_string(),
            start: addr.to_string(),
            end: addr.to_string(),
        }
    }

    fn defined_name(name: &str, sheet: &str, addr: &str) -> DefinedNameRecord {
        DefinedNameRecord {
            name: name.to_string(),
            target: single_cell_range(sheet, addr),
            scope: DefinedNameScope::Workbook,
        }
    }

    /// A synthetic map declaring `name` -> (`sheet`!`addr`) with `cell_value`.
    fn map_declaring(name: &str, sheet: &str, addr: &str, cell_value: Option<&str>) -> WorkbookMap {
        WorkbookMap {
            sheets: vec![SheetRecord {
                name: sheet.to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                data_validations: vec![],
                notes: vec![],
                cells: vec![cell(addr, cell_value)],
            }],
            defined_names: vec![defined_name(name, sheet, addr)],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    /// An empty synthetic map (no defined names) — the absent-declaration case.
    fn empty_map() -> WorkbookMap {
        WorkbookMap {
            sheets: vec![],
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    // ---- parser grammar matrix ----

    #[test]
    fn parses_major_minor() {
        let v = parse_dialect_version("1.0").expect("1.0 parses");
        assert_eq!((v.major(), v.minor(), v.patch()), (1, 0, None));
    }

    #[test]
    fn parses_optional_patch() {
        let v = parse_dialect_version("1.0.999").expect("1.0.999 parses");
        assert_eq!((v.major(), v.minor(), v.patch()), (1, 0, Some(999)));
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let v = parse_dialect_version("  1.0  ").expect("trimmed parses");
        assert_eq!((v.major(), v.minor()), (1, 0));
    }

    #[test]
    fn rejects_embedded_whitespace() {
        assert!(parse_dialect_version("1 .0").is_err());
        assert!(parse_dialect_version("1. 0").is_err());
    }

    #[test]
    fn accepts_leading_zeros_numerically() {
        let v = parse_dialect_version("01.0").expect("leading zero parses");
        assert_eq!((v.major(), v.minor()), (1, 0));
        // 01.0 parses == 1.0
        assert_eq!(v, parse_dialect_version("1.0").expect("1.0"));
    }

    #[test]
    fn accepts_single_zero_component() {
        let v = parse_dialect_version("0.0").expect("0.0 parses");
        assert_eq!((v.major(), v.minor()), (0, 0));
    }

    #[test]
    fn rejects_malformed_strings_without_panic() {
        for bad in ["abc", "", "1.x", "1", "1.0.0.0", "1..0", ".1", "1.", "x.y"] {
            assert!(
                parse_dialect_version(bad).is_err(),
                "`{bad}` must be a typed error, not Ok"
            );
        }
    }

    #[test]
    fn rejects_u64_overflow_component() {
        // 2^64 = 18446744073709551616 — one past u64::MAX.
        let overflow = "18446744073709551616.0";
        assert!(matches!(
            parse_dialect_version(overflow),
            Err(CompileError::Lint(_))
        ));
    }

    // ---- compatibility decision ----

    #[test]
    fn same_major_le_minor_accepts() {
        // supported is 1.0; 1.0 is compatible.
        let v = validate_declared("1.0").expect("1.0 accepted");
        assert_eq!((v.major(), v.minor()), (1, 0));
    }

    #[test]
    fn patch_suffix_is_ignored_for_compat() {
        // 1.0.999 with supported 1.0 → accepted (patch ignored).
        let v = validate_declared("1.0.999").expect("patch ignored for compat");
        assert_eq!(v.patch(), Some(999));
    }

    #[test]
    fn leading_zero_declared_accepts() {
        validate_declared("01.0").expect("01.0 == 1.0 accepted");
    }

    #[test]
    fn different_major_rejected_with_typed_error() {
        let err = validate_declared("2.0").expect_err("different major rejected");
        assert!(matches!(err, CompileError::Lint(_)));
    }

    #[test]
    fn newer_minor_rejected_with_typed_error() {
        let err = validate_declared("1.5").expect_err("newer minor rejected");
        assert!(matches!(err, CompileError::Lint(_)));
    }

    // ---- resolve over the map (absent → baseline) ----

    #[test]
    fn absent_declaration_resolves_to_baseline_no_error() {
        let v = resolve_dialect_version(&empty_map()).expect("absent → baseline, no error");
        let baseline = parse_dialect_version(BASELINE_DIALECT_VERSION).expect("baseline parses");
        assert_eq!(v, baseline);
    }

    #[test]
    fn empty_version_cell_resolves_to_baseline() {
        let map = map_declaring(DIALECT_VERSION_NAME, "0_Meta", "B1", Some("   "));
        let v = resolve_dialect_version(&map).expect("empty cell → baseline");
        assert_eq!(
            v,
            parse_dialect_version(BASELINE_DIALECT_VERSION).expect("baseline")
        );
    }

    #[test]
    fn declared_compatible_resolves_accepted() {
        let map = map_declaring(DIALECT_VERSION_NAME, "0_Meta", "B1", Some("1.0"));
        let v = resolve_dialect_version(&map).expect("1.0 compatible");
        assert_eq!((v.major(), v.minor()), (1, 0));
    }

    #[test]
    fn declared_case_insensitive_name() {
        let map = map_declaring("PMCP_Dialect_Version", "0_Meta", "B1", Some("1.0"));
        resolve_dialect_version(&map).expect("name is case-insensitive");
    }

    #[test]
    fn declared_incompatible_resolves_to_typed_error() {
        let map = map_declaring(DIALECT_VERSION_NAME, "0_Meta", "B1", Some("2.0"));
        assert!(matches!(
            resolve_dialect_version(&map),
            Err(CompileError::Lint(_))
        ));
    }

    #[test]
    fn declared_malformed_resolves_to_typed_error() {
        let map = map_declaring(DIALECT_VERSION_NAME, "0_Meta", "B1", Some("nope"));
        assert!(matches!(
            resolve_dialect_version(&map),
            Err(CompileError::Lint(_))
        ));
    }

    #[test]
    fn range_target_is_not_a_scalar_version() {
        let map = WorkbookMap {
            sheets: vec![SheetRecord {
                name: "0_Meta".to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                data_validations: vec![],
                notes: vec![],
                cells: vec![cell("B1", Some("2.0"))],
            }],
            defined_names: vec![DefinedNameRecord {
                name: DIALECT_VERSION_NAME.to_string(),
                target: RangeRef {
                    sheet: "0_Meta".to_string(),
                    start: "B1".to_string(),
                    end: "B3".to_string(),
                },
                scope: DefinedNameScope::Workbook,
            }],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        };
        // A multi-cell range is ignored → absent → baseline (NOT the 2.0 error).
        let v = resolve_dialect_version(&map).expect("range ignored → baseline");
        assert_eq!(
            v,
            parse_dialect_version(BASELINE_DIALECT_VERSION).expect("baseline")
        );
    }

    /// PROPERTY (exhaustive grid, no new dependency): over a grid of MAJOR.MINOR
    /// pairs, the accept/reject decision matches the same-major-&&-minor<=supported
    /// rule. Adapted from `version.rs`'s round-trip grid.
    #[test]
    fn compat_decision_matches_rule_over_a_grid() {
        let supported = supported_version();
        let components = [0u64, 1, 2, 5, 12, 99];
        for &major in &components {
            for &minor in &components {
                let declared = format!("{major}.{minor}");
                let expected = major == supported.major && minor <= supported.minor;
                let got = validate_declared(&declared).is_ok();
                assert_eq!(
                    got, expected,
                    "compat decision for {declared} (supported {SUPPORTED_DIALECT_VERSION}) \
                     must be {expected}"
                );
            }
        }
    }
}

/// Integration coverage for the WIRED path (WBDL-02 Task 3): the dialect-version
/// check `compile_workbook_inner` runs at step (2a) is exactly
/// [`resolve_dialect_version`] over the ingested `WorkbookMap`. These cases drive
/// that SAME function over synthetic maps for all FIVE outcomes the wiring must
/// produce — {compatible, absent, newer-minor, different-major, malformed} —
/// asserting the compatible+absent maps resolve Ok (the pipeline proceeds) and the
/// newer-minor/different-major/malformed maps refuse with the typed
/// [`CompileError::Lint`] (fail-closed, the same refuse the driver propagates).
///
/// The full real-`.xlsx` pipeline witness that an ABSENT-version workbook still
/// compiles end-to-end (D-05 zero-churn) is `reemit_golden` (the committed
/// `tax-calc` fixture declares no `pmcp_dialect_version`).
#[cfg(test)]
mod wired_path_integration {
    use super::*;
    use crate::ingest::cell_map::{CellRecord, DefinedNameRecord, DefinedNameScope, FormulaKind};
    use crate::ingest::{RangeRef, SheetRecord};

    const NAME: &str = DIALECT_VERSION_NAME;

    /// A synthetic map declaring `NAME` -> (`sheet`!`addr`) with `cell_value`.
    fn map_declaring(sheet: &str, addr: &str, cell_value: &str) -> WorkbookMap {
        WorkbookMap {
            sheets: vec![SheetRecord {
                name: sheet.to_string(),
                state: "visible".to_string(),
                hidden_rows: vec![],
                hidden_cols: vec![],
                col_widths: vec![],
                merges: vec![],
                cf_ranges: vec![],
                tables: vec![],
                data_validations: vec![],
                notes: vec![],
                cells: vec![CellRecord {
                    addr: addr.to_string(),
                    formula: None,
                    value: Some(cell_value.to_string()),
                    fill_argb: None,
                    font_argb: None,
                    number_format: None,
                    is_formula: false,
                    formula_kind: FormulaKind::Normal,
                }],
            }],
            defined_names: vec![DefinedNameRecord {
                name: NAME.to_string(),
                target: RangeRef {
                    sheet: sheet.to_string(),
                    start: addr.to_string(),
                    end: addr.to_string(),
                },
                scope: DefinedNameScope::Workbook,
            }],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    fn absent_map() -> WorkbookMap {
        WorkbookMap {
            sheets: vec![],
            defined_names: vec![],
            external_links: vec![],
            has_macros: false,
            source_extension: "xlsx".to_string(),
            save_timestamp: None,
        }
    }

    #[test]
    fn wired_compatible_proceeds() {
        let map = map_declaring("0_Meta", "B1", "1.0");
        resolve_dialect_version(&map).expect("compatible declaration → pipeline proceeds");
    }

    #[test]
    fn wired_absent_proceeds_as_baseline() {
        let v =
            resolve_dialect_version(&absent_map()).expect("absent → baseline, pipeline proceeds");
        assert_eq!(
            v,
            parse_dialect_version(BASELINE_DIALECT_VERSION).expect("baseline parses")
        );
    }

    #[test]
    fn wired_newer_minor_refuses() {
        let map = map_declaring("0_Meta", "B1", "1.5");
        assert!(matches!(
            resolve_dialect_version(&map),
            Err(CompileError::Lint(_))
        ));
    }

    #[test]
    fn wired_different_major_refuses() {
        let map = map_declaring("0_Meta", "B1", "2.0");
        assert!(matches!(
            resolve_dialect_version(&map),
            Err(CompileError::Lint(_))
        ));
    }

    #[test]
    fn wired_malformed_refuses() {
        let map = map_declaring("0_Meta", "B1", "1.x");
        assert!(matches!(
            resolve_dialect_version(&map),
            Err(CompileError::Lint(_))
        ));
    }

    // ---- GATED-UPDATE lane parity (HI-01) ----
    //
    // The SHARED `validate_dialect_version_step` is the SINGLE function both lanes
    // call: `compile_workbook_inner` (SEED) and `prepare_candidate_inner`
    // (GATED-UPDATE). These cases exercise that shared step directly — the exact
    // call the gated-update re-compile path runs — proving the fail-closed gate is
    // no longer absent from the gated lane (the HI-01 D-04 gap). An author who bumps
    // `pmcp_dialect_version` to an incompatible value on an already-seeded workbook
    // is now refused on the re-compile path, and an ABSENT declaration still
    // re-compiles (baseline, zero churn).

    #[test]
    fn gated_update_step_refuses_incompatible_different_major() {
        // Simulates the author bumping `pmcp_dialect_version` to `2.0` on an
        // already-seeded workbook and re-running `cargo pmcp workbook compile`
        // (the gated-update lane). The shared step both lanes invoke must REFUSE.
        let map = map_declaring("0_Meta", "B1", "2.0");
        assert!(matches!(
            validate_dialect_version_step(&map),
            Err(CompileError::Lint(_))
        ));
    }

    #[test]
    fn gated_update_step_refuses_incompatible_newer_minor() {
        // A newer-than-supported minor (`1.5`) on the re-compile path must also fail
        // closed — identical semantics to the seed lane.
        let map = map_declaring("0_Meta", "B1", "1.5");
        assert!(matches!(
            validate_dialect_version_step(&map),
            Err(CompileError::Lint(_))
        ));
    }

    #[test]
    fn gated_update_step_absent_declaration_recompiles_as_baseline() {
        // An ABSENT declaration must still pass on the gated-update lane (D-05,
        // zero-churn): the shared step returns Ok, so the re-compile proceeds.
        validate_dialect_version_step(&absent_map())
            .expect("absent declaration → baseline, gated-update re-compile proceeds");
    }

    #[test]
    fn gated_update_and_seed_steps_agree_over_a_grid() {
        // Both lanes call the SAME `validate_dialect_version_step`, so the seed-lane
        // `resolve_dialect_version` Ok/Err verdict and the shared step's verdict must
        // agree for every map — proving the two lanes cannot drift (HI-01).
        for declared in ["1.0", "1.0.999", "01.0", "1.5", "2.0", "0.0", "1.x"] {
            let map = map_declaring("0_Meta", "B1", declared);
            assert_eq!(
                resolve_dialect_version(&map).is_ok(),
                validate_dialect_version_step(&map).is_ok(),
                "seed-lane and gated-update-lane verdicts must agree for `{declared}`"
            );
        }
    }
}
