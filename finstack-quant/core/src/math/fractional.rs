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

use nalgebra::DMatrix;

use crate::{Error, Result};

/// Validated Hurst exponent H ∈ (0, 1).
///
/// The Hurst exponent determines the roughness of fractional Brownian motion:
///
/// - H < 0.5 — rough (anti-persistent increments)
/// - H = 0.5 — standard Brownian motion
/// - H > 0.5 — smooth (persistent increments)
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "RawHurstExponent")]
pub struct HurstExponent {
    /// The Hurst parameter value.
    h: f64,
}

#[derive(serde::Deserialize)]
struct RawHurstExponent {
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
    pub fn value(&self) -> f64 {
        self.h
    }

    /// The fractional index α = H + 0.5 used in Volterra-type representations.
    pub fn alpha(&self) -> f64 {
        self.h + 0.5
    }

    /// Returns `true` when the exponent describes a rough process (H < 0.5).
    pub fn is_rough(&self) -> bool {
        self.h < 0.5
    }
}
/// Covariance of fractional Brownian motion.
///
/// $$\operatorname{Cov}(B_H(t), B_H(s)) = \tfrac{1}{2}\bigl(|t|^{2H} + |s|^{2H} - |t-s|^{2H}\bigr)$$
///
/// # Arguments
///
/// * `t` - First time coordinate in the model's chosen time unit.
/// * `s` - Second time coordinate in the same time unit as `t`.
/// * `h` - Hurst exponent controlling roughness and long-memory behavior.
pub(crate) fn fbm_covariance(t: f64, s: f64, h: f64) -> f64 {
    let two_h = 2.0 * h;
    0.5 * (t.abs().powf(two_h) + s.abs().powf(two_h) - (t - s).abs().powf(two_h))
}
/// Covariance of fBM increments on arbitrary intervals.
///
/// $$\operatorname{Cov}\bigl(B_H(t_{i+1}) - B_H(t_i),\; B_H(t_{j+1}) - B_H(t_j)\bigr)$$
///
/// computed via the bilinearity relation on the fBM covariance function.
///
/// # Arguments
///
/// * `ti` - Start time of the first increment.
/// * `ti1` - End time of the first increment, in the same time unit as `ti`.
/// * `tj` - Start time of the second increment.
/// * `tj1` - End time of the second increment, in the same time unit as `tj`.
/// * `h` - Hurst exponent used by the fractional Brownian-motion covariance.
pub fn fbm_increment_covariance(ti: f64, ti1: f64, tj: f64, tj1: f64, h: f64) -> f64 {
    fbm_covariance(ti1, tj1, h) - fbm_covariance(ti1, tj, h) - fbm_covariance(ti, tj1, h)
        + fbm_covariance(ti, tj, h)
}
/// Covariance matrix of fBM increments on a time grid.
///
/// Given times t₀, t₁, …, tₙ the matrix is (n) × (n) with entry
/// (i, j) = Cov(B_H(t_{i+1}) − B_H(tᵢ), B_H(t_{j+1}) − B_H(tⱼ)).
///
/// Requires at least two time points. Returns an empty 0 × 0 matrix
/// when fewer than two points are supplied.
///
/// # Arguments
///
/// * `times` - Ordered grid of increment boundary times in the model's chosen
///   time unit.
/// * `h` - Hurst exponent used by the fractional Brownian-motion covariance.
pub fn fbm_increment_covariance_matrix(times: &[f64], h: f64) -> DMatrix<f64> {
    if times.len() < 2 {
        return DMatrix::zeros(0, 0);
    }
    let n = times.len() - 1;
    DMatrix::from_fn(n, n, |i, j| {
        fbm_increment_covariance(times[i], times[i + 1], times[j], times[j + 1], h)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 1e-10;

    // -- HurstExponent validation ------------------------------------------

    #[test]
    fn hurst_valid() {
        let h = HurstExponent::new(0.1).unwrap();
        assert!((h.value() - 0.1).abs() < TOL);
        assert!((h.alpha() - 0.6).abs() < TOL);
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

    #[test]
    fn hurst_serde_round_trip_preserves_valid_exponents() {
        for h in [0.1, 0.5, 0.9] {
            let exponent = HurstExponent::new(h).expect("valid exponent");
            let json = serde_json::to_string(&exponent).expect("serialize exponent");
            let restored: HurstExponent = serde_json::from_str(&json).expect("validated exponent");
            assert_eq!(restored, exponent);
        }
    }

    // -- fBM covariance ----------------------------------------------------

    #[test]
    fn fbm_cov_h_half_is_min() {
        // When H = 0.5, Cov(B(t), B(s)) = min(s, t) for s, t >= 0
        let h = 0.5;
        for &(t, s) in &[(1.0, 2.0), (3.0, 1.5), (0.5, 0.5)] {
            let cov = fbm_covariance(t, s, h);
            let expected = t.min(s);
            assert!(
                (cov - expected).abs() < TOL,
                "Cov({t},{s}) = {cov}, expected {expected}"
            );
        }
    }
    // -- Covariance matrix -------------------------------------------------
    #[test]
    fn increment_covariance_matrix_symmetric() {
        let times = vec![0.0, 0.25, 0.5, 0.75, 1.0];
        let h = 0.3;
        let cov = fbm_increment_covariance_matrix(&times, h);
        assert_eq!(cov.nrows(), 4);
        assert_eq!(cov.ncols(), 4);
        for i in 0..cov.nrows() {
            for j in 0..cov.ncols() {
                assert!(
                    (cov[(i, j)] - cov[(j, i)]).abs() < TOL,
                    "Asymmetry at ({i},{j})"
                );
            }
        }
    }

    #[test]
    fn increment_covariance_empty_for_single_point() {
        let cov = fbm_increment_covariance_matrix(&[1.0], 0.5);
        assert_eq!(cov.nrows(), 0);
    }

    // -- Kernel evaluation -------------------------------------------------
    // -- Mittag-Leffler ----------------------------------------------------
}
