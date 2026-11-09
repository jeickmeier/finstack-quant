//! Decimal conversion utilities.
//!
//! Decimal-text parsing and the `f64 -> Decimal` conversion. Both propagate
//! errors on non-finite or unrepresentable values rather than silently
//! collapsing to zero. All production code that converts raw `f64` user input
//! or decimal text to `Decimal` should use these helpers; trusted literals in
//! tests and examples can use
//! `Decimal::from_f64_retain(...)` or the `dec!` macro.

use crate::{Error, InputError, NonFiniteKind, Result};
use rust_decimal::Decimal;

/// Largest Decimal coefficient magnitude, `2^96 - 1`.
const MAX_COEFFICIENT: u128 = (1_u128 << 96) - 1;

/// Parse exact fixed-point or scientific decimal text without rounding.
///
/// After outer whitespace trimming, the text must match
/// `[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?`. Internal
/// whitespace, separators, underscores, other bases and non-finite words are
/// rejected, and the exponent must fit `i64` even for a zero value. The
/// exponent is applied and redundant trailing zeros are removed before the
/// final value is tested against Decimal's domain (coefficient magnitude at
/// most `2^96 - 1`, scale `0..=28`), so an intermediate spelling never needs
/// to fit on its own. A value is accepted only if it is represented exactly;
/// any nonzero digit loss, out-of-range magnitude or scale is an error. The
/// written scale is kept when it fits. A lexical zero is zero (positive).
///
/// # Arguments
///
/// * `amount` - Finite decimal text such as `"1234.56"` or `"1.2345e3"`, in
///   the caller's quantity units. No currency rounding is applied.
///
/// # Errors
///
/// Returns [`Error::Validation`] for malformed input or when the value cannot
/// be represented exactly as [`Decimal`].
///
/// # Examples
///
/// ```
/// use finstack_quant_core::decimal::parse_decimal;
/// assert_eq!(parse_decimal("1.2345e3")?.to_string(), "1234.5");
/// assert_eq!(parse_decimal("100000000000000000000000000000e-1")?.to_string(), "10000000000000000000000000000");
/// assert!(parse_decimal("1e-29").is_err());
/// assert!(parse_decimal("1_000").is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn parse_decimal(amount: &str) -> Result<Decimal> {
    let text = amount.trim();
    let malformed = || {
        Error::Validation(format!(
            "decimal value must be exactly representable as Decimal: invalid decimal {amount:?}, expected [+-]digits[.digits][e[+-]digits]"
        ))
    };
    let unrepresentable = || {
        Error::Validation(format!(
            "decimal {amount:?} is not exactly representable as Decimal (coefficient up to 2^96-1, scale up to 28)"
        ))
    };
    let bytes = text.as_bytes();
    let mut index = 0;
    let negative = match bytes.first() {
        Some(b'+') => {
            index = 1;
            false
        }
        Some(b'-') => {
            index = 1;
            true
        }
        _ => false,
    };
    let int_start = index;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    let int_digits = &text[int_start..index];
    let mut frac_digits = "";
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        let frac_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        frac_digits = &text[frac_start..index];
    }
    if int_digits.is_empty() && frac_digits.is_empty() {
        return Err(malformed());
    }
    let mut exponent: i64 = 0;
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        index += 1;
        let exp_start = index;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let digits_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == digits_start {
            return Err(malformed());
        }
        exponent = text[exp_start..index]
            .parse::<i64>()
            .map_err(|_| malformed())?;
    }
    if index != bytes.len() {
        return Err(malformed());
    }

    // value = digits * 10^(exponent - frac_len); strip leading zeros.
    let digits = format!("{int_digits}{frac_digits}");
    let significant = digits.trim_start_matches('0');
    if significant.is_empty() {
        return Ok(Decimal::ZERO);
    }
    let frac_len = i128::try_from(frac_digits.len()).map_err(|_| unrepresentable())?;
    let power = i128::from(exponent) - frac_len;

    // Try the written spelling first so its scale is kept; only when it does
    // not fit, remove trailing zeros (which never loses a nonzero digit).
    let attempt = |coefficient_digits: &str, power: i128| -> Option<Decimal> {
        if coefficient_digits.len() > 29 {
            return None;
        }
        let mut coefficient: u128 = coefficient_digits.parse().ok()?;
        let scale = if power >= 0 {
            let shift = u32::try_from(power).ok()?;
            coefficient = coefficient.checked_mul(10_u128.checked_pow(shift)?)?;
            0
        } else {
            let scale = u32::try_from(-power).ok()?;
            if scale > Decimal::MAX_SCALE {
                return None;
            }
            scale
        };
        if coefficient > MAX_COEFFICIENT {
            return None;
        }
        let signed = i128::try_from(coefficient).ok()?;
        Decimal::try_from_i128_with_scale(if negative { -signed } else { signed }, scale).ok()
    };
    if let Some(decimal) = attempt(significant, power) {
        return Ok(decimal);
    }
    let trimmed = significant.trim_end_matches('0');
    let removed =
        i128::try_from(significant.len() - trimmed.len()).map_err(|_| unrepresentable())?;
    attempt(trimmed, power + removed).ok_or_else(unrepresentable)
}

/// Convert an `f64` to [`Decimal`], returning an error for non-finite values.
///
/// This prevents silent masking of `NaN`/`Infinity` values as zero, which would
/// result in zero rates, strikes, or spreads that materially misprice
/// instruments.
///
/// # Arguments
///
/// * `value` - Finite floating-point amount, rate, price, or other decimal
///   quantity to represent exactly where possible.
///
/// # Errors
///
/// Returns [`InputError::NonFiniteValue`] for `NaN`, `+inf`, or `-inf`.
/// Returns [`InputError::ConversionOverflow`] when the finite value cannot be
/// represented as `Decimal` (extremely large `f64` magnitude).
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::decimal::f64_to_decimal;
///
/// assert!(f64_to_decimal(0.05).is_ok());
/// assert!(f64_to_decimal(f64::NAN).is_err());
/// assert!(f64_to_decimal(f64::INFINITY).is_err());
/// ```
pub fn f64_to_decimal(value: f64) -> Result<Decimal> {
    if !value.is_finite() {
        return Err(InputError::NonFiniteValue {
            kind: NonFiniteKind::classify(value),
        }
        .into());
    }
    Decimal::try_from(value).map_err(|_| Error::from(InputError::ConversionOverflow))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_to_decimal_accepts_typical_financial_values() {
        let decimal = f64_to_decimal(0.0525).expect("finite rate should convert");
        assert!(decimal > Decimal::ZERO);
    }

    #[test]
    fn f64_to_decimal_rejects_nan() {
        let err = f64_to_decimal(f64::NAN);
        assert!(matches!(
            err,
            Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::NaN
            }))
        ));
    }

    #[test]
    fn f64_to_decimal_rejects_positive_and_negative_infinity() {
        assert!(matches!(
            f64_to_decimal(f64::INFINITY),
            Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::PosInfinity
            }))
        ));
        assert!(matches!(
            f64_to_decimal(f64::NEG_INFINITY),
            Err(Error::Input(InputError::NonFiniteValue {
                kind: NonFiniteKind::NegInfinity
            }))
        ));
    }

    #[test]
    fn f64_to_decimal_rejects_unrepresentable_magnitude() {
        assert!(matches!(
            f64_to_decimal(1e100),
            Err(Error::Input(InputError::ConversionOverflow))
        ));
    }

    #[test]
    fn exact_decimal_input_preserves_precision_before_float_validation() {
        assert!(parse_decimal("0.50000000000000000000000000001").is_err());
        assert_eq!(
            parse_decimal(" 2000e-31 ").expect("exact exponent"),
            Decimal::new(2, 28)
        );
    }

    #[test]
    fn exact_grammar_accepts_only_the_selected_syntax() {
        for (text, expected) in [
            ("1.50", Decimal::new(150, 2)),
            ("+1", Decimal::ONE),
            (".5", Decimal::new(5, 1)),
            ("5.", Decimal::new(5, 0)),
            ("-0.000", Decimal::ZERO),
            ("0e-99999", Decimal::ZERO),
            ("1E2", Decimal::new(100, 0)),
        ] {
            let parsed = parse_decimal(text).unwrap_or_else(|e| panic!("{text}: {e}"));
            assert_eq!(parsed, expected, "{text}");
        }
        assert!(parse_decimal("-0").expect("zero").is_sign_positive());
        for bad in [
            "",
            "1_000",
            "1,000",
            "1 0",
            "0x10",
            "NaN",
            "inf",
            "1e",
            "1e+",
            ".",
            "+",
            "1e1.5",
            "0e99999999999999999999",
            "--1",
            "1.2.3",
        ] {
            assert!(parse_decimal(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn final_value_domain_is_tested_after_the_exponent() {
        // Mantissa longer than 29 digits that normalizes into range.
        assert_eq!(
            parse_decimal("100000000000000000000000000000e-2").expect("normalizes"),
            Decimal::from_i128_with_scale(1_000_000_000_000_000_000_000_000_000, 0)
        );
        // 2^96 - 1 is the largest coefficient; one more is out of range.
        assert!(parse_decimal("79228162514264337593543950335").is_ok());
        assert!(parse_decimal("79228162514264337593543950336").is_err());
        assert!(parse_decimal("1e29").is_err());
        assert!(parse_decimal("1e-28").is_ok());
        assert!(parse_decimal("1e-29").is_err());
        assert!(parse_decimal("0.10000000000000000000000000001").is_err());
        assert!(parse_decimal("2.5e-28").is_err());
    }
}
