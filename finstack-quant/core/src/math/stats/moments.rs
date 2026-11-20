//! Two-pass batch moments: mean, variance, covariance, correlation and beta.

use super::{check_constant_slice, frexp_exp, ldexp, normalize_zero, validate_finite_slice};
use crate::math::summation::{NeumaierAccumulator, neumaier_sum};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

struct ScaledSliceMoments {
    origin: f64,
    exp: i32,
    mean_z: f64,
    sum_r: f64,
    ss_z: f64,
}

/// Compute shifted, power-of-two scaled compensated mean and centered second moment
/// for a validated non-constant finite slice of length >= 2.
fn scaled_slice_moments(xs: &[f64]) -> crate::Result<ScaledSliceMoments> {
    let n = xs.len() as f64;
    let first = normalize_zero(xs[0]);
    let mut overflow = false;
    let mut max_diff = 0.0_f64;
    for &x in xs {
        let d = x - first;
        if !d.is_finite() {
            overflow = true;
            break;
        }
        max_diff = max_diff.max(d.abs());
    }
    let (origin, exp) = if overflow {
        (0.0, 1024)
    } else {
        (first, frexp_exp(max_diff))
    };

    let sum_z = neumaier_sum(xs.iter().map(|&x| ldexp(x - origin, -exp)))?;
    let mean_z = sum_z / n;

    let mut acc_r = NeumaierAccumulator::new();
    let mut acc_sq = NeumaierAccumulator::new();
    for &x in xs {
        let r = ldexp(x - origin, -exp) - mean_z;
        acc_r.add(r)?;
        acc_sq.add(r * r)?;
    }
    let sum_r = acc_r.get_total()?;
    let sum_sq = acc_sq.get_total()?;
    let ss_z = sum_sq - (sum_r * sum_r) / n;
    if !ss_z.is_finite() || ss_z <= 0.0 {
        return Err(crate::Error::Internal(
            "Centered second moment failed to remain finite and strictly positive".into(),
        ));
    }

    Ok(ScaledSliceMoments {
        origin,
        exp,
        mean_z,
        sum_r,
        ss_z,
    })
}

/// Arithmetic mean using shifted, power-of-two scaled Neumaier summation.
///
/// Validates that all inputs are finite and that the slice length does not
/// exceed the exact binary64 count limit (`2^53`). Returns `Ok(None)` when `xs`
/// is empty.
///
/// # Arguments
///
/// * `xs` - Observations to average. All entries must be finite.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `xs` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `xs.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if the scaled result cannot be
///   reconstructed as a finite `f64`.
pub fn mean(xs: &[f64]) -> crate::Result<Option<f64>> {
    validate_finite_slice(xs)?;
    if xs.is_empty() {
        return Ok(None);
    }
    if let Some(c) = check_constant_slice(xs) {
        return Ok(Some(c));
    }
    let max_abs = xs.iter().copied().fold(0.0_f64, |m, x| m.max(x.abs()));
    let exp = frexp_exp(max_abs);
    let mut acc = NeumaierAccumulator::new();
    for &x in xs {
        acc.add(ldexp(x, -exp))?;
    }
    let (sum, c) = acc.sum_and_compensation();
    let hi = sum + c;
    if !hi.is_finite() {
        return Err(crate::Error::Internal(
            "Arithmetic mean overflowed to a non-finite value".into(),
        ));
    }
    let lo = if sum.abs() >= c.abs() {
        (sum - hi) + c
    } else {
        (c - hi) + sum
    };
    let n = xs.len() as f64;
    let q_hi = hi / n;
    let rem = (-q_hi).mul_add(n, hi) + lo;
    let m = ldexp(q_hi + rem / n, exp);
    if !m.is_finite() {
        return Err(crate::Error::Internal(
            "Arithmetic mean overflowed to a non-finite value".into(),
        ));
    }
    Ok(Some(normalize_zero(m)))
}

/// Sample variance (unbiased, `n - 1` denominator) using shifted, scaled
/// two-pass compensated summation.
///
/// Validates all observations up front. Returns `Ok(None)` when `xs` has fewer
/// than two observations, and `Ok(Some(0.0))` when `xs.len() >= 2` and all
/// observations are identical.
///
/// # Arguments
///
/// * `xs` - Sample observations whose unbiased variance is required.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `xs` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `xs.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if a non-constant sample's variance
///   overflows or underflows to a non-finite or zero `f64`.
pub fn variance(xs: &[f64]) -> crate::Result<Option<f64>> {
    validate_finite_slice(xs)?;
    if xs.len() < 2 {
        return Ok(None);
    }
    if check_constant_slice(xs).is_some() {
        return Ok(Some(0.0));
    }
    let m = scaled_slice_moments(xs)?;
    let var = ldexp(m.ss_z / (xs.len() - 1) as f64, 2 * m.exp);
    if !var.is_finite() || var <= 0.0 {
        return Err(crate::Error::Internal(
            "Sample variance overflowed or underflowed out of the positive finite range".into(),
        ));
    }
    Ok(Some(var))
}

/// Population variance (`n` denominator) using shifted, scaled two-pass
/// compensated summation.
///
/// Validates all observations up front. Returns `Ok(None)` when `xs` is empty,
/// and `Ok(Some(0.0))` when `xs` has one element or all elements are equal.
///
/// # Arguments
///
/// * `xs` - Complete population of observations.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `xs` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `xs.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if a non-constant sample's population
///   variance overflows or underflows out of the positive finite range.
pub fn population_variance(xs: &[f64]) -> crate::Result<Option<f64>> {
    validate_finite_slice(xs)?;
    if xs.is_empty() {
        return Ok(None);
    }
    if xs.len() == 1 || check_constant_slice(xs).is_some() {
        return Ok(Some(0.0));
    }
    let m = scaled_slice_moments(xs)?;
    let var = ldexp(m.ss_z / xs.len() as f64, 2 * m.exp);
    if !var.is_finite() || var <= 0.0 {
        return Err(crate::Error::Internal(
            "Population variance overflowed or underflowed out of the positive finite range".into(),
        ));
    }
    Ok(Some(var))
}

/// Sample standard deviation (square root of unbiased sample variance) using
/// shifted, scaled moments without materializing squared variance first.
///
/// Validates all observations up front. Returns `Ok(None)` when `xs` has fewer
/// than two observations, and `Ok(Some(0.0))` when all observations are equal.
///
/// # Arguments
///
/// * `xs` - Sample observations whose unbiased sample standard deviation is required.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `xs` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `xs.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if a non-constant sample's standard
///   deviation overflows or underflows out of the positive finite range.
pub fn std_dev(xs: &[f64]) -> crate::Result<Option<f64>> {
    validate_finite_slice(xs)?;
    if xs.len() < 2 {
        return Ok(None);
    }
    if check_constant_slice(xs).is_some() {
        return Ok(Some(0.0));
    }
    let m = scaled_slice_moments(xs)?;
    let sd = ldexp((m.ss_z / (xs.len() - 1) as f64).sqrt(), m.exp);
    if !sd.is_finite() || sd <= 0.0 {
        return Err(crate::Error::Internal(
            "Sample standard deviation overflowed or underflowed out of the positive finite range"
                .into(),
        ));
    }
    Ok(Some(sd))
}

/// Return `(mean, variance)` pair with fieldwise `Option` availability.
///
/// Uses the canonical shifted, scaled estimators so empty input returns
/// `(None, None)`, a singleton returns `(Some(x0), None)`, and two or more
/// finite observations return `(Some(mean), Some(variance))`.
///
/// # Arguments
///
/// * `xs` - Sample observations used for both statistics.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `xs` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `xs.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if either requested estimate cannot be
///   represented as a valid finite `f64`.
pub fn mean_var(xs: &[f64]) -> crate::Result<(Option<f64>, Option<f64>)> {
    let m = mean(xs)?;
    let v = variance(xs)?;
    Ok((m, v))
}

/// Minimum finite observation, ignoring `NaN` and infinite entries.
///
/// Returns `None` when no finite observation exists. Does not validate sample
/// integrity; use ordinary statistical estimators when non-finite inputs
/// must be rejected.
///
/// # Arguments
///
/// * `xs` - Observations to inspect for the smallest finite entry.
#[must_use]
pub fn finite_min(xs: &[f64]) -> Option<f64> {
    let mut min: Option<f64> = None;
    for &x in xs {
        if x.is_finite() {
            min = Some(match min {
                Some(cur) if cur <= x => cur,
                _ => normalize_zero(x),
            });
        }
    }
    min
}

/// Maximum finite observation, ignoring `NaN` and infinite entries.
///
/// Returns `None` when no finite observation exists. Does not validate sample
/// integrity; use ordinary statistical estimators when non-finite inputs
/// must be rejected.
///
/// # Arguments
///
/// * `xs` - Observations to inspect for the largest finite entry.
#[must_use]
pub fn finite_max(xs: &[f64]) -> Option<f64> {
    let mut max: Option<f64> = None;
    for &x in xs {
        if x.is_finite() {
            max = Some(match max {
                Some(cur) if cur >= x => cur,
                _ => normalize_zero(x),
            });
        }
    }
    max
}

/// Count finite observations, excluding `NaN` and infinite entries.
///
/// # Arguments
///
/// * `xs` - Observations to inspect. Only finite floating-point values are counted.
#[must_use]
pub fn finite_count(xs: &[f64]) -> usize {
    xs.iter().copied().filter(|x| x.is_finite()).count()
}

fn validate_paired_finite(x: &[f64], y: &[f64]) -> crate::Result<()> {
    if x.len() != y.len() {
        return Err(crate::InputError::DimensionMismatch.into());
    }
    validate_finite_slice(x)?;
    validate_finite_slice(y)?;
    Ok(())
}

struct PairedScaledMoments {
    exp_x: i32,
    exp_y: i32,
    ss_zx: f64,
    ss_zy: f64,
    cp_z: f64,
}

fn paired_scaled_moments(x: &[f64], y: &[f64]) -> crate::Result<PairedScaledMoments> {
    let mx = scaled_slice_moments(x)?;
    let my = scaled_slice_moments(y)?;
    let n = x.len() as f64;
    let mut acc_cp = NeumaierAccumulator::new();
    for (&xi, &yi) in x.iter().zip(y.iter()) {
        let rx = ldexp(xi - mx.origin, -mx.exp) - mx.mean_z;
        let ry = ldexp(yi - my.origin, -my.exp) - my.mean_z;
        acc_cp.add(rx * ry)?;
    }
    let cp_z = acc_cp.get_total()? - (mx.sum_r * my.sum_r) / n;
    if !cp_z.is_finite() {
        return Err(crate::Error::Internal(
            "Paired co-moment overflowed to a non-finite value".into(),
        ));
    }
    Ok(PairedScaledMoments {
        exp_x: mx.exp,
        exp_y: my.exp,
        ss_zx: mx.ss_z,
        ss_zy: my.ss_z,
        cp_z,
    })
}

/// Sample covariance (unbiased, `n - 1` denominator) between two equal-length
/// finite slices.
///
/// Validates length equality and finiteness of both slices before checking `n < 2`.
/// Returns `Ok(None)` when fewer than two paired observations are supplied, and
/// `Ok(Some(0.0))` when either slice is constant.
///
/// # Arguments
///
/// * `x` - First observation series, paired element-by-element with `y`.
/// * `y` - Second observation series, matching `x.len()`.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::DimensionMismatch`]
///   if `x.len() != y.len()`.
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `x` or `y` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `x.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if a nonzero co-moment cannot be scaled
///   back to a nonzero finite covariance.
pub fn covariance(x: &[f64], y: &[f64]) -> crate::Result<Option<f64>> {
    validate_paired_finite(x, y)?;
    if x.len() < 2 {
        return Ok(None);
    }
    if check_constant_slice(x).is_some() || check_constant_slice(y).is_some() {
        return Ok(Some(0.0));
    }
    let p = paired_scaled_moments(x, y)?;
    if p.cp_z == 0.0 {
        return Ok(Some(0.0));
    }
    let raw = p.cp_z / (x.len() - 1) as f64;
    let Some(exp_sum) = p.exp_x.checked_add(p.exp_y) else {
        return Err(crate::Error::Internal(
            "Covariance exponent sum overflowed".into(),
        ));
    };
    let cov = ldexp(raw, exp_sum);
    if !cov.is_finite() || cov == 0.0 {
        return Err(crate::Error::Internal(
            "Sample covariance overflowed or underflowed out of the nonzero finite range".into(),
        ));
    }
    Ok(Some(normalize_zero(cov)))
}

/// Reconstruct exact binary64-rational Pearson correlation without endpoint clamping.
fn exact_rational_pearson(x: &[f64], y: &[f64]) -> crate::Result<Option<f64>> {
    let mut xs = Vec::with_capacity(x.len());
    let mut ys = Vec::with_capacity(y.len());
    for (&xi, &yi) in x.iter().zip(y.iter()) {
        xs.push(BigRational::from_float(xi).ok_or_else(|| {
            crate::Error::from(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(xi),
            })
        })?);
        ys.push(BigRational::from_float(yi).ok_or_else(|| {
            crate::Error::from(crate::InputError::NonFiniteValue {
                kind: crate::NonFiniteKind::classify(yi),
            })
        })?);
    }
    let count = BigRational::from_integer(BigInt::from(xs.len()));
    let mx = xs.iter().cloned().sum::<BigRational>() / &count;
    let my = ys.iter().cloned().sum::<BigRational>() / &count;
    let mut sxx = BigRational::zero();
    let mut sxy = BigRational::zero();
    let mut syy = BigRational::zero();
    for (xi, yi) in xs.iter().zip(&ys) {
        let dx = xi - &mx;
        let dy = yi - &my;
        sxx += &dx * &dx;
        sxy += &dx * &dy;
        syy += &dy * &dy;
    }
    if sxx.is_zero() || syy.is_zero() {
        return Ok(None);
    }
    if sxy.is_zero() {
        return Ok(Some(0.0));
    }
    let square = (&sxy * &sxy) / (&sxx * &syy);
    let exponent = (square.numer().bits() as i128 - square.denom().bits() as i128) as i32;
    let half = exponent.div_euclid(2);
    let scaled = if half < 0 {
        &square * BigRational::from_integer(BigInt::one() << ((-2 * half) as usize))
    } else {
        &square / BigRational::from_integer(BigInt::one() << ((2 * half) as usize))
    };
    let normalized = scaled.to_f64().ok_or_else(|| {
        crate::Error::Internal("Exact Pearson reconstruction failed conversion to binary64".into())
    })?;
    let mag = ldexp(normalized.sqrt(), half);
    if !mag.is_finite() || !(0.0..=1.0).contains(&mag) || mag == 0.0 {
        return Err(crate::Error::Internal(format!(
            "Exact Pearson reconstruction yielded invalid magnitude {mag}"
        )));
    }
    let signed = if sxy.is_negative() { -mag } else { mag };
    Ok(Some(normalize_zero(signed)))
}

/// Pearson correlation coefficient between two equal-length finite slices.
///
/// Validates length equality and finiteness of both slices before checking
/// sample count or constant-series rules. Returns `Ok(None)` when `x.len() < 2`
/// or either series is constant. Never clamps values outside `[-1, 1]`.
///
/// # Arguments
///
/// * `x` - First observation series, paired element-by-element with `y`.
/// * `y` - Second observation series, matching `x.len()`.
///
/// # Errors
///
/// * Returns [`crate::Error::Input`] with [`crate::InputError::DimensionMismatch`]
///   if `x.len() != y.len()`.
/// * Returns [`crate::Error::Input`] with [`crate::InputError::NonFiniteValue`]
///   when any observation in `x` or `y` is `NaN` or infinite.
/// * Returns [`crate::Error::Validation`] if `x.len()` exceeds the exact count
///   limit (`2^53`).
/// * Returns [`crate::Error::Internal`] if numerical evaluation yields a
///   non-finite value or a correlation outside `[-1.0, 1.0]`.
pub fn correlation(x: &[f64], y: &[f64]) -> crate::Result<Option<f64>> {
    validate_paired_finite(x, y)?;
    if x.len() < 2 || check_constant_slice(x).is_some() || check_constant_slice(y).is_some() {
        return Ok(None);
    }
    let p = paired_scaled_moments(x, y)?;
    let denom = p.ss_zx.sqrt() * p.ss_zy.sqrt();
    let r = if denom.is_finite() && denom > 0.0 {
        p.cp_z / denom
    } else {
        f64::NAN
    };
    if !r.is_finite()
        || r.abs() >= 1.0 - 16.0 * f64::EPSILON
        || p.cp_z.abs() <= f64::EPSILON.sqrt() * denom
    {
        return exact_rational_pearson(x, y);
    }
    Ok(Some(normalize_zero(r)))
}
