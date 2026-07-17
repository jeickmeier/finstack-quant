//! Rough Bergomi (rBergomi) stochastic volatility model.
//!
//! The rBergomi model (Bayer, Friz, Gatheral 2016) extends classical stochastic
//! volatility by driving the variance process with a Riemann-Liouville
//! Volterra process of Hurst exponent H < 0.5, reproducing the power-law
//! explosion of the at-the-money implied volatility skew observed in equity
//! markets.
//!
//! # Stochastic Differential Equations
//!
//! Under the risk-neutral measure ℚ:
//!
//! ```text
//! dS_t = (r - q) S_t dt + √V_t S_t dW(t)
//! V_t  = ξ₀(t) exp(η Ỹ_t - ½ η² t^{2H})
//!
//! Ỹ_t = √(2H) ∫₀ᵗ (t - s)^{H - ½} dW̃(s)   (Riemann-Liouville Volterra process)
//! dW · dW̃ = ρ dt
//! ```
//!
//! where:
//! - **S_t**: Spot price at time t
//! - **V_t**: Instantaneous variance (stochastic, driven by `Ỹ`)
//! - **ξ₀(t)**: Forward variance curve (market-implied)
//! - **η**: Vol-of-vol scaling parameter
//! - **H**: Hurst exponent, typically 0.07–0.12 for equity indices
//! - **ρ**: Spot-vol correlation (typically negative for equity)
//! - **Ỹ_t**: Riemann-Liouville Volterra process, `Var(Ỹ_t) = t^{2H}`
//!
//! Note that `Ỹ` is the *Riemann-Liouville* (truncated, non-stationary-increment)
//! Volterra process — **not** true fractional Brownian motion. The two share
//! the marginal variance `t^{2H}` but have different autocovariance; the
//! Bayer-Friz-Gatheral rBergomi model is defined with `Ỹ`. See
//! [`RiemannLiouvilleVolterra`](super::super::rng::volterra::RiemannLiouvilleVolterra).
//!
//! # Design Notes
//!
//! The variance process V_t is a *functional* of the entire Volterra path, not
//! a diffusive state variable. The standard `drift`/`diffusion` interface
//! cannot express this non-Markovian dependence on path history. Accordingly,
//! `drift` and `diffusion` are implemented as formal no-ops — the actual
//! dynamics are handled entirely by the dedicated
//! [`RoughBergomiEuler`](super::super::discretization::rough_bergomi::RoughBergomiEuler)
//! discretization, which tracks the accumulated Volterra process internally.
//!
//! This is analogous to how the Heston process provides `drift`/`diffusion` for
//! generic Euler schemes but is typically paired with the QE discretization that
//! bypasses them.
//!
//! # References
//!
//! - Bayer, C., Friz, P., & Gatheral, J. (2016). "Pricing under rough
//!   volatility." *Quantitative Finance*, 16(6), 887–904.
//! - Gatheral, J., Jaisson, T., & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949.
//! - McCrickerd, R. & Pakkanen, M. S. (2018). "Turbocharging Monte Carlo
//!   pricing for the rough Bergomi model." *Quantitative Finance*, 18(11),
//!   1877–1886.
//!
//! # Examples
//!
//! ```ignore
//! use finstack_quant_monte_carlo::process::rough_bergomi::{
//!     RoughBergomiProcess, RoughBergomiParams,
//! };
//! use finstack_quant_monte_carlo::traits::StochasticProcess;
//! use finstack_quant_core::market_data::term_structures::ForwardVarianceCurve;
//! use finstack_quant_core::math::fractional::HurstExponent;
//!
//! let params = RoughBergomiParams::new(
//!     0.05,                                      // r = 5%
//!     0.02,                                      // q = 2%
//!     HurstExponent::new(0.1).unwrap(),           // H = 0.1 (rough)
//!     1.9,                                        // η = vol-of-vol
//!     -0.9,                                       // ρ = spot-vol correlation
//!     ForwardVarianceCurve::flat(0.04).unwrap(),  // ξ₀ = 4% (20% vol)
//! )
//! .unwrap();
//!
//! let process = RoughBergomiProcess::new(params);
//! assert_eq!(process.dim(), 1);
//! assert_eq!(process.num_factors(), 3);
//! ```

use super::super::paths::ProcessParams;
#[cfg(test)]
use super::super::traits::state_keys;
use super::super::traits::{PathState, StateKey, StochasticProcess};
use super::metadata::ProcessMetadata;
use finstack_quant_core::market_data::term_structures::ForwardVarianceCurve;
use finstack_quant_core::math::fractional::HurstExponent;

/// rBergomi model parameters.
///
/// Encapsulates all inputs required to define the rough Bergomi dynamics.
/// The forward variance curve `xi` is marked `#[serde(skip)]` because it
/// is typically reconstructed from market data rather than round-tripped
/// through plain-text serialization. On deserialization it defaults to a
/// flat curve at 4% (20% vol).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoughBergomiParams {
    /// Risk-free rate (annual, continuously compounded).
    pub r: f64,
    /// Dividend yield (annual, continuously compounded).
    pub q: f64,
    /// Hurst exponent H ∈ (0, 0.5), typically 0.07–0.12 for equity indices.
    pub hurst: HurstExponent,
    /// Vol-of-vol scaling (η > 0).
    pub eta: f64,
    /// Spot-vol correlation ρ ∈ [-1, 1].
    pub rho: f64,
    /// Initial forward variance curve ξ₀(t).
    #[serde(skip, default)]
    pub xi: ForwardVarianceCurve,
}

impl RoughBergomiParams {
    /// Create new rBergomi parameters with full validation.
    ///
    /// # Arguments
    ///
    /// * `r` - Risk-free rate (must be finite)
    /// * `q` - Dividend yield (must be finite)
    /// * `hurst` - Hurst exponent; warns if not rough (H ≥ 0.5)
    /// * `eta` - Vol-of-vol (must be positive and finite)
    /// * `rho` - Spot-vol correlation (must be in [-1, 1] and finite)
    /// * `xi` - Forward variance curve (passed through as-is)
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] when any parameter is out
    /// of range.
    pub fn new(
        r: f64,
        q: f64,
        hurst: HurstExponent,
        eta: f64,
        rho: f64,
        xi: ForwardVarianceCurve,
    ) -> finstack_quant_core::Result<Self> {
        if !r.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "rBergomi parameter r must be finite, got {r}"
            )));
        }
        if !q.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "rBergomi parameter q must be finite, got {q}"
            )));
        }
        if eta <= 0.0 || !eta.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "rBergomi parameter eta must be positive and finite, got {eta}"
            )));
        }
        if !(-1.0..=1.0).contains(&rho) || !rho.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "rBergomi parameter rho must be in [-1, 1], got {rho}"
            )));
        }

        if !hurst.is_rough() {
            tracing::warn!(
                h = hurst.value(),
                "rBergomi Hurst exponent H = {:.4} is not rough (H >= 0.5). \
                 The model is designed for rough volatility; results may not \
                 match empirical skew behavior.",
                hurst.value()
            );
        }

        Ok(Self {
            r,
            q,
            hurst,
            eta,
            rho,
            xi,
        })
    }
}

/// Rough Bergomi stochastic volatility process.
///
/// State: \[S\] (spot only; variance is a functional of the Volterra path and
/// is reconstructed by the discretization).
///
/// Factors: 3 — one independent standard normal for the uncorrelated spot
/// component, one Riemann-Liouville Volterra increment `ΔỸ` (accumulated into
/// the variance), and one unit-variance driving Brownian normal (used for the
/// exact spot–vol correlation). The latter two are supplied by the engine's
/// fractional noise integration.
#[derive(Debug, Clone)]
pub struct RoughBergomiProcess {
    params: RoughBergomiParams,
}

impl RoughBergomiProcess {
    /// Create a new rBergomi process from validated parameters.
    pub fn new(params: RoughBergomiParams) -> Self {
        Self { params }
    }

    /// Create an rBergomi process from annualized market and rough-volatility inputs.
    ///
    /// `r` and `q` are continuously compounded annual rates, `xi` supplies the
    /// initial forward variance (not volatility), and `eta` scales the
    /// log-variance driver. `hurst` and `rho` control the Volterra roughness and
    /// spot-vol correlation. Simulate this process only with
    /// `RoughBergomiEuler`, which supplies the required fractional noise.
    ///
    /// # Errors
    ///
    /// Propagates [`RoughBergomiParams::new`] validation for non-finite rates,
    /// non-positive or non-finite `eta`, and non-finite correlations outside
    /// `[-1, 1]`. A non-rough Hurst exponent is accepted but logged as a model
    /// suitability warning.
    pub fn with_params(
        r: f64,
        q: f64,
        hurst: HurstExponent,
        eta: f64,
        rho: f64,
        xi: ForwardVarianceCurve,
    ) -> finstack_quant_core::Result<Self> {
        Ok(Self::new(RoughBergomiParams::new(
            r, q, hurst, eta, rho, xi,
        )?))
    }

    /// Get a reference to the process parameters.
    pub fn params(&self) -> &RoughBergomiParams {
        &self.params
    }
}

impl StochasticProcess for RoughBergomiProcess {
    fn dim(&self) -> usize {
        1 // Spot only; variance is non-Markovian and handled by the discretization
    }

    fn num_factors(&self) -> usize {
        // z[0] = independent normal (uncorrelated spot component),
        // z[1] = RL Volterra increment ΔỸ (accumulated into the variance),
        // z[2] = unit-variance driving Brownian normal (spot–vol correlation).
        3
    }

    fn requires_injected_noise(&self) -> bool {
        // z[1]/z[2] must be Volterra increments and their driving normals,
        // generated jointly on the full grid; i.i.d. engine noise is invalid.
        true
    }

    /// Formal no-op: actual drift is applied by the rBergomi discretization.
    fn drift(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.0;
    }

    /// Formal no-op: actual diffusion is applied by the rBergomi discretization.
    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = 0.0;
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        state.set_key(StateKey::Spot, x[0]);
    }
}

impl ProcessMetadata for RoughBergomiProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("rBergomi");
        params.add_param("r", self.params.r);
        params.add_param("q", self.params.q);
        params.add_param("H", self.params.hurst.value());
        params.add_param("eta", self.params.eta);
        params.add_param("rho", self.params.rho);

        params.with_factors(vec!["spot".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_xi() -> ForwardVarianceCurve {
        ForwardVarianceCurve::flat(0.04).expect("valid flat curve")
    }

    fn make_hurst(h: f64) -> HurstExponent {
        HurstExponent::new(h).expect("valid hurst")
    }

    // -- Parameter validation -----------------------------------------------

    #[test]
    fn test_valid_params() {
        let params = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -0.9, make_xi());
        assert!(params.is_ok());
    }

    #[test]
    fn test_r_must_be_finite() {
        let res =
            RoughBergomiParams::new(f64::INFINITY, 0.02, make_hurst(0.1), 1.9, -0.9, make_xi());
        assert!(res.is_err());
    }

    #[test]
    fn test_q_must_be_finite() {
        let res = RoughBergomiParams::new(0.05, f64::NAN, make_hurst(0.1), 1.9, -0.9, make_xi());
        assert!(res.is_err());
    }

    #[test]
    fn test_eta_must_be_positive() {
        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 0.0, -0.9, make_xi());
        assert!(res.is_err());

        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), -1.0, -0.9, make_xi());
        assert!(res.is_err());
    }

    #[test]
    fn test_eta_must_be_finite() {
        let res =
            RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), f64::INFINITY, -0.9, make_xi());
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_must_be_in_range() {
        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -1.1, make_xi());
        assert!(res.is_err());

        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, 1.1, make_xi());
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_must_be_finite() {
        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, f64::NAN, make_xi());
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_boundary_values() {
        // Exact -1 and 1 are valid
        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -1.0, make_xi());
        assert!(res.is_ok());

        let res = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, 1.0, make_xi());
        assert!(res.is_ok());
    }

    // -- Process dimensions -------------------------------------------------

    #[test]
    fn test_process_dim() {
        let params = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -0.9, make_xi())
            .expect("valid");
        let process = RoughBergomiProcess::new(params);

        assert_eq!(process.dim(), 1);
        assert_eq!(process.num_factors(), 3);
    }

    #[test]
    fn test_drift_diffusion_are_noop() {
        let params = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -0.9, make_xi())
            .expect("valid");
        let process = RoughBergomiProcess::new(params);

        let x = [100.0];
        let mut drift = [f64::NAN];
        let mut diff = [f64::NAN];

        process.drift(0.5, &x, &mut drift);
        process.diffusion(0.5, &x, &mut diff);

        assert_eq!(drift[0], 0.0);
        assert_eq!(diff[0], 0.0);
    }

    #[test]
    fn test_populate_path_state() {
        let params = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -0.9, make_xi())
            .expect("valid");
        let process = RoughBergomiProcess::new(params);

        let x = [105.0];
        let mut state = PathState::new(0, 0.0);
        process.populate_path_state(&x, &mut state);

        assert_eq!(state.get(state_keys::SPOT), Some(105.0));
    }

    #[test]
    fn test_metadata() {
        let params = RoughBergomiParams::new(0.05, 0.02, make_hurst(0.1), 1.9, -0.9, make_xi())
            .expect("valid");
        let process = RoughBergomiProcess::new(params);
        let meta = process.metadata();

        assert_eq!(meta.process_type, "rBergomi");
        assert_eq!(meta.parameters.get("H"), Some(&0.1));
        assert_eq!(meta.parameters.get("eta"), Some(&1.9));
        assert_eq!(meta.factor_names, vec!["spot"]);
    }
}
