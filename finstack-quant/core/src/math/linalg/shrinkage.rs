//! Scaled-identity Ledoit–Wolf covariance estimator.

use super::square_len;
use crate::math::stats::{MAX_EXACT_COUNT, frexp_exp, ldexp};
use crate::math::summation::{NeumaierAccumulator, neumaier_sum};
use crate::{Error, Result};

/// Two-term expansions retain the leading rounding residual when a Frobenius
/// energy is subtracted from a nearly equal competing energy at the cap.
#[derive(Clone, Copy, Default)]
struct Pair {
    hi: f64,
    lo: f64,
}

impl Pair {
    fn sum(a: f64, b: f64) -> Self {
        let hi = a + b;
        let z = hi - a;
        Self {
            hi,
            lo: (a - (hi - z)) + (b - z),
        }
    }

    fn product(a: f64, b: f64) -> Self {
        let hi = a * b;
        Self {
            hi,
            lo: a.mul_add(b, -hi),
        }
    }

    fn sub(self, rhs: Self) -> Self {
        let d = Self::sum(self.hi, -rhs.hi);
        Self::sum(d.hi, d.lo + (self.lo - rhs.lo))
    }

    fn scaled(self, exponent: i32) -> Self {
        Self {
            hi: ldexp(self.hi, exponent),
            lo: ldexp(self.lo, exponent),
        }
    }

    fn divided_by(self, value: f64) -> Self {
        let q = self.hi / value;
        Self::sum(q, ((-q).mul_add(value, self.hi) + self.lo) / value)
    }

    fn from_acc(acc: NeumaierAccumulator) -> Self {
        let (hi, lo) = acc.sum_and_compensation();
        Self::sum(hi, lo)
    }
}

/// Compensated sum of squares in units of 2^(2*exponent).
#[derive(Default)]
struct ScaledSquares {
    exponent: Option<i32>,
    sum: NeumaierAccumulator,
}

impl ScaledSquares {
    fn add(&mut self, value: Pair, units: i32, multiplicity: f64) -> Result<()> {
        if value.hi == 0.0 {
            return Ok(());
        }
        let exponent = units + frexp_exp(value.hi);
        let previous = *self.exponent.get_or_insert(exponent);
        if exponent > previous {
            let rescaled = Pair::from_acc(self.sum).scaled(2 * (previous - exponent));
            self.sum = NeumaierAccumulator::new();
            self.sum.add(rescaled.hi)?;
            self.sum.add(rescaled.lo)?;
            self.exponent = Some(exponent);
        }
        let v = value.scaled(units - exponent.max(previous));
        let sq = Pair::product(v.hi, v.hi);
        self.sum.add(multiplicity * sq.hi)?;
        self.sum.add(multiplicity * (sq.lo + 2.0 * v.hi * v.lo))
    }
}

/// Compare B²/T² and D² in common exponent units, preserving a small complement.
fn weights(b: ScaledSquares, d: ScaledSquares, rows: f64) -> Result<(f64, f64)> {
    let (Some(be), Some(de)) = (b.exponent, d.exponent) else {
        // An isotropic sample or identical row outer products yields the sample.
        return Ok((0.0, 1.0));
    };
    let b2 = Pair::from_acc(b.sum).divided_by(rows).divided_by(rows);
    let d2 = Pair::from_acc(d.sum);
    let shift = 2 * (be - de);
    let power_gap = frexp_exp(b2.hi) + shift - frexp_exp(d2.hi);
    if power_gap > 1 {
        return Ok((1.0, 0.0));
    }
    let ratio = ldexp(b2.hi / d2.hi, shift);
    if ratio == 0.0 {
        return Err(Error::Internal(
            "positive shrinkage intensity underflowed".into(),
        ));
    }
    if power_gap < -1 {
        return Ok((ratio, 1.0 - ratio));
    }
    let difference = d2.sub(b2.scaled(shift)).hi;
    if difference <= 0.0 {
        return Ok((1.0, 0.0));
    }
    let sample = difference / d2.hi;
    Ok((if ratio >= 0.5 { 1.0 - sample } else { ratio }, sample))
}

// Keep the final sample and target contributions in a common binary scale until
// their sum has been formed. Individually they can be below the subnormal floor.
fn weighted_covariance(
    sample: f64,
    exp: i32,
    sample_weight: f64,
    target: f64,
    target_exp: i32,
    delta: f64,
) -> Result<f64> {
    let weighted = |value: f64, units: i32, weight: f64| {
        if value == 0.0 || weight == 0.0 {
            return (0.0, i32::MIN / 2);
        }
        let ve = frexp_exp(value);
        let we = frexp_exp(weight);
        (ldexp(value, -ve) * ldexp(weight, -we), units + ve + we)
    };
    let (s, se) = weighted(sample, exp, sample_weight);
    let (m, me) = weighted(target, target_exp, delta);
    let e = se.max(me);
    let scaled = ldexp(s, se - e) + ldexp(m, me - e);
    let result = ldexp(scaled, e);
    if !result.is_finite() || (result == 0.0 && scaled != 0.0) {
        return Err(Error::Internal(
            "shrunk covariance outside the representable range".into(),
        ));
    }
    Ok(result)
}

/// Result of Ledoit-Wolf covariance shrinkage.
///
/// See [`ledoit_wolf_shrinkage`].
#[derive(Debug, Clone, PartialEq)]
pub struct LedoitWolfResult {
    /// Row-major `n × n` shrunk covariance matrix `Σ* = δ*·μ·I + (1 − δ*)·S`.
    pub covariance: Vec<f64>,
    /// Estimated shrinkage intensity `δ* ∈ [0, 1]`.
    pub shrinkage: f64,
}

/// Ledoit-Wolf (2004) shrinkage of a sample covariance matrix toward a scaled
/// identity target, with the estimated shrinkage intensity.
///
/// Given `t` observations of `n` variables (row-major `observations`, one
/// observation per row), columns are demeaned, the sample covariance
/// `S = XᵀX/T` is formed, and the estimator
///
/// ```text
/// Σ* = δ*·μ·I + (1 − δ*)·S
///
/// μ   = tr(S)/n                       (⟨S, I⟩ under ⟨A,B⟩ = tr(ABᵀ)/n)
/// d²  = ‖S − μI‖²                     (‖A‖² = tr(AAᵀ)/n)
/// b̄²  = (1/T²)·Σ_t ‖x_t x_tᵀ − S‖²
/// b²  = min(b̄², d²)
/// δ*  = b²/d²                          (δ* = 0 when d² = 0, i.e. S = μI)
/// ```
///
/// is returned. `Σ*` is a convex combination of the PSD `S` and the PSD `μI`,
/// hence positive semidefinite in real arithmetic.
/// Separate binary exponents preserve the relative units of the centered columns
/// and keep the Frobenius norm ratio stable under common rescaling.
/// The computation is a deterministic serial fold: identical inputs produce
/// bit-identical outputs. Complexity: O(t·n²) time, O(t·n + n²) space.
///
/// # Arguments
///
/// * `observations` - Row-major `t × n` observation matrix (row = one date)
/// * `t` - Number of observations (rows); must be ≥ 2
/// * `n` - Number of variables (columns); must be ≥ 1
///
/// # Errors
///
/// Returns [`crate::Error::Validation`] when `t < 2`, `n == 0`, any entry is
/// non-finite, `observations.len() != t * n`, the `n * n` covariance size
/// overflows `usize`. A numerical error is returned if a required result
/// cannot be represented or the intensity cannot be resolved.
///
/// # Examples
///
/// ```
/// use finstack_quant_core::math::linalg::ledoit_wolf_shrinkage;
///
/// // 4 observations of 2 zero-mean variables (row-major).
/// let x = [1.0, 1.0, -1.0, -1.0, 2.0, -2.0, -2.0, 2.0];
/// let result = ledoit_wolf_shrinkage(&x, 4, 2).unwrap();
/// assert!((result.shrinkage - 17.0 / 18.0).abs() < 1e-14);
/// assert!((result.covariance[1] - (-1.0 / 12.0)).abs() < 1e-13);
/// ```
///
/// # References
///
/// - Ledoit, O., & Wolf, M. (2004). "A well-conditioned estimator for
///   large-dimensional covariance matrices." *Journal of Multivariate
///   Analysis*, 88(2), 365–411. Lemmas 3.2–3.4. `docs/REFERENCES.md#ledoitwolf2004`
pub fn ledoit_wolf_shrinkage(
    observations: &[f64],
    t: usize,
    n: usize,
) -> crate::Result<LedoitWolfResult> {
    if n == 0 {
        return Err(crate::Error::Validation(
            "ledoit_wolf_shrinkage: n must be >= 1".to_owned(),
        ));
    }
    if t < 2 {
        return Err(crate::Error::Validation(format!(
            "ledoit_wolf_shrinkage: need at least 2 observations, got {t}"
        )));
    }
    if t.checked_mul(n) != Some(observations.len()) {
        return Err(crate::Error::Validation(format!(
            "ledoit_wolf_shrinkage: {} observations do not form a {t}x{n} panel",
            observations.len()
        )));
    }
    if square_len(n).is_none() {
        return Err(crate::Error::Validation(format!(
            "ledoit_wolf_shrinkage: a {n}x{n} covariance matrix does not fit in memory"
        )));
    }
    if let Some(bad) = observations.iter().find(|v| !v.is_finite()) {
        return Err(crate::Error::Validation(format!(
            "ledoit_wolf_shrinkage: non-finite observation {bad}"
        )));
    }

    if t > MAX_EXACT_COUNT || n > MAX_EXACT_COUNT {
        return Err(Error::Validation(
            "Ledoit-Wolf dimension exceeds exact count limit".into(),
        ));
    }
    let tf = t as f64;
    let mut x = Vec::new();
    x.try_reserve_exact(observations.len())
        .map_err(|_| Error::Validation("Ledoit-Wolf panel exceeds available storage".into()))?;
    x.extend_from_slice(observations);
    let mut exponents = vec![0; n];
    for (col, column_exp) in exponents.iter_mut().enumerate() {
        let values = || observations.iter().skip(col).step_by(n).copied();
        let lo = values().fold(f64::INFINITY, f64::min);
        let hi = values().fold(f64::NEG_INFINITY, f64::max);
        // Shift clustered values toward zero before taking their mean; a column
        // spanning zero already has a suitable origin for its binary scaling.
        let origin = 0.0_f64.clamp(lo, hi);
        let spread = (lo - origin).abs().max((hi - origin).abs());
        if spread == 0.0 {
            x.iter_mut().skip(col).step_by(n).for_each(|v| *v = 0.0);
            continue;
        }
        let exp = frexp_exp(spread);
        *column_exp = exp;
        let shifted = |v: f64| ldexp(v - origin, -exp);
        let center = neumaier_sum(values().map(shifted))? / tf;
        for value in x.iter_mut().skip(col).step_by(n) {
            *value = shifted(*value) - center;
        }
    }

    // Store S_ij in units of 2^(e_i+e_j), including small diagonal entries.
    let mut s = Vec::new();
    s.try_reserve_exact(n * n).map_err(|_| {
        Error::Validation("Ledoit-Wolf covariance exceeds available storage".into())
    })?;
    s.resize(n * n, Pair::default());
    let mut b = ScaledSquares::default();
    for i in 0..n {
        for j in i..n {
            let products = || x.chunks_exact(n).map(|row| Pair::product(row[i], row[j]));
            let mut sum = NeumaierAccumulator::new();
            for product in products() {
                sum.add(product.hi)?;
                sum.add(product.lo)?;
            }
            let mean = Pair::from_acc(sum).divided_by(tf);
            s[i * n + j] = mean;
            s[j * n + i] = mean;
            let multiplicity = if i == j { 1.0 } else { 2.0 };
            for prod in products() {
                let deviation = prod.sub(mean);
                b.add(deviation, exponents[i] + exponents[j], multiplicity)?;
            }
        }
    }
    let mut covariance = Vec::new();
    covariance.try_reserve_exact(n * n).map_err(|_| {
        Error::Validation("Ledoit-Wolf covariance exceeds available storage".into())
    })?;
    covariance.resize(n * n, 0.0);
    let Some(mu_exp) = (0..n)
        .filter(|&i| s[i * n + i].hi > 0.0)
        .map(|i| 2 * exponents[i] + frexp_exp(s[i * n + i].hi))
        .max()
    else {
        return Ok(LedoitWolfResult {
            covariance,
            shrinkage: 0.0,
        });
    };
    let diagonal = |i: usize| s[i * n + i].scaled(2 * exponents[i] - mu_exp);
    let mut trace = NeumaierAccumulator::new();
    for i in 0..n {
        let var = diagonal(i);
        trace.add(var.hi)?;
        trace.add(var.lo)?;
    }
    let mu = Pair::from_acc(trace).divided_by(n as f64);
    let mut d = ScaledSquares::default();
    for i in 0..n {
        d.add(diagonal(i).sub(mu), mu_exp, 1.0)?;
        for j in 0..i {
            d.add(s[i * n + j], exponents[i] + exponents[j], 2.0)?;
        }
    }
    let (shrinkage, sample_weight) = weights(b, d, tf)?;
    for i in 0..n {
        for j in i..n {
            let exp = exponents[i] + exponents[j];
            let target = if i == j { mu.hi } else { 0.0 };
            let value = weighted_covariance(
                s[i * n + j].hi,
                exp,
                sample_weight,
                target,
                mu_exp,
                shrinkage,
            )?;
            covariance[i * n + j] = value;
            covariance[j * n + i] = value;
        }
    }
    Ok(LedoitWolfResult {
        covariance,
        shrinkage,
    })
}
