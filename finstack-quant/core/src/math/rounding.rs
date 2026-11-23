//! Numerical tolerances, rounding, and clamping helpers.

/// Tolerance for checking if a value is effectively zero.
///
/// Used across the workspace for near-zero guards, safe division, and approximate
/// equality comparisons. Value: 1e-10 (well above f64 machine epsilon ~2.2e-16
/// but small enough to catch actual zeros vs meaningful small values).
pub const ZERO_TOLERANCE: f64 = 1e-10;

/// Quantize a finite binary64 number to decimal places, rounding ties away from zero.
/// The decimal target is converted once to the nearest binary64 value.
/// Returns an error if that conversion overflows.
///
/// # Arguments
///
/// * `x` - Finite input as represented in binary64. A rounded zero retains its sign.
/// * `digits` - Decimal places to retain. Negative values round to multiples of ten.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::round_half_away;
/// assert_eq!(round_half_away(-2.5, 0)?, -3.0);
/// assert_eq!(round_half_away(2.675, 2)?, 2.67);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn round_half_away(x: f64, digits: i32) -> crate::Result<f64> {
    use num_bigint::BigInt;
    use num_rational::BigRational;
    use num_traits::ToPrimitive;

    if !x.is_finite() {
        return Err(crate::Error::Validation(
            "round requires a finite value".into(),
        ));
    }
    if digits == 0 {
        return Ok(x.round());
    }
    if digits >= 324 || (digits > 0 && x.fract() == 0.0) {
        return Ok(x);
    }
    if digits <= -309 {
        return Ok(0.0_f64.copysign(x));
    }
    let magnitude = BigRational::from_float(x.abs())
        .ok_or_else(|| crate::Error::Validation("round requires a finite value".into()))?;
    let power = BigInt::from(10u8).pow(digits.unsigned_abs());
    let rounded = if digits > 0 {
        (magnitude * &power).round() / power
    } else {
        (magnitude / &power).round() * power
    };
    rounded
        .to_f64()
        .filter(|v| v.is_finite())
        .map(|v| v.copysign(x))
        .ok_or_else(|| crate::Error::Validation("rounded result exceeds binary64 range".into()))
}

/// Clamp a finite value to an inclusive, ordered interval.
///
/// # Arguments
///
/// * `x` - Finite value to restrict. An in-range value retains its zero sign.
/// * `lo` - Finite inclusive lower bound.
/// * `hi` - Finite inclusive upper bound, at least `lo`.
pub fn clamp(x: f64, lo: f64, hi: f64) -> crate::Result<f64> {
    if !x.is_finite() || !lo.is_finite() || !hi.is_finite() || lo > hi {
        return Err(crate::Error::Validation(
            "clamp requires finite values and ordered bounds".into(),
        ));
    }
    if (lo..=hi).contains(&x) {
        return Ok(x);
    }
    Ok(if x < lo { lo } else { hi })
}
