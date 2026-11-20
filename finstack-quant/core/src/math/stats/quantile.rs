//! Exact binary64 rational R-7 quantiles, quantile ratios and tail means.

use super::{MAX_EXACT_COUNT, normalize_zero, validate_finite_slice};
use crate::math::interp::exact_f64_eq;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

/// Empirical quantile via partial sort.
///
/// # Interpolation method
///
/// Returns the `p`-th quantile (`0 <= p <= 1`) using **linear interpolation
/// between adjacent order statistics** — specifically R's `type = 7` /
/// NumPy's default `linear` mode, which computes
///
/// ```text
/// h = (n - 1) · p
/// lo = floor(h), hi = lo + 1
/// result = (1 - (h - lo)) · data[lo] + (h - lo) · data[hi]
/// ```
///
/// Ranks and binary64 weights are evaluated in exact rational arithmetic before
/// the single final conversion back to `f64`, preventing near-integer residual
/// loss and endpoint subtraction overflow on large opposite-signed values.
///
/// # Mutation and cost
///
/// The input slice is **mutated** (partially sorted via
/// [`slice::select_nth_unstable_by`]) to avoid allocation. Cost is
/// expected O(n). Returns `Ok(None)` when `data` is empty after validating `p`.
///
/// # Arguments
///
/// * `data` - Finite observations to partially sort in place. The function
///   mutates their order to avoid allocating a sorted copy.
/// * `p` - Quantile probability in the inclusive `0.0..=1.0` range, using the
///   R-7/NumPy linear interpolation convention.
///
/// # Errors
///
/// * Returns [`crate::Error::Validation`] if `p` is non-finite or outside
///   `[0.0, 1.0]`, or if `data.len()` exceeds the exact count limit (`2^53`).
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `data` is `NaN` or infinite (checked before
///   partially sorting `data`).
/// * Returns [`crate::Error::Internal`] when a mathematically nonzero
///   interpolated quantile underflows or overflows binary64 range.
pub fn quantile(data: &mut [f64], p: f64) -> crate::Result<Option<f64>> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(crate::Error::Validation(format!(
            "Quantile probability must be finite and in [0, 1], got {p}"
        )));
    }
    validate_finite_slice(data)?;
    if data.is_empty() {
        return Ok(None);
    }
    let (lo, frac) = r7_rank(data.len(), p)?;
    data.select_nth_unstable_by(lo, f64::total_cmp);
    let v_lo = data[lo];
    if frac.is_zero() {
        return Ok(Some(normalize_zero(v_lo)));
    }
    data[lo + 1..].select_nth_unstable_by(0, f64::total_cmp);
    let v_hi = data[lo + 1];
    if exact_f64_eq(v_lo, v_hi) {
        return Ok(Some(normalize_zero(v_lo)));
    }
    let val = rational_to_f64(&interpolate_r7(v_lo, v_hi, &frac)?)?;
    Ok(Some(normalize_zero(val)))
}

/// Empirical R-7 quantile of data that is already sorted ascending.
///
/// Same estimator as [`quantile`] (`h = (n - 1) · p`, linear interpolation
/// between the bracketing order statistics) for callers that sort once and
/// read several quantiles. Validates finiteness and non-decreasing ordering
/// across the entire slice on every call without mutating `sorted`.
///
/// # Arguments
///
/// * `sorted` - Finite observations sorted in non-decreasing order.
/// * `p` - Quantile probability in the inclusive `0.0..=1.0` range.
///
/// # Errors
///
/// * Returns [`crate::Error::Validation`] if `p` is non-finite or outside
///   `[0.0, 1.0]`, if `sorted` is not non-decreasing, or if `sorted.len()`
///   exceeds `2^53`.
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `sorted` is `NaN` or infinite.
/// * Returns [`crate::Error::Internal`] when a mathematically nonzero
///   interpolated quantile underflows or overflows binary64 range.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::quantile_sorted;
/// # fn main() -> finstack_quant_core::Result<()> {
///
/// let sorted = [1.0, 2.0, 3.0, 4.0];
/// assert_eq!(quantile_sorted(&sorted, 0.5)?, Some(2.5));
/// assert_eq!(quantile_sorted(&sorted, 1.0)?, Some(4.0));
/// assert_eq!(quantile_sorted(&[], 0.5)?, None);
/// # Ok(())
/// # }
/// ```
pub fn quantile_sorted(sorted: &[f64], p: f64) -> crate::Result<Option<f64>> {
    if !p.is_finite() || !(0.0..=1.0).contains(&p) {
        return Err(crate::Error::Validation(format!(
            "Quantile probability must be finite and in [0, 1], got {p}"
        )));
    }
    validate_finite_slice(sorted)?;
    if sorted.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err(crate::Error::Validation(
            "quantile_sorted requires observations in non-decreasing order".into(),
        ));
    }
    if sorted.is_empty() {
        return Ok(None);
    }
    let val = rational_to_f64(&quantile_rational(sorted, p)?)?;
    Ok(Some(normalize_zero(val)))
}

/// Sample median (R-7 0.5-quantile) over finite observations.
///
/// Copies finite observations into a scratch buffer and delegates to
/// [`quantile`] with `p = 0.5`. Returns `Ok(None)` when `xs` is empty.
///
/// # Arguments
///
/// * `xs` - Finite observations whose sample median is required.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `xs` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `xs.len()` exceeds `2^53`.
/// * Returns [`crate::Error::Internal`] when a nonzero median underflows
///   binary64 range.
pub fn median(xs: &[f64]) -> crate::Result<Option<f64>> {
    validate_finite_slice(xs)?;
    if xs.is_empty() {
        return Ok(None);
    }
    let mut buf = xs.to_vec();
    quantile(&mut buf, 0.5)
}

/// Magnitude ratio of two R-7 quantiles, rounded only after division.
///
/// Both quantiles retain their exact binary-rational rank weights and
/// interpolation values until their magnitude ratio is formed. Individually
/// unrepresentable component quantiles therefore do not prevent a
/// representable requested ratio. This also preserves consequential residuals
/// at the final ratio's rounding boundary. A genuinely zero denominator gives
/// positive infinity for a nonzero numerator, or NaN for two zero quantiles.
///
/// # Arguments
///
/// * `sorted` - Nonempty finite observations in ascending order. Their
///   order is validated, and the slice is not modified.
/// * `numerator_p` - Numerator quantile probability in the inclusive
///   `0.0..=1.0` range, interpreted as its supplied binary64 value.
/// * `denominator_p` - Denominator quantile probability in the inclusive
///   `0.0..=1.0` range, interpreted as its supplied binary64 value.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] for empty, nonfinite or unordered
/// observations, or an invalid probability. Returns
/// [`crate::Error::Internal`] for an unrepresentable requested nonzero finite
/// ratio; arithmetic underflow never creates an observed zero ratio.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::quantile_ratio;
/// let tiny = f64::from_bits(1);
/// assert_eq!(quantile_ratio(&[0.0, tiny], 0.75, 0.25)?, 3.0);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn quantile_ratio(sorted: &[f64], numerator_p: f64, denominator_p: f64) -> crate::Result<f64> {
    validate_sorted_sample(sorted)?;
    let numerator = quantile_rational(sorted, numerator_p)?.abs();
    let denominator = quantile_rational(sorted, denominator_p)?.abs();
    if denominator.is_zero() {
        return Ok(if numerator.is_zero() {
            f64::NAN
        } else {
            f64::INFINITY
        });
    }
    rational_to_f64(&(numerator / denominator))
}

/// Return the signed mean of the worst fractional tail of an empirical sample.
///
/// # Arguments
/// * `data` - Nonempty finite observations in any order, such as signed
///   simple returns. The result has the observations' units.
/// * `confidence` - Probability strictly between zero and one. The lower
///   tail contains exactly `1 - confidence` of the empirical mass.
///
/// # Errors
/// Returns a validation error for invalid confidence, empty data or nonfinite
/// observations, or a computation error for an unrepresentable result.
pub fn expected_shortfall(data: &[f64], confidence: f64) -> crate::Result<f64> {
    if !(confidence > 0.0 && confidence < 1.0) {
        return Err(crate::Error::Validation(
            "Expected shortfall confidence must be in (0, 1)".into(),
        ));
    }
    let mut sorted = data.to_vec();
    sorted.sort_unstable_by(f64::total_cmp);
    empirical_tail_mean(&sorted, 1.0 - confidence)
}

/// Mean of exactly the lowest `p` mass of an equal-weight empirical sample.
///
/// With `mass = p * sorted.len()`, the calculation includes every observation
/// below `floor(mass)` and exactly the required fraction of the next one.
/// Ties do not expand the tail. Integer counts and supplied binary64 values
/// are evaluated exactly before the final rounding, preserving consequential
/// near-integer mass residuals, cancellation and subnormal observations.
/// This is an empirical tail mean, not an integral of the R-7 quantile curve.
///
/// # Arguments
///
/// * `sorted` - Nonempty finite observations in ascending order. The slice
///   is validated and is not modified; values can be positive or negative.
/// * `p` - Strictly positive selected probability mass, at most `1.0`.
///   A mass smaller than one observation returns the smallest observation.
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] for an empty, nonfinite or unordered
/// sample or a probability outside `(0, 1]`. Returns
/// [`crate::Error::Internal`] for an unrepresentable nonzero requested mean.
/// Genuine zero means remain valid; no floor or clipping is applied.
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::math::stats::empirical_tail_mean;
/// assert!((empirical_tail_mean(&[-0.2, -0.1, 0.0], 0.5)? + 1.0 / 6.0).abs() < 1e-15);
/// let tiny = f64::from_bits(1);
/// assert_eq!(empirical_tail_mean(&[tiny; 5], 0.4)?, tiny);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn empirical_tail_mean(sorted: &[f64], p: f64) -> crate::Result<f64> {
    validate_sorted_sample(sorted)?;
    if !(p > 0.0 && p <= 1.0) {
        return Err(crate::Error::Validation(
            "Empirical tail probability must be in (0, 1]".into(),
        ));
    }
    let (whole, fraction) = split_mass(sorted.len(), p)?;
    if whole == 0 {
        return Ok(sorted[0]);
    }
    let mut total = BigRational::zero();
    for &value in &sorted[..whole] {
        total += binary_rational(value)?;
    }
    if !fraction.is_zero() {
        total += binary_rational(sorted[whole])? * &fraction;
    }
    let mass = BigRational::from_integer(BigInt::from(whole)) + fraction;
    rational_to_f64(&(total / mass))
}

fn validate_sorted_sample(sorted: &[f64]) -> crate::Result<()> {
    if sorted.is_empty()
        || sorted.len() > MAX_EXACT_COUNT
        || sorted.iter().any(|value| !value.is_finite())
        || sorted.windows(2).any(|pair| pair[0] > pair[1])
    {
        return Err(crate::Error::Validation(
            "A nonempty finite ascending sample is required".into(),
        ));
    }
    Ok(())
}

fn binary_rational(value: f64) -> crate::Result<BigRational> {
    BigRational::from_float(value).ok_or_else(|| {
        crate::Error::Validation("Exact statistical weights require finite values".into())
    })
}

/// Keep the integer rank and its fractional remainder separate and exact.
fn r7_rank(n: usize, p: f64) -> crate::Result<(usize, BigRational)> {
    if n == 0 || !(0.0..=1.0).contains(&p) {
        return Err(crate::Error::Validation(
            "R-7 requires observations and a probability in [0, 1]".into(),
        ));
    }
    split_mass(n - 1, p)
}

fn split_mass(count: usize, p: f64) -> crate::Result<(usize, BigRational)> {
    let mass = binary_rational(p)? * BigInt::from(count);
    let whole = mass.to_integer().to_usize().ok_or_else(|| {
        crate::Error::Internal("Statistical rank does not fit an observation index".into())
    })?;
    let fraction = mass - BigRational::from_integer(BigInt::from(whole));
    Ok((whole, fraction))
}

fn interpolate_r7(lower: f64, upper: f64, fraction: &BigRational) -> crate::Result<BigRational> {
    if lower.to_bits() == upper.to_bits() || fraction.is_zero() {
        return binary_rational(lower);
    }
    Ok(binary_rational(lower)? * (BigRational::one() - fraction)
        + binary_rational(upper)? * fraction)
}

fn quantile_rational(sorted: &[f64], p: f64) -> crate::Result<BigRational> {
    let (lo, fraction) = r7_rank(sorted.len(), p)?;
    let lower = sorted[lo];
    if fraction.is_zero() {
        return binary_rational(lower);
    }
    interpolate_r7(lower, sorted[lo + 1], &fraction)
}

fn rational_to_f64(value: &BigRational) -> crate::Result<f64> {
    let rounded = value.to_f64().ok_or_else(|| {
        crate::Error::Internal("Statistical result could not be reconstructed".into())
    })?;
    if !rounded.is_finite()
        || (rounded.classify() == std::num::FpCategory::Zero && !value.is_zero())
    {
        return Err(crate::Error::Internal(
            "Statistical result is outside the nonzero finite binary64 range".into(),
        ));
    }
    Ok(rounded)
}
