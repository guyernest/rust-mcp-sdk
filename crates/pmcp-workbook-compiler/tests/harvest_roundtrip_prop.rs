//! WBV2-02 PROPERTY test (CLAUDE.md ALWAYS: PROPERTY) — the §3.3 per-row harvest
//! projection is TOTAL, STABLE (deterministic), and CLOSED over arbitrary
//! well-formed input rows.
//!
//! This complements the Plan-02 unit tests (example-based, `synth.rs`), the fuzz
//! target (`workbook_table_ingest` — malformed-XML containment), and the
//! real-template integration test (`template_harvest_e2e` — the actual artifact).
//! Here proptest generates an arbitrary well-formed harvest row and proves four
//! invariants of the projection that no finite set of examples can:
//!
//! 1. **Totality** — every well-formed row yields a `CellRole` with a DEFINED
//!    `dtype` and a DEFINED tier (never panics, never an undefined required field).
//! 2. **Stability / determinism** — harvesting the same row twice yields
//!    `PartialEq`-identical `CellRole`s (a pure fn of the row; no order/clock/hash
//!    dependence).
//! 3. **Unit closure** — `number_format_to_unit` only ever returns one of
//!    `{Some("USD"), Some("rate"), Some("date"), None}` (no arbitrary format string
//!    leaks through as a unit).
//! 4. **Tier closure** — the tier projection only ever yields `strict` or
//!    `variable`; any non-`"strict"`/blank tier cell maps to `variable`.

use pmcp_workbook_compiler::manifest::synth::{
    harvest_dtype, harvest_input_row, harvest_tier, number_format_to_unit, HarvestRow,
    HarvestedTier,
};
use pmcp_workbook_compiler::{Dtype, InputTier, Role};
use proptest::prelude::*;

/// A representative number-format alphabet drawn from the §3.3 unit sources plus
/// arbitrary junk — so the unit-closure property is exercised over BOTH the
/// recognised formats and random strings that must yield `None`.
fn number_format_strategy() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        // Currency witnesses.
        Just(Some("$#,##0".to_string())),
        Just(Some("[$USD] #,##0.00".to_string())),
        // Percent witness.
        Just(Some("0.0%".to_string())),
        // Date witnesses.
        Just(Some("yyyy-mm-dd".to_string())),
        Just(Some("dd/mm/yyyy".to_string())),
        // General / no-unit.
        Just(Some("General".to_string())),
        Just(Some("#,##0".to_string())),
        // Arbitrary junk format strings (must NOT leak through as a unit).
        "[a-zA-Z0-9 #,.%$/\\-]{0,12}".prop_map(Some),
    ]
}

/// A representative `tier` cell alphabet: the two declared dropdown values, blank,
/// absent, and arbitrary garbage — so the tier-closure property is exercised over
/// every shape a `tier` cell can take.
fn tier_strategy() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("strict".to_string())),
        Just(Some("STRICT".to_string())),
        Just(Some("  strict  ".to_string())),
        Just(Some("variable".to_string())),
        Just(Some(String::new())),
        "[a-zA-Z]{0,8}".prop_map(Some),
    ]
}

/// A `value` cell alphabet: numbers (various shapes) and text, so dtype totality is
/// exercised over both numeric-parseable and text values, plus blank.
fn value_strategy() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some(String::new())),
        any::<f64>().prop_map(|n| Some(format!("{n}"))),
        Just(Some("100000".to_string())),
        Just(Some("0.22".to_string())),
        "[a-zA-Z ]{1,16}".prop_map(Some),
    ]
}

prop_compose! {
    /// An arbitrary WELL-FORMED harvest row: a non-empty `name`, a value-cell
    /// content, a number-format drawn from the representative set, and a tier cell.
    fn arbitrary_row()(
        key in "[a-z][a-z0-9_]{0,15}",
        value in value_strategy(),
        number_format in number_format_strategy(),
        description in proptest::option::of("[a-zA-Z ]{0,24}"),
        tier in tier_strategy(),
    ) -> (String, Option<String>, Option<String>, Option<String>, Option<String>) {
        (key, value, number_format, description, tier)
    }
}

proptest! {
    /// Totality + stability over an arbitrary well-formed input row.
    #[test]
    fn input_harvest_is_total_and_stable(
        (key, value, number_format, description, tier) in arbitrary_row()
    ) {
        let row = HarvestRow {
            key: &key,
            value: value.as_deref(),
            number_format: number_format.as_deref(),
            description: description.as_deref(),
            tier: tier.as_deref(),
        };

        // TOTALITY: the projection never panics and yields defined required fields.
        let role = harvest_input_row(format!("S!{key}"), &row);

        // dtype is always one of the closed Dtype set (Number | Text | Bool) — the
        // harvest never produces an undefined dtype.
        prop_assert!(matches!(role.dtype, Dtype::Number | Dtype::Text | Dtype::Bool));

        // A variable input ALWAYS carries a defined Variable tier; a strict input is
        // a Role::Constant with tier None (the is_strict_constant shape). Either way
        // the (role, tier) pair is defined — never an undefined tier.
        match harvest_tier(row.tier) {
            HarvestedTier::Strict => {
                prop_assert_eq!(role.role, Role::Constant);
                prop_assert!(role.tier.is_none(), "strict → untiered constant");
            }
            HarvestedTier::Variable => {
                prop_assert_eq!(role.role, Role::Input);
                prop_assert!(
                    matches!(role.tier, Some(InputTier::Variable { .. })),
                    "variable → InputTier::Variable"
                );
            }
        }

        // The harvested key is always the row's name.
        prop_assert_eq!(role.name.as_deref(), Some(key.as_str()));

        // STABILITY / DETERMINISM: harvesting the same row twice is byte-identical.
        let role_again = harvest_input_row(format!("S!{key}"), &row);
        prop_assert_eq!(role, role_again);
    }

    /// Unit closure: `number_format_to_unit` codomain is `{USD, rate, date, None}`.
    #[test]
    fn unit_codomain_is_closed(fmt in number_format_strategy()) {
        let unit = fmt.as_deref().and_then(number_format_to_unit);
        prop_assert!(
            matches!(unit.as_deref(), Some("USD") | Some("rate") | Some("date") | None),
            "unit must be one of USD/rate/date/None, got {:?}",
            unit
        );
        // Determinism: the same format always projects to the same unit.
        let unit_again = fmt.as_deref().and_then(number_format_to_unit);
        prop_assert_eq!(unit, unit_again);
    }

    /// Tier closure: the tier projection only ever yields strict or variable, and a
    /// non-"strict"/blank tier cell always maps to variable.
    #[test]
    fn tier_codomain_is_closed(tier in tier_strategy()) {
        let harvested = harvest_tier(tier.as_deref());
        // The codomain is exactly {Strict, Variable} — exhaustively matched, so any
        // future third variant would fail to compile here (closed by construction).
        match harvested {
            HarvestedTier::Strict => {
                // Only a trimmed, case-folded "strict" reaches Strict.
                let normalised = tier.as_deref().map(|t| t.trim().to_ascii_lowercase());
                prop_assert_eq!(normalised.as_deref(), Some("strict"));
            }
            HarvestedTier::Variable => {
                // Every non-"strict" / blank / absent tier maps to Variable.
                let is_strict = tier
                    .as_deref()
                    .map(|t| t.trim().eq_ignore_ascii_case("strict"))
                    .unwrap_or(false);
                prop_assert!(!is_strict, "only strict maps away from Variable");
            }
        }
    }

    /// dtype totality (standalone witness): the dtype projection is defined for any
    /// value content (numeric-parseable → Number, else Text), never a panic.
    #[test]
    fn dtype_is_total_over_arbitrary_values(value in value_strategy()) {
        let dtype = harvest_dtype(value.as_deref());
        prop_assert!(matches!(dtype, Dtype::Number | Dtype::Text));
        // Determinism.
        prop_assert_eq!(dtype, harvest_dtype(value.as_deref()));
    }
}
