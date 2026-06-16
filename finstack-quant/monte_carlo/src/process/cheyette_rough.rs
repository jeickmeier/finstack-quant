//! Cheyette + rough stochastic volatility model for interest rates.
//!
//! The Cheyette framework (1-factor Markovian HJM) tracks the short rate via a
//! two-dimensional Markov state (x, y) with the instantaneous forward rate
//! f(0, t) as a deterministic shift.  This module augments that framework with
//! rough stochastic volatility, where the rate diffusion coefficient σ(t) is
//! driven by a Volterra fractional Brownian motion as in the rBergomi paradigm.
//!
//! # Stochastic Differential Equations
//!
//! ```text
//! r(t) = x(t) + φ(t)
//! dx(t) = [y(t) − κ·x(t)] dt + σ(t)·dW(t)
//! dy(t) = [σ(t)² − 2κ·y(t)] dt
//!
//! σ(t)  = σ₀(t) · exp(½·η·W̃_H(t) − ¼·η²·t^{2H})
//! ```
//!
//! The vol-of-vol convention follows rBergomi **variance-lognormal**
//! semantics: the instantaneous variance `V(t) = σ(t)²
//! = σ₀(t)²·exp(η·W̃_H(t) − ½·η²·t^{2H})` is lognormal with full
//! compensation, so `E[σ²(t)] = σ₀²(t)` for every `t` and `η` is directly
//! comparable to the rBergomi vol-of-vol. (The previous σ-lognormal
//! convention `σ = σ₀·exp(η·W̃ − ½η²t^{2H})` compensated the *vol* level
//! instead, inflating `E[σ²]` by `exp(η²·t^{2H})` — ≈22× at η=1.5, t=5y,
//! H=0.1.)
//!
//! where:
//! - **x(t)**: Rate state variable (deviation from initial forward curve)
//! - **y(t)**: Accumulated variance state variable
//! - **φ(t) = f(0, t)**: Instantaneous forward rate from the initial curve
//! - **σ₀(t)**: Base (deterministic) volatility term structure
//! - **W̃_H(t)**: Volterra fBM with Hurst exponent H ∈ (0, 0.5)
//! - **η**: Vol-of-vol scaling
//! - **ρ**: Correlation between rate BM W and vol fBM W̃_H
//! - **κ**: Mean reversion speed for the short rate
//!
//! # Design Notes
//!
//! Because σ(t) depends on the entire fBM path history, the standard
//! `drift`/`diffusion` interface cannot express the non-Markovian volatility
//! dynamics.  Accordingly, `drift` and `diffusion` are implemented as formal
//! no-ops — the actual dynamics are handled entirely by the dedicated
//! [`CheyetteRoughEuler`](super::super::discretization::cheyette_rough::CheyetteRoughEuler)
//! discretization, which tracks the accumulated Volterra integral internally.
//!
//! # References
//!
//! - Cheyette, O. (1994). "Markov Representation of the Heath-Jarrow-Morton
//!   Model." Working paper.
//! - Andersen, L. & Piterbarg, V. (2010). *Interest Rate Modeling*, Volume II,
//!   Chapter 12 (Cheyette / quasi-Gaussian models).
//! - Bayer, C., Friz, P. & Gatheral, J. (2016). "Pricing under rough
//!   volatility." *Quantitative Finance*, 16(6), 887–904.
//! - Gatheral, J., Jaisson, T. & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949.
//!
//! # Examples
//!
//! ```ignore
//! use finstack_quant_monte_carlo::process::cheyette_rough::{
//!     CheyetteRoughVolProcess, CheyetteRoughVolParams,
//! };
//! use finstack_quant_monte_carlo::traits::StochasticProcess;
//! use finstack_quant_core::market_data::term_structures::ForwardVarianceCurve;
//! use finstack_quant_core::math::fractional::HurstExponent;
//!
//! let params = CheyetteRoughVolParams::new(
//!     0.03,                                       // κ = 3% mean reversion
//!     ForwardVarianceCurve::flat(0.005).unwrap(),  // σ₀ = 50bps base vol
//!     HurstExponent::new(0.1).unwrap(),            // H = 0.1 (rough)
//!     1.5,                                         // η = vol-of-vol
//!     -0.3,                                        // ρ = rate-vol correlation
//!     &[(0.0, 0.02), (5.0, 0.025), (30.0, 0.03)], // φ(t) = f(0, t) curve
//! )
//! .unwrap();
//!
//! let process = CheyetteRoughVolProcess::new(params);
//! assert_eq!(process.dim(), 2);
//! assert_eq!(process.num_factors(), 3);
//! ```

use super::super::paths::ProcessParams;
use super::super::traits::{state_keys, PathState, StochasticProcess};
use super::metadata::ProcessMetadata;
use finstack_quant_core::market_data::term_structures::ForwardVarianceCurve;
use finstack_quant_core::math::fractional::HurstExponent;

/// Cheyette + rough stochastic volatility model parameters.
///
/// Encapsulates all inputs required to define the Cheyette dynamics with a
/// rough vol driver.  The forward variance curve `sigma_base` and the initial
/// forward rate curve `phi` are marked `#[serde(skip)]` because they are
/// typically reconstructed from market data rather than round-tripped through
/// plain-text serialization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheyetteRoughVolParams {
    /// Mean reversion of the short rate (κ > 0).
    pub kappa: f64,
    /// Base volatility term structure σ₀(t) for the rate process.
    #[serde(skip, default)]
    pub sigma_base: ForwardVarianceCurve,
    /// Hurst exponent H ∈ (0, 0.5) for the rough vol driver.
    pub hurst: HurstExponent,
    /// Vol-of-vol scaling (η > 0).
    pub eta: f64,
    /// Correlation between rate and vol innovations ρ ∈ [-1, 1].
    pub rho: f64,
    /// Time knots for the initial forward rate curve φ(t) = f(0, t).
    #[serde(skip)]
    phi_times: Vec<f64>,
    /// Value knots for the initial forward rate curve φ(t) = f(0, t).
    #[serde(skip)]
    phi_values: Vec<f64>,
}

impl CheyetteRoughVolParams {
    /// Create new Cheyette + rough vol parameters with full validation.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean reversion speed (must be positive and finite)
    /// * `sigma_base` - Base volatility term structure σ₀(t)
    /// * `hurst` - Hurst exponent; warns if not rough (H ≥ 0.5)
    /// * `eta` - Vol-of-vol (must be positive and finite)
    /// * `rho` - Rate-vol correlation (must be in \[-1, 1\] and finite)
    /// * `phi_points` - Initial forward rate curve as (time, rate) pairs;
    ///   must have at least one point, times non-negative and increasing
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any parameter is out
    /// of range.
    pub fn new(
        kappa: f64,
        sigma_base: ForwardVarianceCurve,
        hurst: HurstExponent,
        eta: f64,
        rho: f64,
        phi_points: &[(f64, f64)],
    ) -> finstack_quant_core::Result<Self> {
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CheyetteRoughVol parameter kappa must be positive and finite, got {kappa}"
            )));
        }
        if eta <= 0.0 || !eta.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CheyetteRoughVol parameter eta must be positive and finite, got {eta}"
            )));
        }
        if !(-1.0..=1.0).contains(&rho) || !rho.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "CheyetteRoughVol parameter rho must be in [-1, 1], got {rho}"
            )));
        }
        if phi_points.is_empty() {
            return Err(finstack_quant_core::Error::Validation(
                "CheyetteRoughVol phi_points must have at least one point".to_string(),
            ));
        }

        let mut phi_times = Vec::with_capacity(phi_points.len());
        let mut phi_values = Vec::with_capacity(phi_points.len());

        for (i, &(t, v)) in phi_points.iter().enumerate() {
            if !t.is_finite() || t < 0.0 {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CheyetteRoughVol phi_points time must be non-negative and finite, \
                     got {t} at index {i}"
                )));
            }
            if !v.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CheyetteRoughVol phi_points value must be finite, got {v} at index {i}"
                )));
            }
            if i > 0 && t <= phi_times[i - 1] {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "CheyetteRoughVol phi_points times must be strictly increasing, \
                     got {t} after {} at index {i}",
                    phi_times[i - 1]
                )));
            }
            phi_times.push(t);
            phi_values.push(v);
        }

        if !hurst.is_rough() {
            tracing::warn!(
                h = hurst.value(),
                "CheyetteRoughVol Hurst exponent H = {:.4} is not rough (H >= 0.5). \
                 The model is designed for rough volatility; results may not \
                 match empirical behavior.",
                hurst.value()
            );
        }

        Ok(Self {
            kappa,
            sigma_base,
            hurst,
            eta,
            rho,
            phi_times,
            phi_values,
        })
    }

    /// Interpolate the initial forward rate φ(t) = f(0, t) at time `t`.
    ///
    /// Uses linear interpolation between knots and flat extrapolation at the
    /// boundaries.
    pub fn phi(&self, t: f64) -> f64 {
        let n = self.phi_times.len();
        debug_assert!(n > 0);

        if n == 1 || t <= self.phi_times[0] {
            return self.phi_values[0];
        }
        if t >= self.phi_times[n - 1] {
            return self.phi_values[n - 1];
        }

        // Binary search for the containing interval
        let i = match self
            .phi_times
            .binary_search_by(|x| x.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(idx) => return self.phi_values[idx],
            Err(idx) => idx - 1,
        };

        let t0 = self.phi_times[i];
        let t1 = self.phi_times[i + 1];
        let v0 = self.phi_values[i];
        let v1 = self.phi_values[i + 1];
        let w = (t - t0) / (t1 - t0);
        v0 + w * (v1 - v0)
    }

    /// Evaluate the base volatility σ₀(t) at time `t`.
    pub fn sigma_base_value(&self, t: f64) -> f64 {
        self.sigma_base.value(t)
    }
}

/// Cheyette + rough stochastic volatility process.
///
/// State: \[x, y\] where x is the rate deviation from the initial forward
/// curve and y is the accumulated variance state.
///
/// Factors: 3 — one independent standard normal for the uncorrelated rate
/// component, one Riemann-Liouville Volterra increment `ΔỸ`, and one
/// unit-variance driving Brownian normal `Z̃` (the rate–vol correlation
/// target), both supplied by the engine's fractional noise integration.
///
/// The short rate is reconstructed as r(t) = x(t) + φ(t) and written to
/// [`state_keys::SHORT_RATE`] in [`populate_path_state`](StochasticProcess::populate_path_state).
#[derive(Debug, Clone)]
pub struct CheyetteRoughVolProcess {
    /// Model parameters.
    params: CheyetteRoughVolParams,
}

impl CheyetteRoughVolProcess {
    /// Create a new Cheyette + rough vol process from validated parameters.
    pub fn new(params: CheyetteRoughVolParams) -> Self {
        Self { params }
    }

    /// Create with inline parameter construction.
    ///
    /// Delegates to [`CheyetteRoughVolParams::new`] for validation.
    pub fn with_params(
        kappa: f64,
        sigma_base: ForwardVarianceCurve,
        hurst: HurstExponent,
        eta: f64,
        rho: f64,
        phi_points: &[(f64, f64)],
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self::new(CheyetteRoughVolParams::new(
            kappa, sigma_base, hurst, eta, rho, phi_points,
        )?))
    }

    /// Get a reference to the process parameters.
    pub fn params(&self) -> &CheyetteRoughVolParams {
        &self.params
    }
}

impl StochasticProcess for CheyetteRoughVolProcess {
    fn dim(&self) -> usize {
        2 // x (rate state) and y (variance state)
    }

    fn num_factors(&self) -> usize {
        // z[0] = independent normal (uncorrelated rate),
        // z[1] = RL Volterra increment ΔỸ,
        // z[2] = unit-variance driving Brownian normal Z̃
        3
    }

    fn requires_injected_noise(&self) -> bool {
        // z[1]/z[2] must be Volterra increments and their driving normals,
        // generated jointly on the full grid; i.i.d. engine noise is invalid.
        true
    }

    /// Formal no-op: actual drift is applied by the Cheyette rough vol discretization.
    fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.0;
        out[1] = 0.0;
    }

    /// Formal no-op: actual diffusion is applied by the Cheyette rough vol discretization.
    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.0;
        out[1] = 0.0;
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        let t = state.time;
        let phi_t = self.params.phi(t);
        state.set(state_keys::SHORT_RATE, x[0] + phi_t);
    }
}

impl ProcessMetadata for CheyetteRoughVolProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("CheyetteRoughVol");
        params.add_param("kappa", self.params.kappa);
        params.add_param("H", self.params.hurst.value());
        params.add_param("eta", self.params.eta);
        params.add_param("rho", self.params.rho);

        params.with_factors(vec!["rate".to_string(), "variance".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sigma_base() -> ForwardVarianceCurve {
        ForwardVarianceCurve::flat(0.005).expect("valid flat curve")
    }

    fn make_hurst(h: f64) -> HurstExponent {
        HurstExponent::new(h).expect("valid hurst")
    }

    fn make_phi_points() -> Vec<(f64, f64)> {
        vec![(0.0, 0.02), (5.0, 0.025), (30.0, 0.03)]
    }

    // -- Parameter validation -----------------------------------------------

    #[test]
    fn test_valid_params() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        );
        assert!(params.is_ok());
    }

    #[test]
    fn test_kappa_must_be_positive() {
        let res = CheyetteRoughVolParams::new(
            0.0,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        );
        assert!(res.is_err());

        let res = CheyetteRoughVolParams::new(
            -1.0,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_kappa_must_be_finite() {
        let res = CheyetteRoughVolParams::new(
            f64::INFINITY,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_eta_must_be_positive() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            0.0,
            -0.3,
            &make_phi_points(),
        );
        assert!(res.is_err());

        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            -1.0,
            -0.3,
            &make_phi_points(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_eta_must_be_finite() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            f64::INFINITY,
            -0.3,
            &make_phi_points(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_must_be_in_range() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -1.1,
            &make_phi_points(),
        );
        assert!(res.is_err());

        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            1.1,
            &make_phi_points(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_must_be_finite() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            f64::NAN,
            &make_phi_points(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_boundary_values() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -1.0,
            &make_phi_points(),
        );
        assert!(res.is_ok());

        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            1.0,
            &make_phi_points(),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_phi_points_must_be_nonempty() {
        let res =
            CheyetteRoughVolParams::new(0.03, make_sigma_base(), make_hurst(0.1), 1.5, -0.3, &[]);
        assert!(res.is_err());
    }

    #[test]
    fn test_phi_points_times_must_be_increasing() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(0.0, 0.02), (0.0, 0.03)],
        );
        assert!(res.is_err());

        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(1.0, 0.02), (0.5, 0.03)],
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_phi_points_times_must_be_nonnegative() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(-0.1, 0.02)],
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_phi_points_values_must_be_finite() {
        let res = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(0.0, f64::NAN)],
        );
        assert!(res.is_err());
    }

    // -- Phi interpolation -------------------------------------------------

    #[test]
    fn test_phi_flat_extrapolation_left() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(1.0, 0.02), (5.0, 0.03)],
        )
        .expect("valid");

        assert!((params.phi(0.0) - 0.02).abs() < 1e-15);
        assert!((params.phi(0.5) - 0.02).abs() < 1e-15);
    }

    #[test]
    fn test_phi_flat_extrapolation_right() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(1.0, 0.02), (5.0, 0.03)],
        )
        .expect("valid");

        assert!((params.phi(10.0) - 0.03).abs() < 1e-15);
    }

    #[test]
    fn test_phi_linear_interpolation() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(0.0, 0.02), (10.0, 0.03)],
        )
        .expect("valid");

        // Midpoint: 0.02 + 0.5 * (0.03 - 0.02) = 0.025
        assert!((params.phi(5.0) - 0.025).abs() < 1e-15);
    }

    #[test]
    fn test_phi_single_point() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(0.0, 0.02)],
        )
        .expect("valid");

        assert!((params.phi(0.0) - 0.02).abs() < 1e-15);
        assert!((params.phi(10.0) - 0.02).abs() < 1e-15);
    }

    // -- Process dimensions -------------------------------------------------

    #[test]
    fn test_process_dim() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        )
        .expect("valid");
        let process = CheyetteRoughVolProcess::new(params);

        assert_eq!(process.dim(), 2);
        assert_eq!(process.num_factors(), 3);
    }

    #[test]
    fn test_drift_diffusion_are_noop() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        )
        .expect("valid");
        let process = CheyetteRoughVolProcess::new(params);

        let x = [0.0, 0.0];
        let mut drift = [f64::NAN, f64::NAN];
        let mut diff = [f64::NAN, f64::NAN];

        process.drift(0.5, &x, &mut drift);
        process.diffusion(0.5, &x, &mut diff);

        assert_eq!(drift[0], 0.0);
        assert_eq!(drift[1], 0.0);
        assert_eq!(diff[0], 0.0);
        assert_eq!(diff[1], 0.0);
    }

    #[test]
    fn test_populate_path_state_sets_short_rate() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(0.0, 0.02), (10.0, 0.03)],
        )
        .expect("valid");
        let process = CheyetteRoughVolProcess::new(params);

        // At t=5.0, phi(5.0) = 0.025, x[0] = 0.005 => r = 0.03
        let x = [0.005, 0.001];
        let mut state = PathState::new(5, 5.0);
        process.populate_path_state(&x, &mut state);

        let short_rate = state.get(state_keys::SHORT_RATE).expect("short_rate set");
        assert!(
            (short_rate - 0.03).abs() < 1e-14,
            "Expected short_rate = 0.03, got {short_rate}"
        );
    }

    #[test]
    fn test_populate_path_state_at_t_zero() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &[(0.0, 0.02), (10.0, 0.03)],
        )
        .expect("valid");
        let process = CheyetteRoughVolProcess::new(params);

        // At t=0.0, phi(0.0) = 0.02, x[0] = 0.0 => r = 0.02
        let x = [0.0, 0.0];
        let mut state = PathState::new(0, 0.0);
        process.populate_path_state(&x, &mut state);

        let short_rate = state.get(state_keys::SHORT_RATE).expect("short_rate set");
        assert!(
            (short_rate - 0.02).abs() < 1e-14,
            "Expected short_rate = 0.02, got {short_rate}"
        );
    }

    #[test]
    fn test_metadata() {
        let params = CheyetteRoughVolParams::new(
            0.03,
            make_sigma_base(),
            make_hurst(0.1),
            1.5,
            -0.3,
            &make_phi_points(),
        )
        .expect("valid");
        let process = CheyetteRoughVolProcess::new(params);
        let meta = process.metadata();

        assert_eq!(meta.process_type, "CheyetteRoughVol");
    }
}
