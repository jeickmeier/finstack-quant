//! Fractional Brownian motion (fBM) primitives and kernel functions.
//!
//! This module provides the mathematical building blocks for rough volatility
//! models, including:
//!
//! - **Hurst exponent** — validated parameter H ∈ (0, 1) controlling path roughness
//! - **Covariance functions** — fBM increment covariance and the increment
//!   covariance matrix consumed by the fBM path generators
//!
//! # Background
//!
//! Fractional Brownian motion B_H is a centered Gaussian process with covariance
//!
//! $$\operatorname{Cov}(B_H(t), B_H(s)) = \tfrac{1}{2}\bigl(|t|^{2H} + |s|^{2H} - |t-s|^{2H}\bigr)$$
//!
//! where H ∈ (0, 1) is the Hurst exponent. When H = 0.5 this reduces to standard
//! Brownian motion. When H < 0.5 the paths are rougher than Brownian motion, which
//! is the empirically observed regime for equity volatility.
//!
//! # References
//!
//! - Mandelbrot, B. & Van Ness, J. (1968). Fractional Brownian motions, fractional
//!   noises and applications. *SIAM Review*, 10(4), 422–437. `docs/REFERENCES.md#mandelbrot-van-ness-1968`
//! - Bayer, C., Friz, P. & Gatheral, J. (2016). Pricing under rough volatility.
//!   *Quantitative Finance*, 16(6), 887–904. `docs/REFERENCES.md#bayer-friz-gatheral-2016`
//! - El Euch, O. & Rosenbaum, M. (2019). The characteristic function of rough Heston
//!   models. *Mathematical Finance*, 29(1), 3–38. `docs/REFERENCES.md#el-euch-rosenbaum-2019`

use crate::math::interp::exact_f64_eq;

use nalgebra::DMatrix;

use crate::{Error, Result};

/// Validated Hurst exponent H ∈ (0, 1).
///
/// The Hurst exponent determines the roughness of fractional Brownian motion:
///
/// - H < 0.5 — rough (anti-persistent increments)
/// - H = 0.5 — standard Brownian motion
/// - H > 0.5 — more regular paths with persistent increments
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawHurstExponent")]
#[cfg_attr(feature = "json-schema", schemars(try_from = "RawHurstExponent"))]
pub struct HurstExponent {
    /// The Hurst parameter value.
    h: f64,
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct RawHurstExponent {
    #[cfg_attr(feature = "json-schema", schemars(extend("exclusiveMinimum" = 0.0, "exclusiveMaximum" = 1.0)))]
    h: f64,
}

impl TryFrom<RawHurstExponent> for HurstExponent {
    type Error = Error;

    fn try_from(raw: RawHurstExponent) -> Result<Self> {
        Self::new(raw.h)
    }
}

impl HurstExponent {
    /// Create a new Hurst exponent, validating that H ∈ (0, 1) and is finite.
    ///
    /// # Arguments
    ///
    /// * `h` - Dimensionless Hurst exponent, strictly between zero and one;
    ///   values below one half describe rough, anti-persistent increments.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`] if `h` is not in the open interval (0, 1)
    /// or is not finite.
    pub fn new(h: f64) -> Result<Self> {
        if !h.is_finite() || h <= 0.0 || h >= 1.0 {
            return Err(Error::Validation(format!(
                "Hurst exponent must be in (0, 1), got {h}"
            )));
        }
        Ok(Self { h })
    }

    /// The raw Hurst parameter value.
    pub fn get_value(&self) -> f64 {
        self.h
    }

    /// Returns `true` when the exponent describes a rough process (H < 0.5).
    pub fn is_rough(&self) -> bool {
        self.h < 0.5
    }
}

/// Covariance of two unit-normalized fractional Brownian increments.
///
/// # Arguments
///
/// * `ti` - Finite nonnegative start of the first interval in elapsed model time.
/// * `ti1` - Finite end of the first interval, at or after `ti`.
/// * `tj` - Finite nonnegative start of the second interval, in the same units.
/// * `tj1` - Finite end of the second interval, at or after `tj`.
/// * `h` - Validated Hurst exponent. Covariance is in units of time raised to `2H`.
///
/// # Errors
///
/// Invalid intervals give a validation error. Unresolved cancellation, underflow
/// of a nonzero covariance, and overflow give computation errors.
pub fn fbm_increment_covariance(
    ti: f64,
    ti1: f64,
    tj: f64,
    tj1: f64,
    h: HurstExponent,
) -> Result<f64> {
    if [ti, ti1, tj, tj1]
        .iter()
        .any(|t| !t.is_finite() || *t < 0.0)
        || ti1 < ti
        || tj1 < tj
    {
        return Err(Error::Validation(format!(
            "invalid fractional Brownian increment intervals [{ti}, {ti1}], [{tj}, {tj1}]"
        )));
    }
    let ((a, b), (c, d)) = if (ti, ti1) <= (tj, tj1) {
        ((ti, ti1), (tj, tj1))
    } else {
        ((tj, tj1), (ti, ti1))
    };
    let x = b - a;
    let y = d - c;
    if x == 0.0 || y == 0.0 {
        return Ok(0.0);
    }
    let p = 2.0 * h.get_value();
    if exact_f64_eq(p, 1.0) {
        return Ok((b.min(d) - c).max(0.0));
    }

    let value = if exact_f64_eq(a, c) && exact_f64_eq(b, d) {
        x.powf(p)
    } else if exact_f64_eq(b, c) {
        touching_covariance(x, y, p)
    } else {
        let distances = [(b - c).abs(), d - a, (b - d).abs(), c - a];
        let scale = distances.iter().copied().fold(0.0, f64::max);
        let mut terms = [0.0; 4];
        for (term, distance) in terms.iter_mut().zip(distances) {
            if distance > 0.0 {
                *term = (p * log_ratio(distance, scale)).exp();
                if *term == 0.0 {
                    return Err(Error::Internal(format!(
                        "fractional covariance lost a scaled distance power for [{a}, {b}], [{c}, {d}]"
                    )));
                }
            }
        }
        let [q1, q2, q3, q4] = terms;
        let sum = super::summation::neumaier_sum([q1, q2, -q3, -q4])?;
        let magnitude = super::summation::neumaier_sum(terms)?;
        if sum.abs() <= 8.0 * f64::EPSILON * magnitude {
            return Err(Error::Internal(format!(
                "unresolved fractional covariance cancellation for [{a}, {b}], [{c}, {d}]"
            )));
        }
        let log_abs = p.mul_add(scale.ln(), (0.5 * sum.abs()).ln());
        log_abs.exp().copysign(sum)
    };
    if !value.is_finite()
        || value == 0.0
        || (value < 0.0 && (p > 1.0 || (a, b) == (c, d)))
        || (value > 0.0 && p < 1.0 && b <= c)
    {
        return Err(Error::Internal(format!(
            "unrepresentable fractional covariance or wrong sign for [{a}, {b}], [{c}, {d}] at H={}",
            h.get_value()
        )));
    }
    Ok(value)
}

/// Log of small/large, preserving close ratios and extreme exponent differences.
fn log_ratio(small: f64, large: f64) -> f64 {
    let ratio = small / large;
    if ratio >= 0.5 {
        ((small - large) / large).ln_1p()
    } else if ratio.is_normal() {
        ratio.ln()
    } else {
        small.ln() - large.ln()
    }
}

/// With r=min/max and e=p-1, the two contributions
/// (1+r)expm1(e log(1+r)) and -r expm1(e log r) share the sign of e.
/// Accumulating their log magnitudes defers the potentially out-of-range
/// product with max^p until the requested covariance can be restored.
fn touching_covariance(x: f64, y: f64, p: f64) -> f64 {
    let large = x.max(y);
    let small = x.min(y);
    let e = p - 1.0;
    if exact_f64_eq(x, y) {
        let weight = (e * std::f64::consts::LN_2).exp_m1();
        let scale = large.powf(p);
        if scale.is_normal() {
            return scale * weight;
        }
        return p.mul_add(large.ln(), weight.abs().ln()).exp().copysign(e);
    }
    let log_r = log_ratio(small, large);
    let r = small / large;
    let log_one_plus_r = r.ln_1p();
    let log_first = if r < f64::EPSILON {
        // At this scale log1p(r) and expm1(e*r) reduce to their linear terms.
        e.abs().ln() + log_r
    } else {
        log_one_plus_r + (e * log_one_plus_r).exp_m1().abs().ln()
    };
    let z = e * log_r;
    let log_second = if z > 0.0 {
        // r * (exp(z)-1) = r^p * (1-exp(-z)).
        p.mul_add(log_r, (-(-z).exp_m1()).ln())
    } else {
        log_r + (-z.exp_m1()).ln()
    };
    let hi = log_first.max(log_second);
    let lo = log_first.min(log_second);
    let log_weight = hi + (lo - hi).exp().ln_1p() - std::f64::consts::LN_2;
    p.mul_add(large.ln(), log_weight).exp().copysign(e)
}

/// Dense symmetric covariance matrix of forward increments on a time grid.
///
/// # Arguments
///
/// * `times` - Strictly increasing finite nonnegative times in one elapsed-time
///   unit. Empty and singleton grids produce a zero-by-zero matrix.
/// * `h` - Validated Hurst exponent. Entry `(i,j)` measures the covariance of
///   increments on intervals `i` and `j` in units of time raised to `2H`.
///
/// # Errors
///
/// Invalid grids and matrix dimensions give validation errors. Allocation
/// failures and unresolved entries give computation errors.
pub fn fbm_increment_covariance_matrix(times: &[f64], h: HurstExponent) -> Result<DMatrix<f64>> {
    if times.iter().any(|t| !t.is_finite() || *t < 0.0) || times.windows(2).any(|w| w[0] >= w[1]) {
        return Err(Error::Validation(
            "fractional covariance times must be finite, nonnegative and strictly increasing"
                .into(),
        ));
    }
    let n = times.len().saturating_sub(1);
    let cells = n
        .checked_mul(n)
        .filter(|&count| std::alloc::Layout::array::<f64>(count).is_ok())
        .ok_or_else(|| {
            Error::Validation(format!(
                "fractional covariance matrix dimension {n} overflows"
            ))
        })?;
    let mut values = Vec::new();
    values.try_reserve_exact(cells).map_err(|err| {
        Error::Internal(format!(
            "cannot allocate {n}-by-{n} fractional covariance: {err}"
        ))
    })?;
    values.resize(cells, 0.0);
    for j in 0..n {
        for i in j..n {
            let covariance =
                fbm_increment_covariance(times[j], times[j + 1], times[i], times[i + 1], h)
                    .map_err(|err| {
                        Error::Internal(format!("fractional covariance entry ({i}, {j}): {err}"))
                    })?;
            values[j * n + i] = covariance;
            values[i * n + j] = covariance;
        }
    }
    Ok(DMatrix::from_vec(n, n, values))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-10;

    // -- HurstExponent validation ------------------------------------------

    #[test]
    fn hurst_valid() {
        let h = HurstExponent::new(0.1).unwrap();
        assert!((h.get_value() - 0.1).abs() < TOL);
        assert!(h.is_rough());
    }

    #[test]
    fn hurst_half() {
        let h = HurstExponent::new(0.5).unwrap();
        assert!(!h.is_rough());
    }

    #[test]
    fn hurst_reject_zero() {
        assert!(HurstExponent::new(0.0).is_err());
    }

    #[test]
    fn hurst_reject_one() {
        assert!(HurstExponent::new(1.0).is_err());
    }

    #[test]
    fn hurst_reject_negative() {
        assert!(HurstExponent::new(-0.3).is_err());
    }

    #[test]
    fn hurst_reject_nan() {
        assert!(HurstExponent::new(f64::NAN).is_err());
    }

    #[test]
    fn hurst_reject_infinity() {
        assert!(HurstExponent::new(f64::INFINITY).is_err());
    }

    #[test]
    fn hurst_serde_rejects_values_outside_constructor_domain() {
        use serde::Deserialize;

        for h in [-0.5, 0.0, 1.0, 1.5] {
            let json = format!("{{\"h\":{h}}}");
            assert!(serde_json::from_str::<HurstExponent>(&json).is_err());
        }
        for json in [r#"{"h":null}"#, r#"{"h":1e999}"#] {
            assert!(serde_json::from_str::<HurstExponent>(json).is_err());
        }
        // Formats other than JSON can carry IEEE non-finite numbers directly.
        for h in [f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
            let deserializer = serde::de::value::MapDeserializer::<_, serde::de::value::Error>::new(
                [("h", h)].into_iter(),
            );
            assert!(HurstExponent::deserialize(deserializer).is_err());
        }
    }

    #[cfg(feature = "json-schema")]
    #[test]
    fn hurst_schema_matches_constructor_bounds() {
        let schema = serde_json::to_value(schemars::schema_for!(HurstExponent)).unwrap();
        let shape = match schema.get("$ref").and_then(|r| r.as_str()) {
            Some(reference) => schema.pointer(reference.trim_start_matches('#')).unwrap(),
            None => &schema,
        };
        let h = &shape["properties"]["h"];
        assert_eq!(h["exclusiveMinimum"], 0.0);
        assert_eq!(h["exclusiveMaximum"], 1.0);
        assert_eq!(shape["additionalProperties"], false);
    }

    #[test]
    fn hurst_serde_round_trip_preserves_valid_exponents() {
        for h in [0.1, 0.5, 0.9] {
            let exponent = HurstExponent::new(h).expect("valid exponent");
            let json = serde_json::to_string(&exponent).expect("serialize exponent");
            let restored: HurstExponent = serde_json::from_str(&json).expect("validated exponent");
            assert_eq!(restored, exponent);
        }
    }

    fn covariance(a: f64, b: f64, c: f64, d: f64, h: f64) -> Result<f64> {
        fbm_increment_covariance(a, b, c, d, HurstExponent::new(h)?)
    }

    #[test]
    fn brownian_covariance_is_exact_overlap_at_any_origin() {
        let origin = (1_u64 << 53) as f64;
        assert_eq!(
            covariance(origin, origin + 2.0, origin, origin + 2.0, 0.5).unwrap(),
            2.0
        );
        assert_eq!(covariance(0.0, 2.0, 1.0, 3.0, 0.5).unwrap(), 1.0);
        assert_eq!(covariance(0.0, 2.0, 2.0, 3.0, 0.5).unwrap(), 0.0);
        assert_eq!(
            covariance(origin, origin + 2.0, origin, origin + 2.0, 0.25).unwrap(),
            2.0_f64.sqrt()
        );
    }

    #[test]
    fn touching_and_overlapping_intervals_have_resolved_signed_covariances() {
        for (a, b, c, d, h, expected) in [
            (0.0, 1.0, 1.0, 2.0, 0.25, 0.5 * 2.0_f64.sqrt() - 1.0),
            (0.0, 1.0, 1.0, 2.0, 0.75, 2.0_f64.sqrt() - 1.0),
            (0.0, 1.0, 1.0, 2.0, 0.5_f64.next_up(), 1.539095918623324e-16),
            (0.0, 2.0, 1.0, 3.0, 0.25, 0.5 * (3.0_f64.sqrt() - 1.0)),
            (0.0, 1.0, 1.0, 5.0, 0.25, 0.5 * (5.0_f64.sqrt() - 3.0)),
            // The length ratio underflows, yet the covariance tends to -sqrt(1e-300)/2.
            (0.0, 1e-300, 1e-300, 1e300, 0.25, -5e-151),
        ] {
            let value = covariance(a, b, c, d, h).unwrap();
            assert!(
                (value / expected - 1.0).abs() < 2e-13,
                "{value} vs {expected}"
            );
            assert_eq!(value, covariance(c, d, a, b, h).unwrap());
        }
    }

    #[test]
    fn unresolved_far_tails_and_range_failures_are_computation_errors() {
        // 100-digit reference for two unit increments with gap 1e8 is
        // 3.74999998125e-5. The four scaled terms cannot resolve it in binary64.
        assert!(matches!(
            covariance(0.0, 1.0, 1e8 + 1.0, 1e8 + 2.0, 0.75),
            Err(Error::Internal(_))
        ));
        assert!(matches!(
            covariance(0.0, 1e300, 0.0, 1e300, 0.75),
            Err(Error::Internal(_))
        ));
        assert!(matches!(
            covariance(0.0, 1e-300, 0.0, 1e-300, 0.75),
            Err(Error::Internal(_))
        ));
    }

    #[test]
    fn validate_all_intervals_and_grid_before_empty_identities() {
        let h = HurstExponent::new(0.25).unwrap();
        for (start, end) in [
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.0, f64::NAN),
            (0.0, f64::INFINITY),
        ] {
            assert!(matches!(
                covariance(0.0, 0.0, start, end, 0.25),
                Err(Error::Validation(_))
            ));
        }
        assert_eq!(covariance(1.0, 1.0, 3.0, 4.0, 0.25).unwrap(), 0.0);
        for grid in [
            &[f64::NAN][..],
            &[-1.0],
            &[0.0, 0.0],
            &[1.0, 0.5],
            &[0.0, f64::INFINITY],
        ] {
            assert!(matches!(
                fbm_increment_covariance_matrix(grid, h),
                Err(Error::Validation(_))
            ));
        }
        for grid in [&[][..], &[1.0]] {
            assert_eq!(
                fbm_increment_covariance_matrix(grid, h).unwrap().shape(),
                (0, 0)
            );
        }
    }

    #[test]
    fn irregular_covariance_matrix_mirrors_each_scalar_exactly() {
        let times = [2.0, 2.25, 2.5, 3.0, 5.0];
        let h = HurstExponent::new(0.3).unwrap();
        let cov = fbm_increment_covariance_matrix(&times, h).unwrap();
        assert_eq!(cov.shape(), (4, 4));
        for i in 0..cov.nrows() {
            assert_eq!(cov[(i, i)], (times[i + 1] - times[i]).powf(0.6));
            for j in 0..i {
                assert_eq!(cov[(i, j)].to_bits(), cov[(j, i)].to_bits());
                assert_eq!(
                    cov[(i, j)],
                    fbm_increment_covariance(times[i], times[i + 1], times[j], times[j + 1], h)
                        .unwrap()
                );
            }
        }
    }
}
