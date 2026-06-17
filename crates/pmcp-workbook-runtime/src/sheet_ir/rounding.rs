//! Deterministic, decimal-aware Excel rounding helpers (finding #10).
//!
//! Naive `(x * 10f64.powi(d)).round() / 10f64.powi(d)` is NOT enough: the half
//! case (`1594.925`) is stored in binary `f64` as a value slightly BELOW
//! `1594.925`, so a naive scale-then-`round` yields `1594.92`, not Excel's
//! `1594.93`. These helpers apply a small epsilon correction at the rounding
//! boundary so the documented Excel half-away-from-zero behaviour is stable, and
//! they implement Excel's away-from-zero rules for ROUNDUP and CEILING so
//! NEGATIVE inputs are correct.
//!
//! - [`excel_round`] — round half away from zero to `digits` decimals.
//! - [`excel_roundup`] — round AWAY FROM ZERO to `digits` decimals (Excel ROUNDUP).
//! - [`excel_ceiling`] — round AWAY FROM ZERO to the nearest multiple of
//!   `significance` (Excel CEILING, magnitude rule).

/// The relative epsilon applied at the rounding boundary to undo binary-`f64`
/// representation error for the documented decimal half cases (e.g. `1594.925`
/// stored just under its decimal value). Scaled by the magnitude of the value.
const ROUND_EPSILON: f64 = 1e-9;

/// `10^digits` as `f64`, supporting negative `digits` (round to tens/hundreds).
fn pow10(digits: i32) -> f64 {
    10f64.powi(digits)
}

/// Excel `ROUND(x, digits)` — round half AWAY FROM ZERO to `digits` decimals.
///
/// A non-finite input passes through unchanged (the caller maps non-finite to an
/// Excel error above this layer).
pub fn excel_round(x: f64, digits: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let factor = pow10(digits);
    let scaled = x * factor;
    // Nudge by a magnitude-scaled epsilon toward away-from-zero so a decimal
    // half that binary-f64 stores just under its true value still rounds up.
    let nudged = scaled + scaled.signum() * (scaled.abs() * ROUND_EPSILON);
    // round() in Rust is already half-away-from-zero.
    nudged.round() / factor
}

/// Excel `ROUNDUP(x, digits)` — round AWAY FROM ZERO to `digits` decimals.
///
/// `ROUNDUP(3.001, 2) == 3.01`; `ROUNDUP(-3.001, 2) == -3.01` (magnitude grows,
/// sign preserved). A non-finite input passes through unchanged.
pub fn excel_roundup(x: f64, digits: i32) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let factor = pow10(digits);
    let scaled = x * factor;
    // Away-from-zero: ceil the magnitude. Apply an epsilon PULL toward zero so a
    // value that is exactly representable (or a hair over due to f64 error) is
    // not spuriously bumped to the next integer.
    let pulled = scaled - scaled.signum() * (scaled.abs() * ROUND_EPSILON);
    let away = if pulled >= 0.0 {
        pulled.ceil()
    } else {
        pulled.floor()
    };
    away / factor
}

/// Excel `CEILING(number, significance)` — round `number` AWAY FROM ZERO to the
/// nearest multiple of `significance` (Excel's magnitude rule).
///
/// - `CEILING(10, 3) == 12`.
/// - `CEILING(-10, -3) == -12` (negative number, negative significance →
///   away-from-zero magnitude).
/// - `significance == 0` → `0` (Excel returns 0 for a zero significance).
/// - A `number`/`significance` sign mismatch returns `NaN` (Excel's `#NUM!`
///   case — the caller maps non-finite to an Excel error above this layer).
pub fn excel_ceiling(number: f64, significance: f64) -> f64 {
    if !number.is_finite() || !significance.is_finite() {
        return f64::NAN;
    }
    if significance == 0.0 {
        return 0.0;
    }
    // Excel: a positive number with a negative significance (or vice versa) is a
    // #NUM! error — signal it as NaN for the caller to map.
    if number != 0.0 && number.signum() != significance.signum() {
        return f64::NAN;
    }
    let ratio = number / significance;
    // Away-from-zero on the multiple count, with a small epsilon pull so an
    // exact multiple is not bumped to the next one by f64 error.
    let pulled = ratio - ratio.signum() * (ratio.abs() * ROUND_EPSILON);
    let steps = if pulled >= 0.0 {
        pulled.ceil()
    } else {
        pulled.floor()
    };
    steps * significance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_half_away_from_zero_decimal_boundary() {
        // The lighthouse golden boundary: 1594.925 → 1594.93 (NOT 1594.92).
        assert_eq!(excel_round(1594.925, 2), 1594.93);
        assert_eq!(excel_round(2.5, 0), 3.0);
        assert_eq!(excel_round(-2.5, 0), -3.0); // half away from zero
        assert_eq!(excel_round(2.4, 0), 2.0);
    }

    #[test]
    fn round_to_negative_digits() {
        assert_eq!(excel_round(1234.0, -2), 1200.0);
        assert_eq!(excel_round(1250.0, -2), 1300.0);
    }

    #[test]
    fn roundup_is_away_from_zero() {
        assert_eq!(excel_roundup(3.001, 2), 3.01);
        assert_eq!(excel_roundup(-3.001, 2), -3.01);
        assert_eq!(excel_roundup(3.0, 2), 3.0); // exact multiple not bumped
        assert_eq!(excel_roundup(0.0, 2), 0.0);
    }

    #[test]
    fn ceiling_positive_rounds_up_to_multiple() {
        assert_eq!(excel_ceiling(10.0, 3.0), 12.0);
        assert_eq!(excel_ceiling(12.0, 3.0), 12.0); // exact multiple unchanged
                                                    // The coil-band CEILING(req*1.05, 50) lands on the next 50.
        let req = 666.0_f64; // req*1.05 = 699.3
        assert_eq!(excel_ceiling(req * 1.05, 50.0), 700.0);
    }

    #[test]
    fn ceiling_negative_magnitude_away_from_zero() {
        // CEILING(-10, -3) == -12 (Excel away-from-zero magnitude rule).
        assert_eq!(excel_ceiling(-10.0, -3.0), -12.0);
        assert_eq!(excel_ceiling(-12.0, -3.0), -12.0);
    }

    #[test]
    fn ceiling_zero_significance_is_zero() {
        assert_eq!(excel_ceiling(10.0, 0.0), 0.0);
    }

    #[test]
    fn ceiling_sign_mismatch_is_nan() {
        assert!(excel_ceiling(10.0, -3.0).is_nan());
        assert!(excel_ceiling(-10.0, 3.0).is_nan());
    }
}
