//! Decimal conversion utilities.
//!
//! Canonical `f64 ↔ Decimal` helpers that propagate errors on non-finite or
//! unrepresentable values rather than silently collapsing to zero. All
//! production code that converts raw `f64` user input to `Decimal` (or back)
//! should use these helpers; trusted literals in tests and examples can use
//! `Decimal::from_f64_retain(...)` or the `dec!` macro.

use crate::{Error, InputError, NonFiniteKind, Result};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

/// Parse exact fixed-point or scientific decimal text without rounding.
///
/// Leading and trailing whitespace is ignored. The mantissa must fit a
/// 96-bit Decimal coefficient and a scale of at most 28; applying a scientific
/// exponent may remove trailing zeroes, but never a nonzero digit.
///
/// # Arguments
///
/// * `amount` - Finite decimal text such as `"1234.56"` or `"1.2345e3"`, in
///   the caller's quantity units. No currency rounding is applied.
///
/// # Errors
///
/// Returns [`Error::Validation`] for malformed or non-finite input, or when
/// the value cannot be represented exactly as [`Decimal`].
///
/// # Examples
///
/// ```
/// use finstack_quant_core::decimal::parse_decimal;
/// assert_eq!(parse_decimal("1.2345e3")?.to_string(), "1234.5");
/// assert!(parse_decimal("1e-29").is_err());
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn parse_decimal(amount: &str) -> Result<Decimal> {
    let amount = amount.trim();
    let invalid = |error| {
        Error::Validation(format!(
            "decimal value must be exactly representable as Decimal: {error}"
        ))
    };
    let out_of_range =
        || Error::Validation("decimal value must be exactly representable as Decimal".to_string());
    let decimal = if let Some((mantissa, exponent)) = amount.split_once(['e', 'E']) {
        let parsed = Decimal::from_str_exact(mantissa).map_err(invalid)?;
        let exponent = exponent
            .parse::<i64>()
            .map_err(|error| Error::Validation(format!("invalid scientific exponent: {error}")))?;
        let mut coefficient = parsed.mantissa();
        if coefficient == 0 {
            Decimal::ZERO
        } else {
            let mut scale = i64::from(parsed.scale())
                .checked_sub(exponent)
                .ok_or_else(out_of_range)?;
            while scale > i64::from(Decimal::MAX_SCALE) && coefficient % 10 == 0 {
                coefficient /= 10;
                scale -= 1;
            }
            let scale = if scale < 0 {
                let shift = u32::try_from(scale.unsigned_abs()).map_err(|_| out_of_range())?;
                if shift > Decimal::MAX_SCALE {
                    return Err(out_of_range());
                }
                coefficient = coefficient
                    .checked_mul(10_i128.pow(shift))
                    .ok_or_else(out_of_range)?;
                0
            } else {
                u32::try_from(scale)
                    .ok()
                    .filter(|scale| *scale <= Decimal::MAX_SCALE)
                    .ok_or_else(out_of_range)?
            };
            Decimal::try_from_i128_with_scale(coefficient, scale).map_err(invalid)?
        }
    } else {
        Decimal::from_str_exact(amount).map_err(invalid)?
    };
    Ok(decimal)
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

/// Convert a [`Decimal`] to `f64`, returning an error if conversion fails.
///
/// While `Decimal` values are always finite, conversion to `f64` can fail for
/// very large magnitudes that exceed `f64`'s representable range
/// (~1.8 × 10^308).
///
/// # Arguments
///
/// * `value` - Decimal quantity to convert to the nearest representable `f64`.
///
/// # Errors
///
/// Returns [`InputError::ConversionOverflow`] when the value cannot be
/// represented as `f64`.
pub fn decimal_to_f64(value: Decimal) -> Result<f64> {
    value
        .to_f64()
        .ok_or_else(|| Error::from(InputError::ConversionOverflow))
}

/// Convert a [`Decimal`] to `f64` only when its exact value is representable.
///
/// Decimal fractions with a factor of five in the reduced denominator, such
/// as `0.1`, cannot be represented exactly by binary floating-point numbers.
/// Integers and binary fractions must also fit the 53-bit `f64` significand
/// after removing trailing powers of two.
///
/// # Arguments
///
/// * `value` - Finite decimal quantity in the caller's units; conversion must
///   preserve its exact numeric value without rounding.
///
/// # Errors
///
/// Returns [`Error::Validation`] if conversion would lose precision.
pub fn decimal_to_f64_exact(value: Decimal) -> Result<f64> {
    let inexact =
        || Error::Validation("Decimal value must be exactly representable as float".into());
    let mut coefficient = value.mantissa().unsigned_abs();
    if coefficient == 0 {
        return Ok(if value.is_sign_negative() { -0.0 } else { 0.0 });
    }

    // Decimal is coefficient / (2^scale * 5^scale). All factors of five in
    // the denominator must cancel for a finite binary representation.
    for _ in 0..value.scale() {
        if !coefficient.is_multiple_of(5) {
            return Err(inexact());
        }
        coefficient /= 5;
    }
    let trailing_zero_bits = coefficient.trailing_zeros();
    coefficient >>= trailing_zero_bits;
    if u128::BITS - coefficient.leading_zeros() > f64::MANTISSA_DIGITS {
        return Err(inexact());
    }

    // The reduced coefficient has at most 53 bits, so this cast is exact.
    // Decimal has a 96-bit coefficient and a scale of at most 28, so the
    // binary exponent fits i32 and the result is normal and finite. Scaling
    // this exact significand by a power of two introduces no rounding.
    let binary_exponent = trailing_zero_bits as i32 - value.scale() as i32;
    let magnitude = coefficient as f64 * 2.0_f64.powi(binary_exponent);
    Ok(if value.is_sign_negative() {
        -magnitude
    } else {
        magnitude
    })
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
    fn decimal_to_f64_roundtrips_typical_values() {
        let d = f64_to_decimal(0.0525).expect("convert");
        let f = decimal_to_f64(d).expect("convert back");
        assert!((f - 0.0525).abs() < 1e-12);
    }

    #[test]
    fn exact_decimal_input_preserves_precision_before_float_validation() {
        assert!(parse_decimal("0.50000000000000000000000000001").is_err());
        assert_eq!(
            parse_decimal(" 2000e-31 ").expect("exact exponent"),
            Decimal::new(2, 28)
        );
        assert_eq!(
            decimal_to_f64_exact(parse_decimal("0.5000").expect("decimal"))
                .expect("exact binary value"),
            0.5
        );
        assert!(decimal_to_f64_exact(parse_decimal("0.1").expect("decimal")).is_err());
    }

    #[test]
    fn exact_decimal_to_float_rejects_tiny_nonbinary_fractions() {
        for text in ["1e-20", "1e-28", "2e-28", "3e-28", "-1e-28"] {
            let value = parse_decimal(text).expect("exact Decimal value");
            assert!(
                matches!(decimal_to_f64_exact(value), Err(Error::Validation(_))),
                "{text} has an uncancelled factor of five in its denominator"
            );
        }
    }

    #[test]
    fn exact_decimal_to_float_accepts_binary_fractions_at_maximum_scale() {
        for multiplier in [1, 3, 123_456_789] {
            let value = Decimal::try_from_i128_with_scale(multiplier * 5_i128.pow(28), 28)
                .expect("96-bit coefficient at maximum Decimal scale");
            let expected = multiplier as f64 * 2.0_f64.powi(-28);
            assert_eq!(decimal_to_f64_exact(value).expect("exact dyadic"), expected);
            assert_eq!(
                decimal_to_f64_exact(-value).expect("exact dyadic"),
                -expected
            );
        }
    }

    #[test]
    fn exact_decimal_to_float_checks_significant_bits_in_large_integers() {
        for exponent in [53, 54, 90, 95] {
            let integer = 1_i128 << exponent;
            let decimal =
                Decimal::try_from_i128_with_scale(integer, 0).expect("96-bit power of two");
            assert_eq!(
                decimal_to_f64_exact(decimal).expect("exact power of two"),
                2.0_f64.powi(exponent)
            );
            assert_eq!(
                decimal_to_f64_exact(-decimal).expect("exact power of two"),
                -2.0_f64.powi(exponent)
            );
        }
        for integer in [(1_i128 << 53) + 1, (1_i128 << 90) + 1] {
            let decimal = Decimal::try_from_i128_with_scale(integer, 0).expect("96-bit integer");
            assert!(matches!(
                decimal_to_f64_exact(decimal),
                Err(Error::Validation(_))
            ));
            assert!(matches!(
                decimal_to_f64_exact(-decimal),
                Err(Error::Validation(_))
            ));
        }
        let exact =
            Decimal::try_from_i128_with_scale((1_i128 << 53) + 2, 0).expect("96-bit integer");
        assert_eq!(
            decimal_to_f64_exact(exact).expect("53 significant bits"),
            9_007_199_254_740_994.0
        );
        assert_eq!(decimal_to_f64_exact(Decimal::ZERO).expect("zero"), 0.0);
    }
}
