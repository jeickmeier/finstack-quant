//! Geometric Brownian Motion (GBM) process for equity and FX simulation.
//!
//! Implements the standard stochastic differential equation for asset prices
//! under the risk-neutral measure. This is the fundamental process underlying
//! the Black-Scholes-Merton framework.
//!
//! # Stochastic Differential Equation
//!
//! Under the risk-neutral measure ℚ:
//!
//! ```text
//! dS_t = (r - q) S_t dt + σ S_t dW_t
//! ```
//!
//! where:
//! - **S_t**: Asset price at time t
//! - **r**: Risk-free interest rate (continuously compounded)
//! - **q**: Dividend yield (equity) or foreign rate (FX)
//! - **σ**: Volatility (constant in standard GBM)
//! - **W_t**: Standard Brownian motion under ℚ
//!
//! # Exact Simulation
//!
//! GBM admits exact simulation without discretization bias. The solution is:
//!
//! ```text
//! S_{t+Δt} = S_t · exp[(r - q - σ²/2)Δt + σ√Δt · Z]
//! ```
//!
//! where Z ~ N(0,1). This is **unbiased** regardless of step size Δt.
//!
//! # Applications
//!
//! - **Equity options**: Standard model for single-stock options
//! - **FX options**: Garman-Kohlhagen model (GBM with two rates)
//! - **Index options**: Assuming constant dividend yield
//! - **Commodity options**: With convenience yield q
//!
//! # Limitations
//!
//! GBM assumes:
//! - Constant volatility (no smile/skew)
//! - Lognormal returns (no jumps)
//! - Continuous trading
//! - No transaction costs
//!
//! For more realistic models, see:
//! - [`crate::process::heston::HestonProcess`] for stochastic volatility
//! - Local volatility models for calibrated smiles
//!
//! # References
//!
//! - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate
//!   Liabilities." *Journal of Political Economy*, 81(3), 637-654.
//!
//! - Merton, R. C. (1973). "Theory of Rational Option Pricing."
//!   *Bell Journal of Economics and Management Science*, 4(1), 141-183.
//!
//! - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
//!   Springer. Section 3.1: Generating Sample Paths. pp. 97-103.
//!
//! # Examples
//!
//! ```rust
//! use finstack_monte_carlo::process::gbm::GbmProcess;
//! use finstack_monte_carlo::traits::StochasticProcess;
//!
//! let gbm = GbmProcess::with_params(
//!     0.05,  // r = 5% risk-free rate
//!     0.02,  // q = 2% dividend yield
//!     0.20,  // sigma = 20% volatility
//! )
//! .unwrap();
//!
//! let mut drift = [0.0];
//! let mut diffusion = [0.0];
//! gbm.drift(0.0, &[100.0], &mut drift);
//! gbm.diffusion(0.0, &[100.0], &mut diffusion);
//!
//! assert_eq!(gbm.dim(), 1);
//! assert!((drift[0] - 3.0).abs() < 1e-12);
//! assert!((diffusion[0] - 20.0).abs() < 1e-12);
//! ```
//!
//! Full pricing example:
//!
//! ```ignore
//! use finstack_core::currency::Currency;
//! use finstack_monte_carlo::prelude::*;
//!
//! let engine = McEngine::builder()
//!     .num_paths(10_000)
//!     .uniform_grid(1.0, 252)
//!     .parallel(false)
//!     .build()
//!     .unwrap();
//! let rng = PhiloxRng::new(42);
//!
//! let gbm = GbmProcess::with_params(
//!     0.05,  // r = 5% risk-free rate
//!     0.02,  // q = 2% dividend yield
//!     0.20,  // sigma = 20% volatility
//! ).unwrap();
//! let disc = ExactGbm::new();
//! let payoff = EuropeanCall::new(100.0, 1.0, 252);
//! let result = engine
//!     .price(
//!         &rng,
//!         &gbm,
//!         &disc,
//!         &[100.0],
//!         &payoff,
//!         Currency::USD,
//!         (-0.05_f64).exp(),
//!     )
//!     .unwrap();
//!
//! assert!(result.mean.amount().is_finite());
//! ```

use super::super::paths::ProcessParams;
use super::super::traits::{state_keys, PathState, ProportionalDiffusion, StochasticProcess};
use super::correlation::validate_correlation_matrix;
use super::metadata::ProcessMetadata;
use std::sync::Arc;

/// Geometric Brownian Motion parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GbmParams {
    /// Risk-free rate (annual)
    pub r: f64,
    /// Dividend/foreign rate (annual)
    pub q: f64,
    /// Volatility (annual)
    pub sigma: f64,
}

impl GbmParams {
    /// Create new GBM parameters with validation.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `r` is non-finite
    /// - `q` is non-finite
    /// - `sigma` is non-finite or negative
    pub fn new(r: f64, q: f64, sigma: f64) -> finstack_core::Result<Self> {
        if !r.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "GBM r (risk-free rate) must be finite, got {r}"
            )));
        }
        if !q.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "GBM q (dividend yield) must be finite, got {q}"
            )));
        }
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(finstack_core::Error::Validation(format!(
                "GBM sigma (volatility) must be finite and non-negative, got {sigma}"
            )));
        }
        Ok(Self { r, q, sigma })
    }
}

/// Deterministic time-varying drift schedule for GBM exact simulation.
///
/// Holds the cumulative risk-neutral log-drift `M(t) = ∫₀ᵗ (r(u) − q(u)) du`
/// sampled at a sorted set of knot times, with linear interpolation between
/// knots (equivalently: piecewise-constant instantaneous drift). The standard
/// constant-drift GBM is the special case `M(t) = (r − q)·t`.
///
/// Attaching a schedule to a [`GbmProcess`] lets
/// [`ExactGbm`](crate::discretization::exact::ExactGbm) price path-dependent
/// payoffs (Asian, lookback) on a non-flat rate curve without the per-fixing
/// forward bias a single maturity-averaged drift introduces.
#[derive(Debug, Clone)]
pub struct DriftSchedule {
    /// Sorted, strictly increasing knot times (years).
    times: Vec<f64>,
    /// Cumulative log-drift `M(t)` at each knot. Only differences are used, so
    /// the absolute offset is immaterial.
    cumulative_drift: Vec<f64>,
}

impl DriftSchedule {
    /// Build a drift schedule from knot times and cumulative log-drift values.
    ///
    /// # Errors
    ///
    /// Returns an error if the inputs are inconsistent: mismatched lengths,
    /// fewer than two knots, times not strictly increasing, or any non-finite
    /// value.
    pub fn new(times: Vec<f64>, cumulative_drift: Vec<f64>) -> finstack_core::Result<Self> {
        if times.len() != cumulative_drift.len() {
            return Err(finstack_core::Error::Validation(format!(
                "DriftSchedule: times ({}) and cumulative_drift ({}) length mismatch",
                times.len(),
                cumulative_drift.len()
            )));
        }
        if times.len() < 2 {
            return Err(finstack_core::Error::Validation(
                "DriftSchedule requires at least two knots".to_string(),
            ));
        }
        for w in times.windows(2) {
            if w[1] <= w[0] {
                return Err(finstack_core::Error::Validation(
                    "DriftSchedule times must be strictly increasing".to_string(),
                ));
            }
        }
        if times
            .iter()
            .chain(cumulative_drift.iter())
            .any(|v| !v.is_finite())
        {
            return Err(finstack_core::Error::Validation(
                "DriftSchedule times and cumulative_drift values must be finite".to_string(),
            ));
        }
        Ok(Self {
            times,
            cumulative_drift,
        })
    }

    /// Cumulative log-drift `M(t)`, linearly interpolated between knots and
    /// flat-slope-extrapolated outside the knot range.
    pub fn cumulative(&self, t: f64) -> f64 {
        let times = &self.times;
        let m = &self.cumulative_drift;
        // Knot times are validated finite at construction and `t` comes from the
        // (finite) engine time grid. Use `total_cmp` for a total order so a
        // stray NaN query cannot silently collapse to `Ordering::Less` and
        // mis-locate the segment (the INVARIANTS §2.4 float-comparison rule).
        debug_assert!(t.is_finite(), "DriftSchedule::cumulative requires finite t");
        match times.binary_search_by(|x| x.total_cmp(&t)) {
            Ok(i) => m[i],
            Err(0) => {
                let slope = (m[1] - m[0]) / (times[1] - times[0]);
                m[0] + slope * (t - times[0])
            }
            Err(i) if i >= times.len() => {
                let n = times.len();
                let slope = (m[n - 1] - m[n - 2]) / (times[n - 1] - times[n - 2]);
                m[n - 1] + slope * (t - times[n - 1])
            }
            Err(i) => {
                let slope = (m[i] - m[i - 1]) / (times[i] - times[i - 1]);
                m[i - 1] + slope * (t - times[i - 1])
            }
        }
    }

    /// Risk-neutral log-drift increment `∫_{t0}^{t1} (r − q) du` over a step.
    pub fn log_drift_increment(&self, t0: f64, t1: f64) -> f64 {
        self.cumulative(t1) - self.cumulative(t0)
    }
}

/// Single-factor GBM process (1D).
///
/// State: S (spot price)
/// Factor: 1 Brownian motion
#[derive(Debug, Clone)]
pub struct GbmProcess {
    params: GbmParams,
    /// Optional deterministic time-varying drift schedule. When present,
    /// `ExactGbm` uses it instead of the constant `(r − q)` drift.
    drift_schedule: Option<Arc<DriftSchedule>>,
}

impl GbmProcess {
    /// Create a new GBM process.
    pub fn new(params: GbmParams) -> Self {
        Self {
            params,
            drift_schedule: None,
        }
    }

    /// Create with explicit parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if any parameter is invalid (see [`GbmParams::new`]).
    pub fn with_params(r: f64, q: f64, sigma: f64) -> finstack_core::Result<Self> {
        Ok(Self::new(GbmParams::new(r, q, sigma)?))
    }

    /// Attach a deterministic time-varying drift schedule.
    ///
    /// When present, [`ExactGbm`](crate::discretization::exact::ExactGbm) uses
    /// the schedule's per-step log-drift increment instead of the constant
    /// `(r − q)·Δt`, removing the per-fixing forward bias on a non-flat rate
    /// curve. The instantaneous [`StochasticProcess::drift`] (used by generic
    /// Euler / Milstein schemes) keeps the constant `(r − q)` rate, so a
    /// schedule should be paired with `ExactGbm`.
    pub fn with_drift_schedule(mut self, schedule: Arc<DriftSchedule>) -> Self {
        self.drift_schedule = Some(schedule);
        self
    }

    /// Get parameters.
    pub fn params(&self) -> &GbmParams {
        &self.params
    }

    /// Time-varying drift schedule, if one is attached.
    pub fn drift_schedule(&self) -> Option<&DriftSchedule> {
        self.drift_schedule.as_deref()
    }

    /// Risk-neutral drift rate.
    pub fn drift_rate(&self) -> f64 {
        self.params.r - self.params.q
    }

    /// Volatility.
    pub fn volatility(&self) -> f64 {
        self.params.sigma
    }
}

impl StochasticProcess for GbmProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // μ(S) = (r - q) S
        out[0] = self.drift_rate() * x[0];
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        // σ(S) = σ S
        out[0] = self.params.sigma * x[0];
    }
}

impl ProcessMetadata for GbmProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("GBM");
        params.add_param("r", self.params.r);
        params.add_param("q", self.params.q);
        params.add_param("sigma", self.params.sigma);
        params.with_factors(vec!["spot".to_string()])
    }
}

// Contract: `GbmProcess::diffusion` returns `sigma * x[0]`, satisfying the
// proportional-diffusion contract `σ(X) = σ_const · X` exactly.
impl ProportionalDiffusion for GbmProcess {}

/// Multi-factor GBM (for correlated assets).
///
/// State: [S_1, S_2, ..., S_n]
/// Factors: n correlated Brownian motions
///
/// Each asset i follows:
/// ```text
/// dS_i = (r - q_i) S_i dt + σ_i S_i dW_i
/// ```
///
/// where W_i are correlated via a correlation matrix ρ.
#[derive(Debug, Clone)]
pub struct MultiGbmProcess {
    /// Parameters for each asset
    params: Vec<GbmParams>,
    /// Correlation matrix (n x n, row-major)
    correlation: Option<Vec<f64>>,
}

impl MultiGbmProcess {
    /// Create a multi-factor GBM process.
    ///
    /// # Arguments
    ///
    /// * `params` - Parameters for each asset
    /// * `correlation` - Optional correlation matrix (if None, assumes independence)
    ///
    /// # Errors
    ///
    /// Returns an error if a supplied correlation matrix is malformed or not
    /// positive semi-definite.
    pub fn new(
        params: Vec<GbmParams>,
        correlation: Option<Vec<f64>>,
    ) -> finstack_core::Result<Self> {
        let n = params.len();
        if let Some(ref corr) = correlation {
            validate_correlation_matrix(corr, n)?;
        }
        Ok(Self {
            params,
            correlation,
        })
    }

    /// Number of assets.
    pub fn num_assets(&self) -> usize {
        self.params.len()
    }

    /// Get asset parameters.
    pub fn asset_params(&self, i: usize) -> &GbmParams {
        &self.params[i]
    }

    /// Get correlation matrix (if set).
    pub fn correlation(&self) -> Option<&[f64]> {
        self.correlation.as_deref()
    }
}

impl StochasticProcess for MultiGbmProcess {
    fn dim(&self) -> usize {
        self.params.len()
    }

    fn num_factors(&self) -> usize {
        self.params.len()
    }

    fn drift(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        for (i, params) in self.params.iter().enumerate() {
            out[i] = (params.r - params.q) * x[i];
        }
    }

    fn diffusion(&self, _t: f64, x: &[f64], out: &mut [f64]) {
        for (i, params) in self.params.iter().enumerate() {
            out[i] = params.sigma * x[i];
        }
    }

    fn factor_correlation(&self) -> Option<Vec<f64>> {
        self.correlation.clone()
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        if let Some(&spot) = x.first() {
            state.set(state_keys::SPOT, spot);
        }
        for (i, &spot) in x.iter().enumerate() {
            state.set_indexed_spot(i, spot);
        }
    }
}

impl ProcessMetadata for MultiGbmProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("MultiGBM");

        // Add per-asset parameters
        for (i, asset_params) in self.params.iter().enumerate() {
            params.add_param(format!("r_{}", i), asset_params.r);
            params.add_param(format!("q_{}", i), asset_params.q);
            params.add_param(format!("sigma_{}", i), asset_params.sigma);
        }

        // Add correlation matrix if present
        let params = if let Some(ref corr) = self.correlation {
            params.with_correlation(corr.clone())
        } else {
            params
        };

        // Add factor names
        let factor_names: Vec<String> = (0..self.params.len())
            .map(|i| format!("spot_{}", i))
            .collect();
        params.with_factors(factor_names)
    }
}

// Contract: each component diffusion is `σ_i * x[i]`, so the proportional
// contract holds independently in every dimension.
impl ProportionalDiffusion for MultiGbmProcess {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::PathState;

    #[test]
    fn test_gbm_creation() {
        let params = GbmParams::new(0.05, 0.02, 0.2).unwrap();
        let gbm = GbmProcess::new(params);

        assert_eq!(gbm.dim(), 1);
        assert!((gbm.drift_rate() - 0.03).abs() < 1e-10); // r - q
        assert!((gbm.volatility() - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_gbm_drift_diffusion() {
        let gbm = GbmProcess::with_params(0.05, 0.02, 0.2).unwrap();
        let x = [100.0];
        let mut drift = [0.0];
        let mut diffusion = [0.0];

        gbm.drift(0.0, &x, &mut drift);
        gbm.diffusion(0.0, &x, &mut diffusion);

        assert!((drift[0] - 3.0).abs() < 1e-10); // 0.03 * 100
        assert!((diffusion[0] - 20.0).abs() < 1e-10); // 0.2 * 100
    }

    #[test]
    fn test_multi_gbm() {
        let params = vec![
            GbmParams::new(0.05, 0.02, 0.2).unwrap(),
            GbmParams::new(0.05, 0.03, 0.3).unwrap(),
        ];
        let multi_gbm = MultiGbmProcess::new(params, None).unwrap();

        assert_eq!(multi_gbm.dim(), 2);
        assert_eq!(multi_gbm.num_assets(), 2);
        assert!(multi_gbm.factor_correlation().is_none());
    }

    #[test]
    fn test_multi_gbm_with_correlation() {
        let params = vec![
            GbmParams::new(0.05, 0.02, 0.2).unwrap(),
            GbmParams::new(0.05, 0.03, 0.3).unwrap(),
        ];
        // Correlation matrix: [[1.0, 0.5], [0.5, 1.0]]
        let corr = vec![1.0, 0.5, 0.5, 1.0];
        let multi_gbm = MultiGbmProcess::new(params, Some(corr)).unwrap();

        assert!(multi_gbm.factor_correlation().is_some());
        assert!(multi_gbm.correlation().is_some());
    }

    #[test]
    fn test_multi_gbm_populates_indexed_spot_state() {
        let params = vec![
            GbmParams::new(0.05, 0.02, 0.2).unwrap(),
            GbmParams::new(0.05, 0.03, 0.3).unwrap(),
        ];
        let process = MultiGbmProcess::new(params, None).unwrap();
        let mut state = PathState::new(0, 0.0);

        process.populate_path_state(&[100.0, 120.0], &mut state);

        assert_eq!(state.get("spot"), Some(100.0));
        assert_eq!(state.get("spot_0"), Some(100.0));
        assert_eq!(state.get("spot_1"), Some(120.0));
    }

    #[test]
    fn test_multi_gbm_new_rejects_asymmetric_correlation() {
        let params = vec![
            GbmParams::new(0.05, 0.02, 0.2).unwrap(),
            GbmParams::new(0.05, 0.03, 0.3).unwrap(),
        ];
        let corr = vec![1.0, 0.2, 0.4, 1.0];

        assert!(MultiGbmProcess::new(params, Some(corr)).is_err());
    }

    #[test]
    fn drift_schedule_constant_drift_matches_linear() {
        // M(t) = (r-q)*t reproduces the constant-drift increment exactly, so a
        // flat-curve schedule is bit-equivalent to the constant-drift GBM.
        let rq = 0.03;
        let times = vec![0.0, 0.25, 0.5, 1.0];
        let cum: Vec<f64> = times.iter().map(|t| rq * t).collect();
        let sched = DriftSchedule::new(times, cum).expect("valid schedule");

        for &(t0, t1) in &[(0.0, 0.25), (0.1, 0.4), (0.5, 1.0), (0.3, 0.3)] {
            let inc = sched.log_drift_increment(t0, t1);
            assert!(
                (inc - rq * (t1 - t0)).abs() < 1e-12,
                "increment {inc} over [{t0},{t1}] should be {}",
                rq * (t1 - t0)
            );
        }
    }

    #[test]
    fn drift_schedule_interpolates_and_extrapolates() {
        // Front-loaded cumulative drift: steeper over the first year.
        let sched =
            DriftSchedule::new(vec![0.0, 1.0, 2.0], vec![0.0, 0.06, 0.08]).expect("valid schedule");

        assert!((sched.cumulative(0.5) - 0.03).abs() < 1e-12); // linear in [0,1]
        assert!((sched.cumulative(1.5) - 0.07).abs() < 1e-12); // linear in [1,2]
        assert!((sched.cumulative(3.0) - 0.10).abs() < 1e-12); // last-slope extrapolation

        assert!(
            sched.log_drift_increment(0.0, 1.0) > sched.log_drift_increment(1.0, 2.0),
            "front-loaded schedule must carry more drift in its first half"
        );
    }

    #[test]
    fn drift_schedule_rejects_invalid_inputs() {
        assert!(
            DriftSchedule::new(vec![0.0], vec![0.0]).is_err(),
            "fewer than two knots must be rejected"
        );
        assert!(
            DriftSchedule::new(vec![0.0, 1.0], vec![0.0]).is_err(),
            "length mismatch must be rejected"
        );
        assert!(
            DriftSchedule::new(vec![0.0, 1.0, 1.0], vec![0.0, 0.03, 0.05]).is_err(),
            "non-increasing times must be rejected"
        );
        assert!(
            DriftSchedule::new(vec![0.0, f64::NAN], vec![0.0, 0.03]).is_err(),
            "non-finite time must be rejected"
        );
    }

    #[test]
    fn gbm_process_carries_optional_drift_schedule() {
        let process = GbmProcess::with_params(0.05, 0.02, 0.2).expect("valid gbm");
        assert!(process.drift_schedule().is_none());

        let sched =
            Arc::new(DriftSchedule::new(vec![0.0, 1.0], vec![0.0, 0.03]).expect("valid schedule"));
        let with_sched = process.with_drift_schedule(sched);
        assert!(with_sched.drift_schedule().is_some());
    }
}
