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
use num_bigint::BigInt;
use rust_decimal::Decimal;

/// Largest magnitude of a Decimal coefficient (96 bits).
const MAX_COEFFICIENT: u128 = (1_u128 << 96) - 1;

/// Sum `a + b`, exact to the minor unit (record 012 #6 with owner decision
/// D7, 2026-11-09).
///
/// The exact sum is returned when it is representable. Otherwise it is
/// rounded once, half-even, at the greatest scale whose coefficient fits,
/// provided that scale keeps every digit at or above `minor_unit_scale`
/// decimal places (the currency's minor unit). If keeping those digits does
/// not fit, the sum is `InputError::ConversionOverflow`: `1e28 + 0.01` USD is
/// an error (the cent would be lost), while sub-minor-unit model residue
/// beyond Decimal's 28-29 digits rounds.
///
/// # Arguments
///
/// * `a` - Left operand.
/// * `b` - Right operand.
/// * `minor_unit_scale` - Decimal places of the currency's minor unit; no digit at or above it may be dropped.
pub(crate) fn repr_add(a: Decimal, b: Decimal, minor_unit_scale: u32) -> Result<Decimal, Error> {
    linear(a, b, a.checked_add(b), |x, y| x + y, minor_unit_scale)
}

/// Difference `a - b`, exact to the minor unit; see [`repr_add`].
///
/// # Arguments
///
/// * `a` - Minuend.
/// * `b` - Subtrahend.
/// * `minor_unit_scale` - Decimal places of the currency's minor unit; no digit at or above it may be dropped.
pub(crate) fn repr_sub(a: Decimal, b: Decimal, minor_unit_scale: u32) -> Result<Decimal, Error> {
    linear(a, b, a.checked_sub(b), |x, y| x - y, minor_unit_scale)
}

/// Accept the dependency's result only when it kept the operands' common
/// scale (no rescale, so no rounding happened); otherwise decide exactly.
fn linear(
    a: Decimal,
    b: Decimal,
    dependency: Option<Decimal>,
    combine: fn(BigInt, BigInt) -> BigInt,
    minor_unit_scale: u32,
) -> Result<Decimal, Error> {
    let scale = a.scale().max(b.scale());
    if let Some(result) = dependency.filter(|result| result.scale() == scale) {
        return Ok(result);
    }
    let align = |d: Decimal| BigInt::from(d.mantissa()) * pow10(scale - d.scale());
    let exact = combine(align(a), align(b));
    let negative = exact.sign() == num_bigint::Sign::Minus;
    let magnitude = if negative { -exact } else { exact };
    // Greatest scale first; the exact value at `scale`, then each rounding
    // that still keeps the minor unit.
    for target in (minor_unit_scale.min(scale)..=scale).rev() {
        let divisor = pow10(scale - target);
        let quotient = &magnitude / &divisor;
        let twice_remainder = (&magnitude % &divisor) * 2_u8;
        let odd = (&quotient % 2_u8) == BigInt::from(1_u8);
        let rounded = if twice_remainder > divisor || (twice_remainder == divisor && odd) {
            quotient + 1_u8
        } else {
            quotient
        };
        if let Some(coefficient) = u128::try_from(&rounded)
            .ok()
            .filter(|c| *c <= MAX_COEFFICIENT)
        {
            let signed = coefficient as i128;
            return Ok(Decimal::from_i128_with_scale(
                if negative { -signed } else { signed },
                target,
            ));
        }
    }
    Err(InputError::ConversionOverflow.into())
}

fn pow10(exponent: u32) -> BigInt {
    num_traits::pow(BigInt::from(10_u8), exponent as usize)
}

/// Bounded product `a * factor` (record 012 #7).
///
/// The result is the exact product when representable, otherwise the exact
/// product rounded once, half-even, at the greatest scale whose coefficient
/// fits (the dependency's `checked_mul`, qualified against an exact
/// reference). Overflow is `InputError::ConversionOverflow`; a nonzero product
/// that rounds to zero is `InputError::DecimalUnderflow`.
pub(crate) fn repr_mul(a: Decimal, factor: Decimal) -> Result<Decimal, Error> {
    let product = a
        .checked_mul(factor)
        .ok_or(Error::Input(InputError::ConversionOverflow))?;
    if product.is_zero() && !a.is_zero() && !factor.is_zero() {
        return Err(InputError::DecimalUnderflow.into());
    }
    Ok(product)
}

/// Bounded quotient `a / divisor` (record 012 #7); see [`repr_mul`].
///
/// A zero divisor is `Error::Validation("division by zero")`.
pub(crate) fn repr_div(a: Decimal, divisor: Decimal) -> Result<Decimal, Error> {
    if divisor.is_zero() {
        return Err(Error::Validation("division by zero".into()));
    }
    let quotient = a
        .checked_div(divisor)
        .ok_or(Error::Input(InputError::ConversionOverflow))?;
    if quotient.is_zero() && !a.is_zero() {
        return Err(InputError::DecimalUnderflow.into());
    }
    Ok(quotient)
}

/// `f64` adapter for [`repr_mul`]: the scalar is admitted by its shortest
/// round-trip text (record 012 #4) and then multiplied exactly as a Decimal.
pub(crate) fn repr_mul_f64(a: Decimal, rhs: f64) -> Result<Decimal, Error> {
    repr_mul(a, f64_to_decimal(rhs)?)
}

/// `f64` adapter for [`repr_div`]; `0.0` and `-0.0` are division by zero.
pub(crate) fn repr_div_f64(a: Decimal, rhs: f64) -> Result<Decimal, Error> {
    repr_div(a, f64_to_decimal(rhs)?)
}

/// Validate a financial rounding scale: decimal places in `0..=28`
/// (record 012 #8). There is no invalid-precision identity.
///
/// # Arguments
///
/// * `scale` - Requested decimal places in major currency units.
pub(crate) fn validate_rounding_scale(scale: u32) -> Result<(), Error> {
    if scale > Decimal::MAX_SCALE {
        return Err(Error::Validation(format!(
            "rounding scale must be in 0..=28, got {scale}"
        )));
    }
    Ok(())
}

/// Round a Decimal to `dp` decimal places using the supplied [`RoundingMode`].
///
/// Callers validate `dp` (see [`validate_rounding_scale`]); a value already
/// at or below `dp` places is returned unchanged.
#[inline]
pub(crate) fn round_decimal(x: Decimal, dp: u32, mode: RoundingMode) -> Decimal {
    use rust_decimal::RoundingStrategy;

    let strategy = match mode {
        RoundingMode::Bankers => RoundingStrategy::MidpointNearestEven,
        RoundingMode::MidpointAwayFromZero => RoundingStrategy::MidpointAwayFromZero,
        RoundingMode::TowardZero => RoundingStrategy::ToZero,
        RoundingMode::Floor => RoundingStrategy::ToNegativeInfinity,
        RoundingMode::Ceil => RoundingStrategy::ToPositiveInfinity,
    };

    x.round_dp_with_strategy(dp, strategy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Zero;
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
        let result = repr_add(a, b, 2).expect("addition should succeed");
        assert_eq!(result, Decimal::from_str("150.75").expect("valid decimal"));
    }

    #[test]
    fn repr_add_negative() {
        let a = Decimal::from_str("100.00").expect("valid decimal");
        let b = Decimal::from_str("-25.00").expect("valid decimal");
        let result = repr_add(a, b, 2).expect("addition should succeed");
        assert_eq!(result, Decimal::from_str("75.00").expect("valid decimal"));
    }

    #[test]
    fn repr_sub_basic() {
        let a = Decimal::from_str("100.00").expect("valid decimal");
        let b = Decimal::from_str("30.00").expect("valid decimal");
        let result = repr_sub(a, b, 2).expect("subtraction should succeed");
        assert_eq!(result, Decimal::from_str("70.00").expect("valid decimal"));
    }

    #[test]
    fn repr_sub_negative_result() {
        let a = Decimal::from_str("25.00").expect("valid decimal");
        let b = Decimal::from_str("100.00").expect("valid decimal");
        let result = repr_sub(a, b, 2).expect("subtraction should succeed");
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
        let result1 = round_f64(1.5, 0, RoundingMode::MidpointAwayFromZero);
        assert_eq!(result1, Decimal::from_str("2").expect("valid decimal"));

        let result2 = round_f64(-1.5, 0, RoundingMode::MidpointAwayFromZero);
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

    /// Test helper: admit `x` exactly (record 012 #4), then round.
    fn round_f64(x: f64, dp: u32, mode: RoundingMode) -> Decimal {
        round_decimal(f64_to_decimal(x).expect("finite fixture"), dp, mode)
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
        let result = round_decimal(val, 0, RoundingMode::MidpointAwayFromZero);
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
    fn rounding_scale_is_bounded_by_decimal() {
        assert!(validate_rounding_scale(0).is_ok());
        assert!(validate_rounding_scale(28).is_ok());
        assert!(validate_rounding_scale(29).is_err());
    }

    #[test]
    fn record_012_mode_table_at_scale_zero() {
        let cases = [
            (RoundingMode::Bankers, "2", "-2"),
            (RoundingMode::MidpointAwayFromZero, "3", "-3"),
            (RoundingMode::TowardZero, "2", "-2"),
            (RoundingMode::Floor, "2", "-3"),
            (RoundingMode::Ceil, "3", "-2"),
        ];
        for (mode, up, down) in cases {
            assert_eq!(round_decimal(Decimal::new(25, 1), 0, mode).to_string(), up);
            assert_eq!(
                round_decimal(Decimal::new(-25, 1), 0, mode).to_string(),
                down
            );
        }
        // Midpoint-away is not directed-away rounding of every fraction.
        let m = RoundingMode::MidpointAwayFromZero;
        assert_eq!(round_decimal(Decimal::new(11, 1), 0, m).to_string(), "1");
        assert_eq!(round_decimal(Decimal::new(-11, 1), 0, m).to_string(), "-1");
    }

    #[test]
    fn round_decimal_with_decimal_places() {
        let val = Decimal::from_str("1.23456").expect("valid decimal");
        let result = round_decimal(val, 2, RoundingMode::Bankers);
        let expected = Decimal::from_str("1.23").expect("valid decimal");
        assert!((result - expected).abs() < Decimal::from_str("0.01").expect("valid decimal"));
    }

    // Record 012 #6/#7 qualification against an exact BigInt reference.

    fn big(d: Decimal) -> (BigInt, u32) {
        (BigInt::from(d.mantissa()), d.scale())
    }

    fn max_coefficient() -> BigInt {
        BigInt::from(MAX_COEFFICIENT)
    }

    /// Exact value `n / d` (d > 0) rounded half-even at `scale` places.
    fn round_half_even(n: &BigInt, d: &BigInt, scale: u32) -> BigInt {
        let num = if n.sign() == num_bigint::Sign::Minus {
            -n
        } else {
            n.clone()
        } * pow10(scale);
        let q = &num / d;
        let r = &num % d;
        let twice = r * 2_u8;
        let odd = (&q % 2_u8) == BigInt::from(1_u8);
        if twice > *d || (twice == *d && odd) {
            q + 1_u8
        } else {
            q
        }
    }

    /// Item 7 reference: `Ok(value)` as `(coefficient, scale)`, or the error kind.
    fn bounded_reference(n: BigInt, d: BigInt) -> Result<(BigInt, u32), &'static str> {
        if n.is_zero() {
            return Ok((BigInt::zero(), 0));
        }
        let magnitude = if n.sign() == num_bigint::Sign::Minus {
            -&n
        } else {
            n.clone()
        };
        if magnitude > max_coefficient() * &d {
            return Err("overflow");
        }
        for scale in (0..=28_u32).rev() {
            let c = round_half_even(&n, &d, scale);
            if c <= max_coefficient() {
                if c.is_zero() {
                    return Err("underflow");
                }
                let c = if n.sign() == num_bigint::Sign::Minus {
                    -c
                } else {
                    c
                };
                return Ok((c, scale));
            }
        }
        Err("overflow")
    }

    /// Item 6 / D7 reference: the exact sum rounded half-even at the greatest
    /// scale that fits, never below `minor` places.
    fn sum_reference(c: BigInt, scale: u32, minor: u32) -> Result<(BigInt, u32), &'static str> {
        let d = pow10(scale);
        for target in (minor.min(scale)..=scale).rev() {
            let rounded = round_half_even(&c, &d, target);
            if rounded <= max_coefficient() {
                let rounded = if c.sign() == num_bigint::Sign::Minus {
                    -rounded
                } else {
                    rounded
                };
                return Ok((rounded, target));
            }
        }
        Err("inexact")
    }

    fn same_value(d: Decimal, (c, scale): &(BigInt, u32)) -> bool {
        let (m, ds) = big(d);
        m * pow10(*scale) == c * pow10(ds)
    }

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0 >> 11
        }
        fn decimal(&mut self) -> Decimal {
            let digits = (self.next() % 29).max(1);
            let mut m: i128 = 0;
            for _ in 0..digits {
                m = m * 10 + (self.next() % 10) as i128;
            }
            m %= MAX_COEFFICIENT as i128;
            if self.next().is_multiple_of(2) {
                m = -m;
            }
            Decimal::from_i128_with_scale(m, (self.next() % 29) as u32)
        }
    }

    fn check(
        op: &str,
        a: Decimal,
        b: Decimal,
        got: Result<Decimal, Error>,
        want: Result<(BigInt, u32), &'static str>,
    ) {
        match (&got, &want) {
            (Ok(value), Ok(expected)) => assert!(
                same_value(*value, expected),
                "{op}({a}, {b}) = {value}, want {}e-{}",
                expected.0,
                expected.1
            ),
            (Err(Error::Input(InputError::DecimalUnderflow)), Err("underflow"))
            | (Err(Error::Input(InputError::ConversionOverflow)), Err("overflow" | "inexact")) => {}
            _ => panic!("{op}({a}, {b}): got {got:?}, want {want:?}"),
        }
    }

    fn check_all(a: Decimal, b: Decimal) {
        let ((ma, sa), (mb, sb)) = (big(a), big(b));
        let scale = sa.max(sb);
        let (na, nb) = (&ma * pow10(scale - sa), &mb * pow10(scale - sb));
        for minor in [0_u32, 2, 28] {
            let sum = sum_reference(&na + &nb, scale, minor);
            check("add", a, b, repr_add(a, b, minor), sum);
            let difference = sum_reference(&na - &nb, scale, minor);
            check("sub", a, b, repr_sub(a, b, minor), difference);
        }
        check(
            "mul",
            a,
            b,
            repr_mul(a, b),
            bounded_reference(&ma * &mb, pow10(sa + sb)),
        );
        if !b.is_zero() {
            let (mut n, mut d) = (&ma * pow10(sb), &mb * pow10(sa));
            if d.sign() == num_bigint::Sign::Minus {
                n = -n;
                d = -d;
            }
            check("div", a, b, repr_div(a, b), bounded_reference(n, d));
        }
    }

    #[test]
    fn decimal_arithmetic_matches_the_exact_reference() {
        let mut rng = Lcg(0x5eed);
        for _ in 0..20_000 {
            check_all(rng.decimal(), rng.decimal());
        }
        // Exact half-even ties at the coefficient limit.
        for _ in 0..5_000 {
            let m = ((rng.next() as i128) * (rng.next() as i128)) % MAX_COEFFICIENT as i128;
            let a = Decimal::from_i128_with_scale(m | 1, (rng.next() % 29) as u32);
            for b in [
                Decimal::new(5, 1),
                Decimal::new(5, 2),
                Decimal::new(25, 2),
                Decimal::from(2),
                Decimal::new(125, 3),
            ] {
                check_all(a, b);
            }
        }
    }

    #[test]
    fn sums_never_drop_a_minor_unit_digit() {
        let big = Decimal::from_i128_with_scale(10_i128.pow(28), 0);
        let cent = Decimal::new(1, 2);
        // The cent would be lost: an error for a 2-decimal currency.
        assert!(matches!(
            repr_add(big, cent, 2),
            Err(Error::Input(InputError::ConversionOverflow))
        ));
        assert!(repr_sub(big, cent, 2).is_err());
        assert!(repr_add(Decimal::MAX, Decimal::ONE, 0).is_err());
        // Sub-minor-unit residue beyond Decimal's digits rounds half-even.
        let model = Decimal::from_str("24889.265874428517229867135231").expect("model amount");
        let other = Decimal::from_str("990070.2447286517").expect("model amount");
        assert_eq!(
            repr_add(model, other, 2)
                .expect("rounded below the cent")
                .to_string(),
            "1014959.5106030802172298671352"
        );
        // Exact cancellation and representable sums stay exact.
        assert_eq!(
            repr_sub(Decimal::new(10, 2), Decimal::new(10, 2), 2).ok(),
            Some(Decimal::ZERO)
        );
        assert_eq!(
            repr_add(Decimal::MAX, Decimal::MIN, 0).ok(),
            Some(Decimal::ZERO)
        );
        assert_eq!(
            repr_add(Decimal::new(1, 28), Decimal::new(1, 28), 2).ok(),
            Some(Decimal::new(2, 28))
        );
    }

    #[test]
    fn products_and_quotients_round_half_even_and_reject_underflow() {
        let five_e_29 = Decimal::new(5, 28);
        // 5e-28 * 0.1 = 5e-29 rounds to 0 at 28 places: underflow, not zero.
        assert!(matches!(
            repr_mul(five_e_29, Decimal::new(1, 1)),
            Err(Error::Input(InputError::DecimalUnderflow))
        ));
        // 1.5e-28 -> 2e-28 and 2.5e-28 -> 2e-28 (ties to even).
        assert_eq!(
            repr_mul(Decimal::new(15, 28), Decimal::new(1, 1)).ok(),
            Some(Decimal::new(2, 28))
        );
        assert_eq!(
            repr_mul(Decimal::new(25, 28), Decimal::new(1, 1)).ok(),
            Some(Decimal::new(2, 28))
        );
        // Exact zero factors and numerators are not underflow.
        assert_eq!(
            repr_mul(Decimal::new(1, 28), Decimal::ZERO).ok(),
            Some(Decimal::ZERO)
        );
        assert_eq!(
            repr_div(Decimal::ZERO, Decimal::new(7, 0)).ok(),
            Some(Decimal::ZERO)
        );
        assert!(matches!(
            repr_div(Decimal::new(1, 28), Decimal::from(3)),
            Err(Error::Input(InputError::DecimalUnderflow))
        ));
        assert!(matches!(
            repr_div(Decimal::ONE, Decimal::ZERO),
            Err(Error::Validation(_))
        ));
        assert!(matches!(
            repr_mul(Decimal::MAX, Decimal::from(2)),
            Err(Error::Input(InputError::ConversionOverflow))
        ));
        assert!(repr_div_f64(Decimal::ONE, -0.0).is_err());
        assert!(repr_mul_f64(Decimal::ONE, f64::NAN).is_err());
    }
}
