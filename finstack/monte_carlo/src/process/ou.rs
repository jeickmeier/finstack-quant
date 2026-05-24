//! Ornstein-Uhlenbeck and Hull-White 1-factor processes.
//!
//! Implements the single-factor short-rate model in the Vasicek-style
//! mean-reversion-level convention:
//!
//! ```text
//! dr_t = κ·[θ(t) - r_t] dt + σ dW_t
//! ```
//!
//! where:
//! - κ = mean reversion speed
//! - θ(t) = time-dependent mean reversion *level* (the value the rate is
//!   pulled toward — not the Brigo–Mercurio eq. 3.35 "drift term")
//! - σ = instantaneous volatility
//! - W_t = Brownian motion
//!
//! # Convention — θ(t) is the stationary level
//!
//! The HW1F literature also uses a drift form `(θ_BM(t) - κ·r) dt` in
//! which `θ_BM` is **not** the stationary level but `κ` times the
//! stationary level. Both conventions are valid, but they are not
//! interchangeable: feeding a `θ_BM`-convention value into the drift
//! `κ·(θ - r)` gives a stationary mean off by a factor of κ.
//!
//! This crate exclusively uses the Vasicek-style mean-reversion-level
//! convention. The calibrator [`calibrate_theta_from_curve`] produces θ
//! in this convention; callers constructing [`HullWhite1FParams::new`]
//! or [`HullWhite1FProcess::vasicek`] directly must likewise supply θ as
//! a stationary rate level.
//!
//! The Hull-White 1F model uses a time-dependent θ(t) to fit the initial
//! yield curve; the Ornstein-Uhlenbeck (Vasicek) model uses constant θ.

use super::super::traits::{state_keys, PathState, StochasticProcess};
use tracing::warn;

/// Hull-White 1-factor parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HullWhite1FParams {
    /// Mean reversion speed (κ)
    pub kappa: f64,
    /// Instantaneous volatility (σ)
    pub sigma: f64,
    /// Time-dependent mean reversion level θ(t)
    /// Stored as piecewise-constant segments
    pub theta_curve: Vec<f64>,
    /// Time breakpoints for θ(t) (must be sorted)
    pub theta_times: Vec<f64>,
}

impl HullWhite1FParams {
    /// Create new Hull-White 1F parameters with constant θ.
    pub fn new(kappa: f64, sigma: f64, theta: f64) -> Self {
        Self {
            kappa,
            sigma,
            theta_curve: vec![theta],
            theta_times: vec![0.0],
        }
    }

    /// Create with time-dependent θ(t).
    ///
    /// # Arguments
    ///
    /// * `kappa` - Mean reversion speed
    /// * `sigma` - Volatility
    /// * `theta_curve` - θ values (piecewise constant)
    /// * `theta_times` - Time breakpoints (must be sorted)
    pub fn with_time_dependent_theta(
        kappa: f64,
        sigma: f64,
        theta_curve: Vec<f64>,
        theta_times: Vec<f64>,
    ) -> Self {
        assert_eq!(
            theta_curve.len(),
            theta_times.len(),
            "Theta curve and times must have same length"
        );
        assert!(
            !theta_times.is_empty(),
            "Must have at least one theta value"
        );

        Self {
            kappa,
            sigma,
            theta_curve,
            theta_times,
        }
    }

    /// Get θ(t) at a given time.
    pub fn theta_at_time(&self, t: f64) -> f64 {
        // Find the appropriate theta value for time t
        // Use piecewise-constant interpolation
        for i in (0..self.theta_times.len()).rev() {
            if t >= self.theta_times[i] {
                return self.theta_curve[i];
            }
        }

        // If t < first breakpoint, use first value
        self.theta_curve[0]
    }
}

/// Hull-White 1-factor short rate process.
///
/// State dimension: 1 (short rate r)
/// Factor dimension: 1 (Brownian motion)
///
/// # SDE
///
/// ```text
/// dr_t = κ[θ(t) - r_t]dt + σ dW_t
/// ```
///
/// # Exact Solution
///
/// For piecewise-constant θ(t), there exists an exact discretization:
///
/// ```text
/// r_{t+Δt} = r_t e^{-κΔt} + θ(1 - e^{-κΔt}) + σ√[(1-e^{-2κΔt})/(2κ)] Z
/// ```
///
/// Use `ExactHullWhite1F` discretization for best accuracy.
#[derive(Debug, Clone)]
pub struct HullWhite1FProcess {
    params: HullWhite1FParams,
}

impl HullWhite1FProcess {
    /// Create a new Hull-White 1F process.
    pub fn new(params: HullWhite1FParams) -> Self {
        Self { params }
    }

    /// Create with constant θ (Vasicek model).
    pub fn vasicek(kappa: f64, theta: f64, sigma: f64) -> Self {
        Self::new(HullWhite1FParams::new(kappa, sigma, theta))
    }

    /// Get parameters.
    pub fn params(&self) -> &HullWhite1FParams {
        &self.params
    }

    /// Get θ(t) at a given time.
    pub fn theta_at_time(&self, t: f64) -> f64 {
        self.params.theta_at_time(t)
    }
}

impl StochasticProcess for HullWhite1FProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        // μ(r) = κ[θ(t) - r]
        let theta = self.theta_at_time(t);
        out[0] = self.params.kappa * (theta - x[0]);
    }

    fn diffusion(&self, _t: f64, _x: &[f64], out: &mut [f64]) {
        // σ(r) = σ (constant volatility)
        out[0] = self.params.sigma;
    }

    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        if !x.is_empty() {
            state.set(state_keys::SHORT_RATE, x[0]);
            state.set(state_keys::SPOT, x[0]);
        }
    }
}

// ============================================================================
// Curve-Derived θ(t) Calibration
// ============================================================================

/// Build Hull-White 1F parameters with θ(t) derived from a discount curve.
///
/// This crate uses the **Vasicek-style mean-reversion-level convention**
/// for θ: the drift is `κ·(θ(t) - r)`, so θ(t) is the time-dependent
/// stationary level the short rate is pulled toward. Under this
/// convention the calibrated θ(t) for a market-consistent HW1F is
///
/// ```text
/// θ(t) = f(0,t) + (1/κ)·∂f/∂t(0,t) + σ²/(2κ²)·(1 − e^{−2κt})
/// ```
///
/// where `f(0,t) = -d/dt ln P(0,t)` is the market instantaneous forward.
///
/// This is algebraically equivalent to the more commonly-cited
/// Brigo–Mercurio form `θ_HW(t) = ∂f/∂t + κ·f(0,t) + σ²/(2κ)·(1-e^{-2κt})`
/// used with the drift form `(θ_HW(t) - κ·r)dt`, via `θ_Vas = θ_HW / κ`.
/// The two forms give identical dynamics when θ is interpreted
/// consistently with the drift; mixing them produces a stationary mean
/// off by a factor of κ.
///
/// For a flat curve at rate `r_flat`: ∂f/∂t = 0 and f(0,t) = r_flat, so
/// θ(t) = r_flat + σ²/(2κ²)·(1 − e^{−2κt}) ≈ r_flat.
///
/// # Arguments
///
/// * `kappa` - Mean reversion speed
/// * `sigma` - Short rate volatility
/// * `discount_curve_fn` - Function mapping time (years) to discount factor P(0,t)
/// * `theta_times` - Time breakpoints for θ(t) discretization
///
/// # Panics / limits
///
/// For numerical stability `κ` must satisfy `|κ| > 1e-10`. The Vasicek-form
/// θ(t) involves `∂f/∂t / κ` and `σ²/(2κ²)` terms that diverge as `κ → 0`.
/// Callers simulating a driftless Brownian-motion short rate (κ ≈ 0)
/// should use a dedicated `BrownianMotion` process rather than HW1F with
/// tiny κ.
///
/// # Example
///
/// ```ignore
/// use finstack_monte_carlo::process::ou::calibrate_theta_from_curve;
///
/// let discount_fn = |t: f64| (-0.03 * t).exp();  // Flat 3% curve
/// let params = calibrate_theta_from_curve(0.03, 0.01, discount_fn, &[0.5, 1.0, 2.0, 5.0]);
/// # let _ = params;
/// ```
pub fn calibrate_theta_from_curve<F>(
    kappa: f64,
    sigma: f64,
    discount_curve_fn: F,
    theta_times: &[f64],
) -> HullWhite1FParams
where
    F: Fn(f64) -> f64,
{
    if theta_times.is_empty() {
        // Fallback: use instantaneous forward at t=0 as constant theta.
        // f(0,0) is itself the stationary level for a "flat-near-zero"
        // curve under the Vasicek convention.
        let f_0 = instantaneous_forward(&discount_curve_fn, 0.0);
        return HullWhite1FParams::new(kappa, sigma, f_0);
    }

    let mut theta_curve = Vec::with_capacity(theta_times.len());

    for &t in theta_times {
        let theta_t = compute_theta_at_time(kappa, sigma, &discount_curve_fn, t);
        theta_curve.push(theta_t);
    }

    HullWhite1FParams::with_time_dependent_theta(kappa, sigma, theta_curve, theta_times.to_vec())
}

/// Compute θ(t) at a specific time in the Vasicek-style mean-reversion-level
/// convention used by this crate's HW1F drift `κ·(θ(t) - r)`.
///
/// ```text
/// θ(t) = f(0,t) + (1/κ)·∂f/∂t(0,t) + σ²/(2κ²)·(1 − e^{−2κt})
/// ```
///
/// Derivation: the Brigo–Mercurio (2006) eq. 3.35 form
///   θ_HW(t) = ∂f/∂t(0,t) + κ·f(0,t) + σ²/(2κ)·(1 − e^{−2κt})
/// is meant to be consumed by a drift of the form `θ_HW(t) - κ·r`.
/// Dividing by κ converts it to the Vasicek-style mean-reversion level
/// θ_Vas = θ_HW / κ that the drift `κ·(θ - r)` expects.
///
/// Reference: Brigo & Mercurio (2006) *Interest Rate Models* §3.3.1 eq. 3.35;
/// Hull & White (1990).
fn compute_theta_at_time<F>(kappa: f64, sigma: f64, discount_curve_fn: &F, t: f64) -> f64
where
    F: Fn(f64) -> f64,
{
    // For near-zero κ the Vasicek-form θ diverges (as ∂f/∂t / κ and
    // σ²/(2κ²)). Callers near κ = 0 should be using a driftless process;
    // here we fall back to the constant level f(0,t) which at least keeps
    // the simulator bounded.
    const KAPPA_EPS: f64 = 1e-10;
    if kappa.abs() < KAPPA_EPS {
        return instantaneous_forward(discount_curve_fn, t);
    }

    // Compute f(0,t) = instantaneous forward rate
    let f_t = instantaneous_forward(discount_curve_fn, t);

    // Compute ∂f/∂t via finite difference
    let df_dt = forward_derivative(discount_curve_fn, t);

    // Vol-correction term in the Vasicek convention: σ²/(2κ²)·(1 − e^{−2κt}).
    // This is the O(σ²/κ²) gap between the stationary level and the market
    // instantaneous forward that arises from the HW1F drift-and-diffusion
    // balance. See Brigo–Mercurio §3.3.1 eq. 3.35 divided by κ.
    let vol_term = (sigma * sigma) / (2.0 * kappa * kappa) * (1.0 - (-2.0 * kappa * t).exp());

    f_t + df_dt / kappa + vol_term
}

/// Compute instantaneous forward rate f(0,t) = -d/dt ln P(0,t)
/// using central finite differences.
fn instantaneous_forward<F>(discount_curve_fn: &F, t: f64) -> f64
where
    F: Fn(f64) -> f64,
{
    let eps = 1e-4; // Small perturbation for numerical derivative

    if t < eps {
        // Near t=0, use forward difference
        let df_0 = discount_curve_fn(0.0);
        let df_eps = discount_curve_fn(eps);
        if df_0 > 0.0 && df_eps > 0.0 {
            -(df_eps.ln() - df_0.ln()) / eps
        } else {
            warn!(t, "instantaneous_forward: non-positive discount factor near t=0; returning 0.0. Check market data quality.");
            0.0
        }
    } else {
        // Central difference: f(t) ≈ -[ln P(t+ε) - ln P(t-ε)] / (2ε)
        let t_plus = t + eps;
        let t_minus = (t - eps).max(0.0);
        let df_plus = discount_curve_fn(t_plus);
        let df_minus = discount_curve_fn(t_minus);

        if df_plus > 0.0 && df_minus > 0.0 {
            -(df_plus.ln() - df_minus.ln()) / (t_plus - t_minus)
        } else {
            warn!(t, df_plus, df_minus, "instantaneous_forward: non-positive discount factor; returning 0.0. Check market data quality.");
            0.0
        }
    }
}

/// Compute ∂f/∂t via the second derivative of ln P(0,t).
///
/// Since f(0,t) = -d/dt ln P(0,t), we have ∂f/∂t = -d²/dt² ln P(0,t).
/// Computing this directly from P avoids the double finite-difference
/// (differentiating a finite-difference approximation of f), which would
/// amplify curve noise.
fn forward_derivative<F>(discount_curve_fn: &F, t: f64) -> f64
where
    F: Fn(f64) -> f64,
{
    let eps = 1e-4;

    if t < eps {
        // Near t=0, use forward difference on f
        let f_0 = instantaneous_forward(discount_curve_fn, 0.0);
        let f_eps = instantaneous_forward(discount_curve_fn, eps);
        (f_eps - f_0) / eps
    } else {
        // Central second derivative of ln P:
        // ∂f/∂t = -[ln P(t+ε) - 2·ln P(t) + ln P(t-ε)] / ε²
        let p_plus = discount_curve_fn(t + eps);
        let p_mid = discount_curve_fn(t);
        let p_minus = discount_curve_fn((t - eps).max(0.0));

        let dt_actual = (t + eps) - (t - eps).max(0.0);
        let half_dt = dt_actual / 2.0;

        if p_plus > 0.0 && p_mid > 0.0 && p_minus > 0.0 {
            -(p_plus.ln() - 2.0 * p_mid.ln() + p_minus.ln()) / (half_dt * half_dt)
        } else {
            // Fallback to differentiating instantaneous forwards
            let f_plus = instantaneous_forward(discount_curve_fn, t + eps);
            let f_minus = instantaneous_forward(discount_curve_fn, (t - eps).max(0.0));
            (f_plus - f_minus) / dt_actual
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hw1f_params_constant_theta() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03);

        assert_eq!(params.kappa, 0.1);
        assert_eq!(params.sigma, 0.01);
        assert_eq!(params.theta_at_time(0.0), 0.03);
        assert_eq!(params.theta_at_time(1.0), 0.03);
        assert_eq!(params.theta_at_time(10.0), 0.03);
    }

    #[test]
    fn test_hw1f_params_time_dependent_theta() {
        let theta_curve = vec![0.02, 0.03, 0.04];
        let theta_times = vec![0.0, 1.0, 2.0];

        let params =
            HullWhite1FParams::with_time_dependent_theta(0.1, 0.01, theta_curve, theta_times);

        assert_eq!(params.theta_at_time(0.0), 0.02);
        assert_eq!(params.theta_at_time(0.5), 0.02);
        assert_eq!(params.theta_at_time(1.0), 0.03);
        assert_eq!(params.theta_at_time(1.5), 0.03);
        assert_eq!(params.theta_at_time(2.0), 0.04);
        assert_eq!(params.theta_at_time(10.0), 0.04);
    }

    #[test]
    fn test_hw1f_drift() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03);
        let process = HullWhite1FProcess::new(params);

        let x = vec![0.04]; // Rate above mean
        let mut drift = vec![0.0];

        process.drift(0.0, &x, &mut drift);

        // Drift should be negative (pull back to mean)
        assert!(drift[0] < 0.0);
        assert_eq!(drift[0], 0.1 * (0.03 - 0.04));
    }

    #[test]
    fn test_hw1f_diffusion() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03);
        let process = HullWhite1FProcess::new(params);

        let x = vec![0.05];
        let mut diffusion = vec![0.0];

        process.diffusion(0.0, &x, &mut diffusion);

        // Constant diffusion
        assert_eq!(diffusion[0], 0.01);
    }

    #[test]
    fn test_vasicek_alias() {
        let process = HullWhite1FProcess::vasicek(0.1, 0.03, 0.01);

        assert_eq!(process.params().kappa, 0.1);
        assert_eq!(process.params().sigma, 0.01);
        assert_eq!(process.theta_at_time(0.0), 0.03);
    }

    #[test]
    fn test_calibrate_theta_from_flat_curve() {
        // Flat 3% curve: P(0,t) = exp(-0.03 * t)
        let discount_fn = |t: f64| (-0.03 * t).exp();
        let theta_times = vec![0.0, 1.0, 2.0, 5.0, 10.0];

        let params = calibrate_theta_from_curve(0.03, 0.01, discount_fn, &theta_times);

        // For a flat curve, f(0,t) = 0.03 and ∂f/∂t ≈ 0
        // θ(t) ≈ κ·f + volatility term
        // At t=0, vol term = 0, so θ(0) ≈ κ·r = 0.03 * 0.03 = 0.0009
        // But we add the forward derivative which is near zero
        // Actually for flat curve: θ(t) = κ·f + σ²/(2κ²)·(1-e^{-κt})²

        assert_eq!(params.kappa, 0.03);
        assert_eq!(params.sigma, 0.01);
        assert_eq!(params.theta_times.len(), 5);

        // Theta values should be positive and reasonable
        for &theta in &params.theta_curve {
            assert!(theta.is_finite(), "Theta must be finite");
            assert!(
                theta > -0.1 && theta < 0.2,
                "Theta should be reasonable: {}",
                theta
            );
        }
    }

    #[test]
    fn test_calibrate_theta_empty_times() {
        let discount_fn = |t: f64| (-0.03 * t).exp();
        let params = calibrate_theta_from_curve(0.03, 0.01, discount_fn, &[]);

        // Should fall back to constant theta at instantaneous forward f(0,0)
        assert_eq!(params.theta_curve.len(), 1);
        assert_eq!(params.theta_times.len(), 1);
    }

    #[test]
    fn test_instantaneous_forward_flat_curve() {
        let discount_fn = |t: f64| (-0.05 * t).exp();

        // For flat curve, f(0,t) should be constant ≈ 0.05
        let f_0 = instantaneous_forward(&discount_fn, 0.0);
        let f_1 = instantaneous_forward(&discount_fn, 1.0);
        let f_5 = instantaneous_forward(&discount_fn, 5.0);

        assert!((f_0 - 0.05).abs() < 0.01, "f(0,0) ≈ 0.05, got {}", f_0);
        assert!((f_1 - 0.05).abs() < 0.01, "f(0,1) ≈ 0.05, got {}", f_1);
        assert!((f_5 - 0.05).abs() < 0.01, "f(0,5) ≈ 0.05, got {}", f_5);
    }

    /// Verify that calibrated θ(t) matches the analytical HW1F formula.
    ///
    /// For a flat yield curve at rate r_flat:
    ///   f(0,t) = r_flat,  ∂f/∂t = 0
    ///
    /// The HW1F formula (Brigo & Mercurio 2006, eq. 3.35) gives:
    ///   θ(t) = ∂f/∂t + κ·f(0,t) + σ²/(2κ)·(1 - e^{-2κt})
    ///         = κ·r_flat + σ²/(2κ)·(1 - e^{-2κt})
    ///
    /// This test validates that `calibrate_theta_from_curve` produces θ values
    /// matching this formula to numerical precision (the only error is from the
    /// finite-difference approximation of the instantaneous forward rate).
    #[test]
    fn test_calibrate_theta_matches_analytical_formula_flat_curve() {
        let r_flat = 0.05_f64;
        let kappa = 0.2_f64;
        let sigma = 0.01_f64;

        // Flat discount curve: P(0,t) = exp(-r_flat * t)
        let discount_fn = |t: f64| (-r_flat * t).exp();

        let times: Vec<f64> = vec![0.5, 1.0, 2.0, 3.0, 5.0, 10.0];
        let params = calibrate_theta_from_curve(kappa, sigma, discount_fn, &times);

        // Check each calibrated θ(t) against the analytical formula in the
        // *Vasicek-style mean-reversion-level* convention, which is what the
        // HW1F drift `κ·(θ - r)` consumes. Under this convention,
        //
        //   θ_Vas(t) = θ_HW(t) / κ
        //            = (∂f/∂t)/κ + f(0,t) + σ²/(2κ²)·(1 - e^{−2κt}).
        //
        // For a flat curve ∂f/∂t = 0 and f(0,t) = r_flat, so
        //
        //   θ_Vas(t) = r_flat + σ²/(2κ²)·(1 - e^{−2κt}).
        //
        // The BM 3.35 θ_HW formula used directly with the Vasicek-style
        // `κ·(θ - r)` drift form produces a stationary mean of κ·r_flat
        // instead of r_flat — the two θ conventions must not be mixed.
        for &t in &times {
            if t < 1e-8 {
                continue; // Skip t=0 where FD approximation is poorest
            }
            let theta_calibrated = params.theta_at_time(t);

            let theta_analytical =
                r_flat + (sigma * sigma) / (2.0 * kappa * kappa) * (1.0 - (-2.0 * kappa * t).exp());

            let abs_err = (theta_calibrated - theta_analytical).abs();
            assert!(
                abs_err < 1e-4,
                "θ mismatch at t={t}: calibrated={theta_calibrated:.8}, analytical={theta_analytical:.8}, err={abs_err:.2e}"
            );
        }
    }

    // ========================================================================
    // Hull-White drift initial-curve fit
    // ========================================================================

    /// Behavioural regression for the Vasicek / Brigo-Mercurio θ
    /// convention split.
    ///
    /// For a flat discount curve at rate `r_flat`, the HW1F model calibrated
    /// to that curve must produce a short-rate process whose stationary mean
    /// converges to `r_flat` — not to `κ·r_flat` (the pre-fix bug) and not
    /// to any other scale-shifted value.
    ///
    /// Because the process uses the exact conditional-distribution stepper
    /// (`ExactHullWhite1F`), we can verify this property analytically by
    /// evaluating the drift at `r = r_flat` and `t → large`: it must vanish
    /// within the small vol correction `σ²/(2κ²)·(1 − e^{−2κt})`.
    ///
    /// Pre-fix behaviour: with `κ = 0.2, r_flat = 5%`, the calibrator
    /// returned `θ_HW ≈ κ·r_flat = 1%` and the drift at r = 5% was
    /// `κ·(0.01 − 0.05) = −0.8%`/year — i.e., the model pulled hard toward
    /// 1% instead of staying near 5%.
    #[test]
    fn hw_drift_vanishes_at_curve_level_for_flat_curve() {
        let r_flat = 0.05_f64;
        let kappa = 0.2_f64;
        let sigma = 0.01_f64;

        let discount_fn = |t: f64| (-r_flat * t).exp();
        let times: Vec<f64> = vec![1.0, 2.0, 5.0, 10.0, 20.0];
        let params = calibrate_theta_from_curve(kappa, sigma, discount_fn, &times);
        let process = HullWhite1FProcess::new(params);

        // Evaluate the drift at r = r_flat for several t. At large t the
        // vol correction approaches σ²/(2κ²) = 1.25e-4, so the drift should
        // be tiny.
        let mut drift_buf = vec![0.0_f64];
        for &t in &times {
            process.drift(t, &[r_flat], &mut drift_buf);

            // The expected drift at r = r_flat under the fix is
            //     κ · (θ_Vas(t) − r_flat)
            //   = κ · σ²/(2κ²)·(1 − e^{−2κt})
            //   = σ²/(2κ)·(1 − e^{−2κt})
            // which is O(σ²/κ), tiny for typical calibrations. Bound it
            // with a generous tolerance (5 bp/year) that nevertheless
            // rejects the pre-fix O(κ·r_flat) = 1%/year signal.
            let expected_magnitude =
                (sigma * sigma) / (2.0 * kappa) * (1.0 - (-2.0 * kappa * t).exp());
            let tolerance = 5e-4; // 5 bp/year — between pre-fix (~8e-3) and true (~2.5e-5)

            assert!(
                drift_buf[0].abs() < tolerance,
                "HW1F drift at r=r_flat must be O(σ²/κ) = {:.2e}, got {:.2e} at t={}; \
                 pre-fix C2 bug produced drift ≈ −0.008 here (pulling toward κ·r_flat)",
                expected_magnitude,
                drift_buf[0],
                t,
            );
        }
    }

    /// Under the exact HW1F stepper, the conditional mean of r_{t+Δt} given
    /// r_t = θ(t) is exactly θ(t) (no motion in expectation from the
    /// stationary level). This test asserts the initial-curve-fit invariant
    /// operationally: starting at r_0 = f(0, 0) and simulating with one
    /// stateful draw of zero shocks, the rate stays at r_flat within the
    /// vol-correction tolerance for a flat curve.
    #[test]
    fn hw_exact_step_from_curve_level_has_zero_expected_drift() {
        use super::super::super::discretization::exact_hw1f::ExactHullWhite1F;
        use super::super::super::traits::Discretization;

        let r_flat = 0.05_f64;
        let kappa = 0.2_f64;
        let sigma = 0.01_f64;
        let discount_fn = |t: f64| (-r_flat * t).exp();
        let times: Vec<f64> = vec![0.5, 1.0, 2.0, 5.0, 10.0];
        let params = calibrate_theta_from_curve(kappa, sigma, discount_fn, &times);
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();

        // One step of dt = 1 from r_t = r_flat at several t values, with a
        // zero shock. The result is the conditional expectation E[r_{t+dt} |
        // r_t = r_flat] which, under a correctly-calibrated HW1F on a flat
        // curve, should equal r_flat up to the vol correction.
        for &t in &times {
            let dt = 1.0_f64;
            let mut x = vec![r_flat];
            let z = vec![0.0_f64];
            let mut work = vec![0.0_f64; disc.work_size(&process)];
            disc.step(&process, t, dt, &mut x, &z, &mut work);

            // Pre-fix: at r_flat = 5%, κ = 0.2, 1-step mean under θ = κ·r =
            // 1% gives E[r_1] ≈ 0.05·e^{−0.2} + 0.01·(1 − e^{−0.2}) ≈ 0.0409
            // — a 91 bp/year downward "rate decay". Post-fix: within ≤ 5 bp
            // of r_flat (pure vol correction).
            let deviation = (x[0] - r_flat).abs();
            assert!(
                deviation < 5e-4,
                "Exact HW1F conditional mean drifted from curve level at t={}: \
                 |r_1 − r_flat| = {:.6} (expected < 5bp). \
                 Pre-fix C2 bug produced 91 bp/year downward drift here.",
                t,
                deviation
            );
        }
    }
}
