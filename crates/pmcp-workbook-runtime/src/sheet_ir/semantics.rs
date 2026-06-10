//! The Excel-semantics layer: real bodies for all 13 whitelisted functions
//! over the range-capable [`EvalValue`] (D-09).
//!
//! [`apply`] dispatches on the [`crate::formula::Expr::Call`] `name` (D-02 keeps
//! `name` a `String`) over the 13-name [`crate::dialect::rules::WHITELIST`] —
//! mirroring the small exhaustive `match` + early-return shape of
//! `dialect::rules::candidate_role`. Each function owns the genuinely-Excel
//! concerns (1-based indexing, empty-cell-as-0, error propagation, lookup); leaf
//! SCALAR arithmetic delegates to [`super::eval_bridge`], and `ROUND`/`ROUNDUP`/
//! `CEILING` delegate to the deterministic [`super::rounding`] helpers.
//!
//! Boundaries (D-04, finding #4):
//! - `IFERROR`/`ISNUMBER` inspect the argument's [`CellValue`] DIRECTLY — never
//!   routed through eval.
//! - `SUM`/`SUMIF`/`VLOOKUP`/`INDEX`/`MATCH` consume [`EvalValue::Range`].
//! - Every index/lookup is 1-based and bounds-checked → an [`ExcelError`] (never
//!   a panic, T-09-13).

use crate::excel_error::ExcelError;

use super::eval_value::EvalValue;
use super::rounding::{excel_ceiling, excel_round, excel_roundup};
use super::value::CellValue;

/// Dispatch an Excel function call by `name` over its range-capable `args`.
///
/// An unknown name (outside the whitelist) yields `#NAME?` — the parser already
/// rejects out-of-whitelist names (Plan 02), so this is a defensive backstop.
pub fn apply(name: &str, args: &[EvalValue]) -> CellValue {
    // Each body returns `Result<_, ExcelError>` so argument coercion can use `?`;
    // a propagated `Err` becomes a `CellValue::Error` at this single boundary.
    let result = match name {
        "IF" => f_if(args),
        "SUM" => f_sum(args),
        "SUMIF" => f_sumif(args),
        "VLOOKUP" => f_vlookup(args),
        "INDEX" => f_index(args),
        "MATCH" => f_match(args),
        "ROUND" => f_round(args),
        "ROUNDUP" => f_roundup(args),
        "CEILING" => f_ceiling(args),
        "IFERROR" => f_iferror(args),
        "ISNUMBER" => f_isnumber(args),
        "SEARCH" => f_search(args),
        "TEXT" => f_text(args),
        _ => Err(ExcelError::Name),
    };
    result.unwrap_or_else(CellValue::Error)
}

// ---------------------------------------------------------------------------
// Shared primitives (empty-cell-as-0, error propagation, coercion, indexing).
// ---------------------------------------------------------------------------

/// Coerce a scalar [`CellValue`] to an Excel number: `Number`→itself,
/// `Empty`→0 (empty-cell-as-0), `Bool`→1/0, a numeric `Text`→its value, a
/// non-numeric `Text`→`#VALUE!`, an `Error`→propagated.
///
/// `pub(crate)` so the executor's off-kernel `^`/`%` arms reuse the single
/// canonical scalar→f64 Excel coercion instead of re-implementing it.
pub(crate) fn to_number(cv: &CellValue) -> Result<f64, ExcelError> {
    match cv {
        CellValue::Number(n) => Ok(*n),
        CellValue::Empty => Ok(0.0),
        CellValue::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
        CellValue::Text(s) => s.trim().parse::<f64>().map_err(|_| ExcelError::Value),
        CellValue::Error(e) => Err(*e),
    }
}

/// Excel truthiness for `IF` conditions: `Bool`→itself, `Number`→non-zero,
/// `Empty`→false, `Text`→non-empty, `Error`→propagated.
fn is_truthy(cv: &CellValue) -> Result<bool, ExcelError> {
    match cv {
        CellValue::Bool(b) => Ok(*b),
        CellValue::Number(n) => Ok(*n != 0.0),
        CellValue::Empty => Ok(false),
        CellValue::Text(s) => Ok(!s.is_empty()),
        CellValue::Error(e) => Err(*e),
    }
}

/// Pull the scalar out of an arg, error if it is a range where a scalar is
/// required.
fn arg_scalar(args: &[EvalValue], i: usize) -> Result<&CellValue, ExcelError> {
    match args.get(i) {
        Some(EvalValue::Scalar(cv)) => Ok(cv),
        Some(EvalValue::Range(_)) => Err(ExcelError::Value),
        None => Err(ExcelError::Na), // missing required arg
    }
}

/// Pull a range out of an arg, error if it is a scalar where a range is required.
fn arg_range(args: &[EvalValue], i: usize) -> Result<&Vec<Vec<CellValue>>, ExcelError> {
    match args.get(i) {
        Some(EvalValue::Range(rows)) => Ok(rows),
        Some(EvalValue::Scalar(_)) => Err(ExcelError::Value),
        None => Err(ExcelError::Na),
    }
}

/// Flatten a range's cells row-major (used by SUM / SUMIF / single-column MATCH).
fn flatten(rows: &[Vec<CellValue>]) -> impl Iterator<Item = &CellValue> {
    rows.iter().flat_map(|r| r.iter())
}

/// Excel value-equality for lookup/criteria keys: numeric-aware for two numbers,
/// case-insensitive for two texts, else structural. An `Error` never matches.
fn values_equal(a: &CellValue, b: &CellValue) -> bool {
    match (a, b) {
        (CellValue::Number(x), CellValue::Number(y)) => x == y,
        (CellValue::Text(x), CellValue::Text(y)) => x.eq_ignore_ascii_case(y),
        (CellValue::Bool(x), CellValue::Bool(y)) => x == y,
        (CellValue::Empty, CellValue::Empty) => true,
        // Cross-type: a number matches a numeric text key (Excel coerces).
        (CellValue::Number(x), CellValue::Text(t)) | (CellValue::Text(t), CellValue::Number(x)) => {
            t.trim().parse::<f64>().map(|v| v == *x).unwrap_or(false)
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// The 13 whitelisted functions.
// ---------------------------------------------------------------------------

/// `IF(condition, then, else)` — returns the `then`/`else` branch by truthiness.
fn f_if(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let cond = arg_scalar(args, 0)?;
    Ok(if is_truthy(cond)? {
        branch(args, 1)
    } else {
        branch(args, 2)
    })
}

/// Return the scalar of a branch arg (the `then`/`else` of `IF`/`IFERROR`); a
/// missing `else` is Excel's `FALSE`-as-0 → we return `Empty`.
fn branch(args: &[EvalValue], i: usize) -> CellValue {
    match args.get(i) {
        Some(EvalValue::Scalar(cv)) => cv.clone(),
        Some(EvalValue::Range(_)) => CellValue::Error(ExcelError::Value),
        None => CellValue::Empty,
    }
}

/// `SUM(range, …)` — sums all numeric members of every range/scalar arg
/// (empty=0). An `Error` member propagates.
fn f_sum(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let mut total = 0.0;
    for a in args {
        match a {
            EvalValue::Scalar(cv) => total += to_number(cv)?,
            EvalValue::Range(rows) => {
                for cv in flatten(rows) {
                    match cv {
                        CellValue::Error(e) => return Err(*e),
                        // SUM ignores text/bool members the way Excel does for a
                        // range, but counts numbers and empties (as 0).
                        CellValue::Number(n) => total += n,
                        CellValue::Empty | CellValue::Text(_) | CellValue::Bool(_) => {}
                    }
                }
            }
        }
    }
    Ok(CellValue::Number(total))
}

/// `SUMIF(criteria_range, criteria, [sum_range])` — sums `sum_range` (or
/// `criteria_range` when omitted) where `criteria_range` matches `criteria`.
fn f_sumif(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let crit_range = arg_range(args, 0)?;
    let criteria = arg_scalar(args, 1)?;
    // Optional sum_range (3rd arg); default to the criteria range itself.
    let sum_range = match args.get(2) {
        Some(EvalValue::Range(r)) => r,
        Some(EvalValue::Scalar(_)) => return Err(ExcelError::Value),
        None => crit_range,
    };
    // Zip pairs criteria cells with sum cells positionally; the shorter range
    // bounds the walk (a criteria cell with no sum counterpart contributes 0).
    let mut total = 0.0;
    for (c, target) in flatten(crit_range).zip(flatten(sum_range)) {
        if values_equal(c, criteria) {
            total += to_number(target)?;
        }
    }
    Ok(CellValue::Number(total))
}

/// `VLOOKUP(lookup_value, table_array, col_index, [range_lookup])` — EXACT match
/// only (we require `FALSE`/0 for `range_lookup`; an approximate lookup is out of
/// scope). Returns the matched row's `col_index` (1-based) value; `#N/A` on miss.
fn f_vlookup(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let lookup = arg_scalar(args, 0)?;
    let table = arg_range(args, 1)?;
    let col = to_number(arg_scalar(args, 2)?)?;
    // 1-based column index, bounds-checked (T-09-13).
    if col < 1.0 || col.fract() != 0.0 {
        return Err(ExcelError::Value);
    }
    let col_idx = (col as usize) - 1;
    for row in table {
        if let Some(key) = row.first() {
            if values_equal(key, lookup) {
                return Ok(match row.get(col_idx) {
                    Some(cv) => cv.clone(),
                    None => CellValue::Error(ExcelError::Ref), // col beyond row width
                });
            }
        }
    }
    Ok(CellValue::Error(ExcelError::Na))
}

/// `INDEX(range, n)` — the 1-based `n`-th cell of a single-row/column range, or
/// `INDEX(range, row, col)` for a 2-D range. Bounds-checked → `#REF!`.
fn f_index(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let range = arg_range(args, 0)?;
    let n = to_number(arg_scalar(args, 1)?)?;
    if n < 1.0 || n.fract() != 0.0 {
        return Err(ExcelError::Value);
    }
    let n = n as usize;
    // 2-D form: INDEX(range, row, col).
    if let Some(EvalValue::Scalar(_)) = args.get(2) {
        let col = to_number(arg_scalar(args, 2)?)?;
        if col < 1.0 || col.fract() != 0.0 {
            return Err(ExcelError::Value);
        }
        return Ok(
            match range.get(n - 1).and_then(|r| r.get((col as usize) - 1)) {
                Some(cv) => cv.clone(),
                None => CellValue::Error(ExcelError::Ref),
            },
        );
    }
    // 1-D form: take the n-th cell row-major without materializing the range.
    Ok(match flatten(range).nth(n - 1) {
        Some(cv) => cv.clone(),
        None => CellValue::Error(ExcelError::Ref),
    })
}

/// `MATCH(lookup_value, range, [match_type])` — EXACT match only (we honour
/// `match_type == 0`); returns the 1-based position of `lookup_value` in the
/// (single-row/column) `range`; `#N/A` on miss.
fn f_match(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let lookup = arg_scalar(args, 0)?;
    let range = arg_range(args, 1)?;
    for (i, cv) in flatten(range).enumerate() {
        if values_equal(cv, lookup) {
            return Ok(CellValue::Number((i + 1) as f64)); // 1-based
        }
    }
    Ok(CellValue::Error(ExcelError::Na))
}

/// `ROUND(number, digits)` — half away from zero (via [`excel_round`]).
fn f_round(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    round_like(args, excel_round)
}

/// `ROUNDUP(number, digits)` — away from zero (via [`excel_roundup`]).
fn f_roundup(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    round_like(args, excel_roundup)
}

/// The inclusive `digits` magnitude Excel accepts for ROUND/ROUNDUP before it
/// returns `#NUM!` (`|digits|` beyond ~307 drives `pow10` to a non-finite
/// scale). WR-01 / CR-02: bound `digits` BEFORE the `as i32` cast (which would
/// otherwise saturate `1e20` to `i32::MAX`, making `pow10` `+inf` and leaking a
/// non-finite `CellValue::Number`).
const MAX_ROUND_DIGITS: f64 = 307.0;

/// Shared `ROUND`/`ROUNDUP` shape: `(number, digits)` with `digits` an integer.
fn round_like(args: &[EvalValue], f: fn(f64, i32) -> f64) -> Result<CellValue, ExcelError> {
    let x = to_number(arg_scalar(args, 0)?)?;
    let digits = to_number(arg_scalar(args, 1)?)?;
    if digits.fract() != 0.0 {
        return Err(ExcelError::Value);
    }
    // WR-01: reject an out-of-range `digits` BEFORE the saturating `as i32` cast.
    if digits.abs() > MAX_ROUND_DIGITS {
        return Err(ExcelError::Num);
    }
    // Defense in depth (CR-02 family): a non-finite result is a typed #NUM!,
    // never a leaked CellValue::Number(inf/NaN).
    let result = f(x, digits as i32);
    if result.is_finite() {
        Ok(CellValue::Number(result))
    } else {
        Ok(CellValue::Error(ExcelError::Num))
    }
}

/// `CEILING(number, significance)` — away-from-zero to a multiple of
/// `significance` (via [`excel_ceiling`]). A sign-mismatch yields `#NUM!`.
fn f_ceiling(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let number = to_number(arg_scalar(args, 0)?)?;
    let significance = to_number(arg_scalar(args, 1)?)?;
    let result = excel_ceiling(number, significance);
    // #NUM! for a sign mismatch (NaN) or any non-finite result (CR-02 family —
    // a non-finite value must never leak as CellValue::Number(inf/NaN)).
    if result.is_finite() {
        Ok(CellValue::Number(result))
    } else {
        Ok(CellValue::Error(ExcelError::Num))
    }
}

/// `IFERROR(value, value_if_error)` — inspects the FIRST arg's [`CellValue`]
/// DIRECTLY (D-04, never through eval): if it is an `Error`, returns the 2nd
/// arg, else the 1st.
fn f_iferror(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    Ok(match args.first() {
        Some(EvalValue::Scalar(CellValue::Error(_))) => branch(args, 1),
        Some(EvalValue::Scalar(cv)) => cv.clone(),
        // A range first arg: not an error scalar → return it is not meaningful
        // for this scaffold; treat a range as non-error and surface #VALUE!.
        Some(EvalValue::Range(_)) => CellValue::Error(ExcelError::Value),
        None => CellValue::Error(ExcelError::Na),
    })
}

/// `ISNUMBER(value)` — `Bool(matches!(scalar, Number))`, inspecting the
/// [`CellValue`] DIRECTLY (D-04). A range arg is `FALSE`.
fn f_isnumber(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    Ok(match args.first() {
        Some(EvalValue::Scalar(cv)) => CellValue::Bool(matches!(cv, CellValue::Number(_))),
        Some(EvalValue::Range(_)) => CellValue::Bool(false),
        None => CellValue::Error(ExcelError::Na),
    })
}

/// `SEARCH(find_text, within_text, [start_num])` — 1-based, CASE-INSENSITIVE
/// substring position; `#VALUE!` when not found.
fn f_search(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let needle = text_of(arg_scalar(args, 0)?)?;
    let haystack = text_of(arg_scalar(args, 1)?)?;
    // Optional 1-based start position.
    let start = match args.get(2) {
        Some(_) => {
            let v = to_number(arg_scalar(args, 2)?)?;
            if v >= 1.0 && v.fract() == 0.0 {
                (v as usize) - 1
            } else {
                return Err(ExcelError::Value);
            }
        }
        None => 0,
    };
    let hay_lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    // Search over CHARACTERS for a correct 1-based char position (not bytes).
    let hay_chars: Vec<char> = hay_lower.chars().collect();
    let needle_chars: Vec<char> = needle_lower.chars().collect();
    if start > hay_chars.len() {
        return Err(ExcelError::Value);
    }
    if needle_chars.is_empty() {
        return Ok(CellValue::Number((start + 1) as f64));
    }
    // A needle longer than the haystack can never match; guard BEFORE the loop
    // so the `hay_chars[i..i + needle_len]` slice can never overrun (T-09-13:
    // never a panic). Without this, `saturating_sub` floors the range end at 0
    // and `0..=0` would still slice `[0..needle_len]` out of bounds.
    if needle_chars.len() > hay_chars.len() {
        return Err(ExcelError::Value);
    }
    for i in start..=hay_chars.len() - needle_chars.len() {
        if hay_chars[i..i + needle_chars.len()] == needle_chars[..] {
            return Ok(CellValue::Number((i + 1) as f64)); // 1-based
        }
    }
    Err(ExcelError::Value)
}

/// Coerce a scalar to its text representation for `SEARCH`/`TEXT`.
fn text_of(cv: &CellValue) -> Result<String, ExcelError> {
    match cv {
        CellValue::Text(s) => Ok(s.clone()),
        CellValue::Number(n) => Ok(format_plain_number(*n)),
        CellValue::Bool(b) => Ok(if *b { "TRUE".into() } else { "FALSE".into() }),
        CellValue::Empty => Ok(String::new()),
        CellValue::Error(e) => Err(*e),
    }
}

/// `TEXT(value, format_text)` — formats a number per the format string. This
/// scaffold supports the lighthouse `£#,##0.00` thousands+2dp lighthouse
/// pattern (and the generic `#,##0.00` / `0.00` shapes); other formats fall
/// back to a plain decimal render.
fn f_text(args: &[EvalValue]) -> Result<CellValue, ExcelError> {
    let value = to_number(arg_scalar(args, 0)?)?;
    let format = match arg_scalar(args, 1)? {
        CellValue::Text(s) => s.clone(),
        CellValue::Empty => String::new(),
        _ => return Err(ExcelError::Value),
    };
    Ok(CellValue::Text(format_number(value, &format)))
}

/// Format `value` per a (supported) Excel number `format`. Handles an optional
/// leading currency prefix (e.g. `£`/`$`), a thousands separator (`#,##0`), and
/// a fixed number of decimals (`.00`). Half-away-from-zero rounding to the
/// decimal count via [`excel_round`].
fn format_number(value: f64, format: &str) -> String {
    // Currency prefix: any leading non-`#0,.` characters (e.g. "£", "$").
    let prefix: String = format
        .chars()
        .take_while(|c| !matches!(c, '#' | '0' | ',' | '.'))
        .collect();
    let body = &format[prefix.len()..];
    let thousands = body.contains(',');
    let decimals = body
        .split_once('.')
        .map(|(_, frac)| frac.chars().filter(|c| *c == '0' || *c == '#').count())
        .unwrap_or(0) as i32;

    let rounded = excel_round(value, decimals);
    let negative = rounded < 0.0;
    let abs = rounded.abs();

    let int_part = abs.trunc() as i64;
    let int_str = if thousands {
        group_thousands(int_part)
    } else {
        int_part.to_string()
    };

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&prefix);
    out.push_str(&int_str);
    if decimals > 0 {
        let scale = 10f64.powi(decimals);
        let frac = ((abs - abs.trunc()) * scale).round() as i64;
        out.push('.');
        out.push_str(&format!("{:0width$}", frac, width = decimals as usize));
    }
    out
}

/// Group an integer's digits with commas (`1594` → `1,594`).
fn group_thousands(n: i64) -> String {
    let digits = n.abs().to_string();
    let bytes = digits.as_bytes();
    let mut out = String::new();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

/// Render a number plainly (no thousands/format) for `SEARCH`/text coercion.
fn format_plain_number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        // A stable decimal render.
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(cv: CellValue) -> EvalValue {
        EvalValue::Scalar(cv)
    }
    fn num(n: f64) -> EvalValue {
        EvalValue::Scalar(CellValue::Number(n))
    }
    fn text(s: &str) -> EvalValue {
        EvalValue::Scalar(CellValue::Text(s.to_string()))
    }
    fn col(values: &[CellValue]) -> EvalValue {
        EvalValue::Range(values.iter().map(|v| vec![v.clone()]).collect())
    }

    #[test]
    fn if_branches_on_truthiness() {
        assert_eq!(
            apply("IF", &[scalar(CellValue::Bool(true)), num(1.0), num(2.0)]),
            CellValue::Number(1.0)
        );
        assert_eq!(
            apply("IF", &[scalar(CellValue::Bool(false)), num(1.0), num(2.0)]),
            CellValue::Number(2.0)
        );
        // An error condition propagates.
        assert_eq!(
            apply(
                "IF",
                &[
                    scalar(CellValue::Error(ExcelError::Ref)),
                    num(1.0),
                    num(2.0)
                ]
            ),
            CellValue::Error(ExcelError::Ref)
        );
    }

    #[test]
    fn sum_sums_a_range_with_empty_as_zero() {
        let r = col(&[
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Number(5.0),
        ]);
        assert_eq!(apply("SUM", &[r]), CellValue::Number(15.0));
    }

    #[test]
    fn sum_propagates_an_error_member() {
        let r = col(&[
            CellValue::Number(1.0),
            CellValue::Error(ExcelError::DivZero),
        ]);
        assert_eq!(apply("SUM", &[r]), CellValue::Error(ExcelError::DivZero));
    }

    #[test]
    fn sumif_sums_where_criteria_matches() {
        // criteria_range vs sum_range, both 3 rows.
        let crit = col(&[
            CellValue::Text("A".into()),
            CellValue::Text("B".into()),
            CellValue::Text("A".into()),
        ]);
        let sums = col(&[
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(5.0),
        ]);
        let out = apply("SUMIF", &[crit, text("A"), sums]);
        assert_eq!(out, CellValue::Number(15.0));
    }

    #[test]
    fn vlookup_exact_match_and_na_miss() {
        // table_array: 3 rows × 2 cols (key, value).
        let table = EvalValue::Range(vec![
            vec![CellValue::Text("X".into()), CellValue::Number(100.0)],
            vec![CellValue::Text("Y".into()), CellValue::Number(200.0)],
        ]);
        assert_eq!(
            apply(
                "VLOOKUP",
                &[
                    text("Y"),
                    table.clone(),
                    num(2.0),
                    scalar(CellValue::Bool(false))
                ]
            ),
            CellValue::Number(200.0)
        );
        assert_eq!(
            apply(
                "VLOOKUP",
                &[text("Z"), table, num(2.0), scalar(CellValue::Bool(false))]
            ),
            CellValue::Error(ExcelError::Na)
        );
    }

    #[test]
    fn index_and_match_are_one_based() {
        let r = col(&[
            CellValue::Text("a".into()),
            CellValue::Text("b".into()),
            CellValue::Text("c".into()),
        ]);
        // MATCH("b", range, 0) == 2 (1-based)
        assert_eq!(
            apply("MATCH", &[text("b"), r.clone(), num(0.0)]),
            CellValue::Number(2.0)
        );
        // INDEX(range, 1) == first element (1-based)
        assert_eq!(
            apply("INDEX", &[r.clone(), num(1.0)]),
            CellValue::Text("a".into())
        );
        // out-of-bounds → #REF!, never a panic (T-09-13)
        assert_eq!(
            apply("INDEX", &[r, num(9.0)]),
            CellValue::Error(ExcelError::Ref)
        );
    }

    #[test]
    fn match_miss_is_na() {
        let r = col(&[CellValue::Number(1.0), CellValue::Number(2.0)]);
        assert_eq!(
            apply("MATCH", &[num(9.0), r, num(0.0)]),
            CellValue::Error(ExcelError::Na)
        );
    }

    #[test]
    fn round_and_roundup_delegate_to_rounding() {
        assert_eq!(
            apply("ROUND", &[num(1594.925), num(2.0)]),
            CellValue::Number(1594.93)
        );
        assert_eq!(
            apply("ROUNDUP", &[num(3.001), num(2.0)]),
            CellValue::Number(3.01)
        );
    }

    #[test]
    fn round_with_out_of_range_digits_is_num_not_non_finite() {
        // WR-01 / CR-02: a huge `digits` would saturate `as i32` to i32::MAX,
        // make pow10 +inf, and leak a non-finite Number. It must be #NUM! instead.
        assert_eq!(
            apply("ROUND", &[num(1.5), num(1e20)]),
            CellValue::Error(ExcelError::Num)
        );
        assert_eq!(
            apply("ROUNDUP", &[num(1.5), num(-1e20)]),
            CellValue::Error(ExcelError::Num)
        );
        // The boundary (±307) still rounds normally.
        assert!(matches!(
            apply("ROUND", &[num(1.5), num(307.0)]),
            CellValue::Number(_)
        ));
    }

    #[test]
    fn ceiling_coil_band_rounds_up_to_next_50() {
        // CEILING(req*1.05, 50): req=666 → 699.3 → 700.
        let req = 666.0_f64;
        assert_eq!(
            apply("CEILING", &[num(req * 1.05), num(50.0)]),
            CellValue::Number(700.0)
        );
    }

    #[test]
    fn iferror_inspects_cellvalue_directly() {
        assert_eq!(
            apply(
                "IFERROR",
                &[scalar(CellValue::Error(ExcelError::Na)), num(7.0)]
            ),
            CellValue::Number(7.0)
        );
        assert_eq!(
            apply("IFERROR", &[num(3.0), num(7.0)]),
            CellValue::Number(3.0)
        );
    }

    #[test]
    fn isnumber_on_each_variant() {
        assert_eq!(apply("ISNUMBER", &[num(1.0)]), CellValue::Bool(true));
        assert_eq!(apply("ISNUMBER", &[text("x")]), CellValue::Bool(false));
        assert_eq!(
            apply("ISNUMBER", &[scalar(CellValue::Bool(true))]),
            CellValue::Bool(false)
        );
        assert_eq!(
            apply("ISNUMBER", &[scalar(CellValue::Error(ExcelError::Na))]),
            CellValue::Bool(false)
        );
        assert_eq!(
            apply("ISNUMBER", &[scalar(CellValue::Empty)]),
            CellValue::Bool(false)
        );
    }

    #[test]
    fn search_hit_and_miss_case_insensitive() {
        // 1-based, case-insensitive.
        assert_eq!(
            apply("SEARCH", &[text("world"), text("Hello World")]),
            CellValue::Number(7.0)
        );
        assert_eq!(
            apply("SEARCH", &[text("xyz"), text("Hello World")]),
            CellValue::Error(ExcelError::Value)
        );
        // A needle longer than the haystack must be a #VALUE! miss, NOT a panic
        // (T-09-13): the slice index `[0..needle_len]` would otherwise overrun.
        assert_eq!(
            apply("SEARCH", &[text("abc"), text("ab")]),
            CellValue::Error(ExcelError::Value)
        );
    }

    #[test]
    fn text_formats_the_lighthouse_currency_pattern() {
        assert_eq!(
            apply("TEXT", &[num(1594.93), text("£#,##0.00")]),
            CellValue::Text("£1,594.93".into())
        );
    }

    #[test]
    fn unknown_name_is_name_error() {
        assert_eq!(
            apply("OFFSET", &[num(1.0)]),
            CellValue::Error(ExcelError::Name)
        );
    }
}
