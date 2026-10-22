//! Internal rounding helpers backing `Money` arithmetic.
//!
//! Amounts are stored as `Decimal` values to provide accounting-grade
//! precision and deterministic arithmetic. The routines here provide Decimal arithmetic
//! on that representation and expose helpers that honour [`RoundingMode`](crate::config::RoundingMode).
//!
//! The rounding functions are used internally by `Money` operations and
//! are not part of the public API. For rounding examples, see `Money` documentation.

use crate::config::RoundingMode;
use crate::decimal::f64_to_decimal;
use crate::{Error, InputError};
use rust_decimal::Decimal;

#[inline]
pub(crate) fn repr_add(a: Decimal, b: Decimal) -> Result<Decimal, Error> {
    a.checked_add(b)
        .ok_or_else(|| InputError::ConversionOverflow.into())
}

#[inline]
pub(crate) fn repr_sub(a: Decimal, b: Decimal) -> Result<Decimal, Error> {
    a.checked_sub(b)
        .ok_or_else(|| InputError::ConversionOverflow.into())
}

/// Fallible multiplication by an `f64` scalar (no silent substitution).
#[inline]
pub(crate) fn repr_mul_f64(a: Decimal, rhs: f64) -> Result<Decimal, Error> {
    let rhs_decimal = f64_to_decimal(rhs)?;
    // Checked Decimal multiply: a finite, representable scalar can still drive
    // the product past Decimal's range (~7.9e28). Surface that as an error so
    // `checked_mul_f64` is genuinely panic-free, rather than relying on the
    // panicking `*` operator.
    a.checked_mul(rhs_decimal)
        .ok_or_else(|| InputError::ConversionOverflow.into())
}

/// Fallible division by an `f64` scalar (no silent substitution).
#[inline]
pub(crate) fn repr_div_f64(a: Decimal, rhs: f64) -> Result<Decimal, Error> {
    // `f64_to_decimal` rejects NaN / ±Inf first, so the exact-zero check below
    // only ever sees finite input. Division by any non-zero divisor (however
    // small) is representable as a Decimal; division by exact 0.0 is not.
    let rhs_decimal = f64_to_decimal(rhs)?;
    #[allow(clippy::float_cmp)]
    let is_zero = rhs == 0.0;
    if is_zero {
        return Err(Error::Validation("division by zero".into()));
    }
    // Checked Decimal divide: dividing by a tiny scalar can overflow Decimal's
    // range. Surface as an error so `checked_div_f64` never panics.
    a.checked_div(rhs_decimal)
        .ok_or_else(|| InputError::ConversionOverflow.into())
}

/// Fallible rounding of an `f64` into a Decimal (no silent substitution).
#[inline]
pub(crate) fn try_round_f64(x: f64, dp: i32, mode: RoundingMode) -> Result<Decimal, Error> {
    Ok(round_decimal(f64_to_decimal(x)?, dp, mode))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
fn round_f64(x: f64, dp: i32, mode: RoundingMode) -> Decimal {
    try_round_f64(x, dp, mode).expect("Money rounding requires finite, representable scalar")
}

/// Round a Decimal to `dp` decimal places using the supplied [`RoundingMode`].
#[inline]
pub(crate) fn round_decimal(x: Decimal, dp: i32, mode: RoundingMode) -> Decimal {
    use rust_decimal::RoundingStrategy;

    if dp < 0 {
        return x;
    }

    let strategy = match mode {
        RoundingMode::Bankers => RoundingStrategy::MidpointNearestEven,
        RoundingMode::AwayFromZero => RoundingStrategy::MidpointAwayFromZero,
        RoundingMode::TowardZero => RoundingStrategy::ToZero,
        RoundingMode::Floor => RoundingStrategy::ToNegativeInfinity,
        RoundingMode::Ceil => RoundingStrategy::ToPositiveInfinity,
    };

    x.round_dp_with_strategy(dp as u32, strategy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn decimal_conversion_converts_normal_values() {
        // Normal monetary amounts should convert without issues
        let cases = [
            ("0.0", 0.0),
            ("100.0", 100.0),
            ("-50.25", -50.25),
            ("1000000.00", 1_000_000.0),
            ("0.0001", 0.0001),
        ];
        for (decimal_str, expected) in cases {
            let decimal = Decimal::from_str(decimal_str).expect("Valid decimal string");
            let result = decimal.as_f64();
            assert!(
                (result - expected).abs() < 1e-10,
                "Expected {} but got {}",
                expected,
                result
            );
        }
    }

    #[test]
    fn decimal_conversion_handles_large_values_within_f64_range() {
        // Decimal max is ~7.9e28, which is within f64 range
        // Test with a large value that should still convert
        let large = Decimal::from_str("1000000000000000.0").expect("Valid decimal"); // 1 quadrillion
        let result = large.as_f64();
        assert!(result > 0.0, "Large value should not silently become 0");
        assert!((result - 1e15).abs() < 1e5);
    }

    #[test]
    fn decimal_conversion_preserves_sign() {
        let negative = Decimal::from_str("-999999999.99").expect("Valid decimal");
        let result = negative.as_f64();
        assert!(result < 0.0, "Negative value must remain negative");
        assert!((result - (-999_999_999.99)).abs() < 1e-2);
    }

    // repr_add and repr_sub tests

    #[test]
    fn repr_add_basic() {
        let a = Decimal::from_str("100.50").expect("valid decimal");
        let b = Decimal::from_str("50.25").expect("valid decimal");
        let result = repr_add(a, b).expect("addition should succeed");
        assert_eq!(result, Decimal::from_str("150.75").expect("valid decimal"));
    }

    #[test]
    fn repr_add_negative() {
        let a = Decimal::from_str("100.00").expect("valid decimal");
        let b = Decimal::from_str("-25.00").expect("valid decimal");
        let result = repr_add(a, b).expect("addition should succeed");
        assert_eq!(result, Decimal::from_str("75.00").expect("valid decimal"));
    }

    #[test]
    fn repr_sub_basic() {
        let a = Decimal::from_str("100.00").expect("valid decimal");
        let b = Decimal::from_str("30.00").expect("valid decimal");
        let result = repr_sub(a, b).expect("subtraction should succeed");
        assert_eq!(result, Decimal::from_str("70.00").expect("valid decimal"));
    }

    #[test]
    fn repr_sub_negative_result() {
        let a = Decimal::from_str("25.00").expect("valid decimal");
        let b = Decimal::from_str("100.00").expect("valid decimal");
        let result = repr_sub(a, b).expect("subtraction should succeed");
        assert_eq!(result, Decimal::from_str("-75.00").expect("valid decimal"));
    }

    // round_f64 tests

    #[test]
    fn round_f64_bankers_positive() {
        let result = round_f64(1.5, 0, RoundingMode::Bankers);
        assert_eq!(result, Decimal::from_str("2").expect("valid decimal"));
    }

    #[test]
    fn round_f64_bankers_tie_to_even() {
        // 2.5 rounds to 2 (even), 3.5 rounds to 4 (even)
        let result1 = round_f64(2.5, 0, RoundingMode::Bankers);
        assert_eq!(result1, Decimal::from_str("2").expect("valid decimal"));

        let result2 = round_f64(3.5, 0, RoundingMode::Bankers);
        assert_eq!(result2, Decimal::from_str("4").expect("valid decimal"));
    }

    #[test]
    fn round_f64_away_from_zero() {
        let result1 = round_f64(1.5, 0, RoundingMode::AwayFromZero);
        assert_eq!(result1, Decimal::from_str("2").expect("valid decimal"));

        let result2 = round_f64(-1.5, 0, RoundingMode::AwayFromZero);
        assert_eq!(result2, Decimal::from_str("-2").expect("valid decimal"));
    }

    #[test]
    fn round_f64_toward_zero() {
        let result1 = round_f64(1.9, 0, RoundingMode::TowardZero);
        assert_eq!(result1, Decimal::from_str("1").expect("valid decimal"));

        let result2 = round_f64(-1.9, 0, RoundingMode::TowardZero);
        assert_eq!(result2, Decimal::from_str("-1").expect("valid decimal"));
    }

    #[test]
    fn round_f64_floor() {
        let result1 = round_f64(1.9, 0, RoundingMode::Floor);
        assert_eq!(result1, Decimal::from_str("1").expect("valid decimal"));

        let result2 = round_f64(-1.1, 0, RoundingMode::Floor);
        assert_eq!(result2, Decimal::from_str("-2").expect("valid decimal"));
    }

    #[test]
    fn round_f64_ceil() {
        let result1 = round_f64(1.1, 0, RoundingMode::Ceil);
        assert_eq!(result1, Decimal::from_str("2").expect("valid decimal"));

        let result2 = round_f64(-1.9, 0, RoundingMode::Ceil);
        assert_eq!(result2, Decimal::from_str("-1").expect("valid decimal"));
    }

    #[test]
    fn round_f64_with_decimal_places() {
        let result = round_f64(1.234567, 2, RoundingMode::Bankers);
        let expected = Decimal::from_str("1.23").expect("valid decimal");
        assert!((result - expected).abs() < Decimal::from_str("0.01").expect("valid decimal"));
    }

    #[test]
    #[should_panic(expected = "Money rounding requires finite")]
    fn round_f64_panics_on_nan() {
        round_f64(f64::NAN, 2, RoundingMode::Bankers);
    }

    // round_decimal tests

    #[test]
    fn round_decimal_bankers() {
        let val = Decimal::from_str("2.5").expect("valid decimal");
        let result = round_decimal(val, 0, RoundingMode::Bankers);
        assert_eq!(result, Decimal::from_str("2").expect("valid decimal"));
    }

    #[test]
    fn round_decimal_away_from_zero() {
        let val = Decimal::from_str("1.5").expect("valid decimal");
        let result = round_decimal(val, 0, RoundingMode::AwayFromZero);
        assert_eq!(result, Decimal::from_str("2").expect("valid decimal"));
    }

    #[test]
    fn round_decimal_toward_zero() {
        let val = Decimal::from_str("1.9").expect("valid decimal");
        let result = round_decimal(val, 0, RoundingMode::TowardZero);
        assert_eq!(result, Decimal::from_str("1").expect("valid decimal"));
    }

    #[test]
    fn round_decimal_floor() {
        let val = Decimal::from_str("1.9").expect("valid decimal");
        let result = round_decimal(val, 0, RoundingMode::Floor);
        assert_eq!(result, Decimal::from_str("1").expect("valid decimal"));
    }

    #[test]
    fn round_decimal_ceil() {
        let val = Decimal::from_str("1.1").expect("valid decimal");
        let result = round_decimal(val, 0, RoundingMode::Ceil);
        assert_eq!(result, Decimal::from_str("2").expect("valid decimal"));
    }

    #[test]
    fn round_decimal_negative_dp_returns_unchanged() {
        let val = Decimal::from_str("123.456").expect("valid decimal");
        let result = round_decimal(val, -1, RoundingMode::Bankers);
        assert_eq!(result, val);
    }

    #[test]
    fn round_decimal_with_decimal_places() {
        let val = Decimal::from_str("1.23456").expect("valid decimal");
        let result = round_decimal(val, 2, RoundingMode::Bankers);
        let expected = Decimal::from_str("1.23").expect("valid decimal");
        assert!((result - expected).abs() < Decimal::from_str("0.01").expect("valid decimal"));
    }
}
