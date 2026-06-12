//! Rough Heston stochastic volatility model.
//!
//! The rough Heston model (El Euch & Rosenbaum 2019) generalizes the classical
//! Heston model by replacing the Markovian CIR variance process with a Volterra
//! stochastic integral driven by a singular power-law kernel.  This captures the
//! empirically observed power-law behavior of the at-the-money implied volatility
//! skew at short maturities.
//!
//! # Stochastic Differential Equations
//!
//! Under the risk-neutral measure ℚ:
//!
//! ```text
//! dS_t = √V_t · S_t · dW(t)
//! V_t  = V₀ + (1/Γ(α)) ∫₀ᵗ (t − s)^{α−1} [κ(θ − V_s) ds + σᵥ √V_s dW̃(s)]
//!
//! α = H + 0.5,   H ∈ (0, 0.5)
//! dW · dW̃ = ρ dt
//! ```
//!
//! Unlike the rough Bergomi model the roughness enters through a Volterra
//! integral with a singular kernel, **not** through fractional Brownian motion.
//! The driving noise W̃ is a standard Brownian motion; the fractional memory
//! structure arises from the `(t − s)^{α−1}` kernel.  Consequently the rough
//! Heston engine does **not** require the fBM generator infrastructure — each
//! step consumes two standard normal variates.
//!
//! # Design Notes
//!
//! Because the variance at time t depends on the entire history of shocks
//! `{V_s, dW̃_s : s ≤ t}`, the standard `drift`/`diffusion` interface cannot
//! express these non-Markovian dynamics.  The formal implementations provided
//! here mirror the classical Heston coefficients for metadata and display
//! purposes; the actual time-stepping is handled by the dedicated
//! [`RoughHestonHybrid`](super::super::discretization::rough_heston::RoughHestonHybrid)
//! discretization, which evaluates the Volterra integral at every step.
//!
//! # References
//!
//! - El Euch, O. & Rosenbaum, M. (2019). "The characteristic function of rough
//!   Heston models." *Mathematical Finance*, 29(1), 3–38.
//! - El Euch, O. & Rosenbaum, M. (2018). "Perfect hedging in rough Heston
//!   models." *Annals of Applied Probability*, 28(6), 3813–3856.
//! - Gatheral, J., Jaisson, T. & Rosenbaum, M. (2018). "Volatility is rough."
//!   *Quantitative Finance*, 18(6), 933–949.
//!
//! # Examples
//!
//! ```ignore
//! use finstack_monte_carlo::process::rough_heston::{
//!     RoughHestonProcess, RoughHestonParams,
//! };
//! use finstack_monte_carlo::traits::StochasticProcess;
//! use finstack_core::math::fractional::HurstExponent;
//!
//! let params = RoughHestonParams::new(
//!     0.05,                                   // r = 5%
//!     0.02,                                   // q = 2%
//!     HurstExponent::new(0.1).unwrap(),       // H = 0.1 (rough)
//!     2.0,                                    // κ = mean reversion
//!     0.04,                                   // θ = long-run variance
//!     0.3,                                    // σᵥ = vol-of-vol
//!     -0.7,                                   // ρ = spot-vol correlation
//!     0.04,                                   // v₀ = initial variance
//! )
//! .unwrap();
//!
//! let process = RoughHestonProcess::new(params);
//! assert_eq!(process.dim(), 2);
//! assert_eq!(process.num_factors(), 2);
//! ```

use super::super::paths::ProcessParams;
use super::super::traits::{state_keys, PathState, StochasticProcess};
use super::metadata::ProcessMetadata;
use finstack_core::math::fractional::HurstExponent;

/// Rough Heston model parameters.
///
/// All parameters must be finite.  Positivity constraints apply to `kappa`,
/// `theta`, `sigma_v`, and `v0`.  The Hurst exponent is validated by
/// [`HurstExponent`]; a warning is logged when `H ≥ 0.5` (non-rough regime).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoughHestonParams {
    /// Risk-free rate (annual, continuously compounded).
    pub r: f64,
    /// Dividend yield (annual, continuously compounded).
    pub q: f64,
    /// Hurst exponent H ∈ (0, 0.5) — controls the roughness of the variance.
    pub hurst: HurstExponent,
    /// Mean reversion speed (κ > 0).
    pub kappa: f64,
    /// Long-run variance level (θ > 0).
    pub theta: f64,
    /// Volatility of variance — vol-of-vol (σᵥ > 0).
    pub sigma_v: f64,
    /// Spot–variance correlation ρ ∈ \[−1, 1\].
    pub rho: f64,
    /// Initial variance (v₀ > 0).
    pub v0: f64,
}

impl RoughHestonParams {
    /// Create new rough Heston parameters with full validation.
    ///
    /// # Arguments
    ///
    /// * `r` - Risk-free rate (must be finite)
    /// * `q` - Dividend yield (must be finite)
    /// * `hurst` - Hurst exponent; warns if not rough (H ≥ 0.5)
    /// * `kappa` - Mean reversion speed (must be positive and finite)
    /// * `theta` - Long-run variance (must be positive and finite)
    /// * `sigma_v` - Vol-of-vol (must be positive and finite)
    /// * `rho` - Spot–variance correlation (must be in \[−1, 1\] and finite)
    /// * `v0` - Initial variance (must be positive and finite)
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] when any parameter is out
    /// of range.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        r: f64,
        q: f64,
        hurst: HurstExponent,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> finstack_core::Result<Self> {
        if !r.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter r must be finite, got {r}"
            )));
        }
        if !q.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter q must be finite, got {q}"
            )));
        }
        if kappa <= 0.0 || !kappa.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter kappa must be positive, got {kappa}"
            )));
        }
        if theta <= 0.0 || !theta.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter theta must be positive, got {theta}"
            )));
        }
        if sigma_v <= 0.0 || !sigma_v.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter sigma_v must be positive, got {sigma_v}"
            )));
        }
        if !(-1.0..=1.0).contains(&rho) || !rho.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter rho must be in [-1, 1], got {rho}"
            )));
        }
        if v0 <= 0.0 || !v0.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "Rough Heston parameter v0 must be positive, got {v0}"
            )));
        }

        if !hurst.is_rough() {
            tracing::warn!(
                h = hurst.value(),
                "Rough Heston Hurst exponent H = {:.4} is not rough (H >= 0.5). \
                 The model is designed for rough volatility; results may not \
                 match empirical skew behavior.",
                hurst.value()
            );
        }

        Ok(Self {
            r,
            q,
            hurst,
            kappa,
            theta,
            sigma_v,
            rho,
            v0,
        })
    }
}

/// Rough Heston stochastic volatility process.
///
/// State: \[S, V\] (spot price and instantaneous variance).
///
/// Factors: 2 — both are standard normal variates (the fractional memory
/// structure is encoded in the Volterra kernel, **not** via fBM increments).
#[derive(Debug, Clone)]
pub struct RoughHestonProcess {
    params: RoughHestonParams,
}

impl RoughHestonProcess {
    /// Create a new rough Heston process from validated parameters.
    pub fn new(params: RoughHestonParams) -> Self {
        Self { params }
    }

    /// Create with inline parameter construction.
    ///
    /// Delegates to [`RoughHestonParams::new`] for validation.
    #[allow(clippy::too_many_arguments)]
    pub fn with_params(
        r: f64,
        q: f64,
        hurst: HurstExponent,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        v0: f64,
    ) -> finstack_core::Result<Self> {
        Ok(Self::new(RoughHestonParams::new(
            r, q, hurst, kappa, theta, sigma_v, rho, v0,
        )?))
    }

    /// Get a reference to the process parameters.
    pub fn params(&self) -> &RoughHestonParams {
        &self.params
    }
}

impl StochasticProcess for RoughHestonProcess {
    fn dim(&self) -> usize {
        2 // Spot + variance
    }

    fn num_factors(&self) -> usize {
        2 // Two standard Brownian motions (no fBM needed)
    }

    /// Formal drift — mirrors classical Heston coefficients for metadata.
    ///
    /// The actual variance dynamics are handled by the Volterra discretization.
    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        let s = x[0];
        let v = x[1].max(0.0);

        // dS/dt = (r − q) S
        out[0] = (self.params.r - self.params.q) * s;
        // dv/dt = κ(θ − v) — formal; not used by the Volterra discretization
        out[1] = self.params.kappa * (self.params.theta - v);
    }

    /// Formal diffusion — mirrors classical Heston coefficients for metadata.
    ///
    /// The actual variance dynamics are handled by the Volterra discretization.
    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        let s = x[0];
        let v = x[1].max(0.0);

        // σ_S = √v · S
        out[0] = v.sqrt() * s;
        // σ_v = σᵥ √v — formal
        out[1] = self.params.sigma_v * v.sqrt();
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        state.set(state_keys::SPOT, x[0]);
        state.set(state_keys::VARIANCE, x[1]);
    }
}

impl ProcessMetadata for RoughHestonProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("RoughHeston");
        params.add_param("r", self.params.r);
        params.add_param("q", self.params.q);
        params.add_param("H", self.params.hurst.value());
        params.add_param("kappa", self.params.kappa);
        params.add_param("theta", self.params.theta);
        params.add_param("sigma_v", self.params.sigma_v);
        params.add_param("rho", self.params.rho);
        params.add_param("v0", self.params.v0);

        let correlation = vec![1.0, self.params.rho, self.params.rho, 1.0];

        params
            .with_correlation(correlation)
            .with_factors(vec!["spot".to_string(), "variance".to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_hurst(h: f64) -> HurstExponent {
        HurstExponent::new(h).expect("valid hurst")
    }

    // -- Parameter validation -----------------------------------------------

    #[test]
    fn test_valid_params() {
        let params =
            RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.3, -0.7, 0.04);
        assert!(params.is_ok());
    }

    #[test]
    fn test_r_must_be_finite() {
        let res = RoughHestonParams::new(
            f64::INFINITY,
            0.02,
            make_hurst(0.1),
            2.0,
            0.04,
            0.3,
            -0.7,
            0.04,
        );
        assert!(res.is_err());
    }

    #[test]
    fn test_q_must_be_finite() {
        let res =
            RoughHestonParams::new(0.05, f64::NAN, make_hurst(0.1), 2.0, 0.04, 0.3, -0.7, 0.04);
        assert!(res.is_err());
    }

    #[test]
    fn test_kappa_must_be_positive() {
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 0.0, 0.04, 0.3, -0.7, 0.04);
        assert!(res.is_err());
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), -1.0, 0.04, 0.3, -0.7, 0.04);
        assert!(res.is_err());
    }

    #[test]
    fn test_theta_must_be_positive() {
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.0, 0.3, -0.7, 0.04);
        assert!(res.is_err());
    }

    #[test]
    fn test_sigma_v_must_be_positive() {
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.0, -0.7, 0.04);
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_must_be_in_range() {
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.3, 1.5, 0.04);
        assert!(res.is_err());
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.3, -1.5, 0.04);
        assert!(res.is_err());
    }

    #[test]
    fn test_v0_must_be_positive() {
        let res = RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.3, -0.7, 0.0);
        assert!(res.is_err());
    }

    #[test]
    fn test_rho_boundary_values() {
        // rho = -1 and rho = 1 are valid
        assert!(
            RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.3, -1.0, 0.04).is_ok()
        );
        assert!(
            RoughHestonParams::new(0.05, 0.02, make_hurst(0.1), 2.0, 0.04, 0.3, 1.0, 0.04).is_ok()
        );
    }

    // -- Process dimensions -------------------------------------------------

    #[test]
    fn test_dim_and_factors() {
        let process = RoughHestonProcess::with_params(
            0.05,
            0.02,
            make_hurst(0.1),
            2.0,
            0.04,
            0.3,
            -0.7,
            0.04,
        )
        .expect("valid");

        assert_eq!(process.dim(), 2);
        assert_eq!(process.num_factors(), 2);
    }

    // -- Drift / diffusion --------------------------------------------------

    #[test]
    fn test_formal_drift_diffusion() {
        let process = RoughHestonProcess::with_params(
            0.05,
            0.02,
            make_hurst(0.1),
            2.0,
            0.04,
            0.3,
            -0.7,
            0.04,
        )
        .expect("valid");

        let x = vec![100.0_f64, 0.04_f64];
        let mut drift = vec![0.0_f64; 2];
        let mut diffusion = vec![0.0_f64; 2];

        process.drift(0.0, &x, &mut drift);
        process.diffusion(0.0, &x, &mut diffusion);

        // S drift: (r − q) S = 0.03 × 100 = 3.0
        assert!((drift[0] - 3.0).abs() < 1e-10);
        // v drift: κ(θ − v) = 2.0 × (0.04 − 0.04) = 0
        assert!((drift[1]).abs() < 1e-10);

        // S diffusion: √v · S = 0.2 × 100 = 20
        assert!((diffusion[0] - 20.0).abs() < 1e-10);
        // v diffusion: σᵥ √v = 0.3 × 0.2 = 0.06
        assert!((diffusion[1] - 0.06).abs() < 1e-10);
    }

    // -- Metadata -----------------------------------------------------------

    #[test]
    fn test_metadata_name() {
        let process = RoughHestonProcess::with_params(
            0.05,
            0.02,
            make_hurst(0.1),
            2.0,
            0.04,
            0.3,
            -0.7,
            0.04,
        )
        .expect("valid");

        let meta = process.metadata();
        assert_eq!(meta.process_type, "RoughHeston");
    }
}
