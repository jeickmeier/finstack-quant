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

use super::super::traits::{PathState, StateKey, StochasticProcess};
use crate::rates::hull_white::{fd_instantaneous_forward, HullWhiteParams};
use finstack_quant_core::math::piecewise::PiecewiseConstantCurve;
use finstack_quant_core::Result;
use std::ops::Deref;
use tracing::warn;

/// Validated Hull-White one-factor process parameters.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(try_from = "RawHullWhite1FParams")]
pub struct HullWhite1FParams {
    /// Canonical mean reversion and volatility schedule.
    #[serde(flatten)]
    pub model: HullWhiteParams,
    /// Piecewise-constant mean-reversion levels.
    theta_curve: Vec<f64>,
    /// Left edges for `theta_curve`, starting at zero.
    theta_times: Vec<f64>,
}

#[derive(serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
struct RawHullWhite1FParams {
    #[serde(flatten)]
    model: HullWhiteParams,
    theta_curve: Vec<f64>,
    theta_times: Vec<f64>,
}

impl TryFrom<RawHullWhite1FParams> for HullWhite1FParams {
    type Error = finstack_quant_core::Error;

    fn try_from(raw: RawHullWhite1FParams) -> Result<Self> {
        Self::from_parts(raw.model, raw.theta_curve, raw.theta_times)
    }
}

impl Deref for HullWhite1FParams {
    type Target = HullWhiteParams;

    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

impl HullWhite1FParams {
    /// Create constant-volatility, constant-level Hull-White parameters.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Positive finite mean-reversion speed in inverse years.
    /// * `sigma` - Positive finite short-rate volatility per square-root year.
    /// * `theta` - Finite long-run short-rate level as a decimal.
    ///
    /// # Errors
    ///
    /// Returns an error when any parameter violates its finite/range contract.
    pub fn new(kappa: f64, sigma: f64, theta: f64) -> Result<Self> {
        Self::from_parts(
            HullWhiteParams::constant(kappa, sigma)?,
            vec![theta],
            vec![0.0],
        )
    }

    /// Create constant-volatility parameters with time-dependent θ(t).
    ///
    /// # Arguments
    ///
    /// * `kappa` - Positive finite mean-reversion speed in inverse years.
    /// * `sigma` - Positive finite short-rate volatility per square-root year.
    /// * `theta_curve` - Finite piecewise-constant long-run levels.
    /// * `theta_times` - Strictly increasing non-negative knot times.
    ///
    /// # Errors
    ///
    /// Returns an error when the model or theta schedule is invalid.
    pub fn with_time_dependent_theta(
        kappa: f64,
        sigma: f64,
        theta_curve: Vec<f64>,
        theta_times: Vec<f64>,
    ) -> Result<Self> {
        Self::from_parts(
            HullWhiteParams::constant(kappa, sigma)?,
            theta_curve,
            theta_times,
        )
    }

    /// Create parameters with piecewise-constant volatility and θ(t).
    ///
    /// # Arguments
    ///
    /// * `kappa` - Positive finite mean-reversion speed in inverse years.
    /// * `sigma_times` - Strictly increasing volatility knots starting at zero.
    /// * `sigma_values` - Positive finite volatility values aligned with the knots.
    /// * `theta_curve` - Finite piecewise-constant long-run levels.
    /// * `theta_times` - Strictly increasing non-negative theta knots.
    ///
    /// # Errors
    ///
    /// Returns an error when either schedule or mean reversion is invalid.
    pub fn with_piecewise_sigma(
        kappa: f64,
        sigma_times: Vec<f64>,
        sigma_values: Vec<f64>,
        theta_curve: Vec<f64>,
        theta_times: Vec<f64>,
    ) -> Result<Self> {
        Self::from_parts(
            HullWhiteParams::new(
                kappa,
                PiecewiseConstantCurve::new(sigma_times, sigma_values)?,
            )?,
            theta_curve,
            theta_times,
        )
    }

    fn from_parts(
        model: HullWhiteParams,
        theta_curve: Vec<f64>,
        theta_times: Vec<f64>,
    ) -> Result<Self> {
        if theta_curve.is_empty() || theta_curve.len() != theta_times.len() {
            return Err(finstack_quant_core::Error::Validation(
                "Hull-White theta curve and times must be non-empty and equally sized".into(),
            ));
        }
        for (index, (&time, &theta)) in theta_times.iter().zip(&theta_curve).enumerate() {
            if !time.is_finite() || time < 0.0 || !theta.is_finite() {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "invalid Hull-White theta knot at index {index}: time={time}, theta={theta}"
                )));
            }
            if index > 0 && time <= theta_times[index - 1] {
                return Err(finstack_quant_core::Error::Validation(
                    "Hull-White theta times must increase strictly".into(),
                ));
            }
        }
        Ok(Self {
            model,
            theta_curve,
            theta_times,
        })
    }

    /// Instantaneous short-rate volatility at `t`.
    #[must_use]
    pub fn sigma_at_time(&self, t: f64) -> f64 {
        self.model.volatility.value_at(t)
    }

    /// Exact OU state variance over `[t, t + dt]`.
    pub fn sigma_variance(&self, t: f64, dt: f64) -> Result<f64> {
        if dt <= 0.0 {
            return Ok(0.0);
        }
        self.model
            .volatility
            .integrate_squared_exp_weight(self.kappa, t + dt, t, t + dt)
    }

    /// Theta schedule values.
    #[must_use]
    pub fn theta_values(&self) -> &[f64] {
        &self.theta_curve
    }

    /// Theta schedule knot times.
    #[must_use]
    pub fn theta_times(&self) -> &[f64] {
        &self.theta_times
    }

    /// Exact OU variance for an already-validated simulation step.
    ///
    /// Simulation grids provide finite `t >= 0` and `dt > 0`; invalid values
    /// produce `NaN` so numerical validation can surface the error instead of
    /// silently pricing with a different volatility.
    #[must_use]
    pub fn sigma_variance_for_step(&self, t: f64, dt: f64) -> f64 {
        if !t.is_finite() || !dt.is_finite() || t < 0.0 || dt <= 0.0 {
            return f64::NAN;
        }
        self.sigma_variance(t, dt).unwrap_or(f64::NAN)
    }

    /// Get θ(t) at a given time.
    pub fn theta_at_time(&self, t: f64) -> f64 {
        // Piecewise-constant interpolation.
        for i in (0..self.theta_times.len()).rev() {
            if t >= self.theta_times[i] {
                return self.theta_curve[i];
            }
        }

        self.theta_curve[0]
    }

    /// Time-average of the piecewise-constant θ over `[t, t + dt]`.
    ///
    /// Integrates θ exactly across any knot boundaries inside the step and
    /// divides by `dt`. This is an ordinary arithmetic average; the exact
    /// Hull-White transition uses exponential weighting instead.
    ///
    /// # Arguments
    ///
    /// * `t` - Year-fraction time from the curve or surface base date to the query point
    /// * `dt` - Positive time-step width in year-fraction units.
    pub fn theta_average(&self, t: f64, dt: f64) -> f64 {
        if dt <= 0.0 {
            return self.theta_at_time(t);
        }
        let t_end = t + dt;
        let mut integral = 0.0;
        let mut seg_start = t;
        for &knot in &self.theta_times {
            if knot <= seg_start {
                continue;
            }
            if knot >= t_end {
                break;
            }
            integral += self.theta_at_time(seg_start) * (knot - seg_start);
            seg_start = knot;
        }
        integral += self.theta_at_time(seg_start) * (t_end - seg_start);
        integral / dt
    }

    /// Exact deterministic OU forcing over one step:
    /// `κ ∫ exp(-κ(t+dt-s)) θ(s) ds`, integrated segment by segment.
    pub(crate) fn theta_mean_contribution(&self, t: f64, dt: f64) -> f64 {
        let end = t + dt;
        let mut start = t;
        let mut contribution = 0.0;
        for boundary in self.theta_times.iter().copied().chain(std::iter::once(end)) {
            if boundary <= start {
                continue;
            }
            let segment_end = boundary.min(end);
            let weight = (-self.kappa * (end - segment_end)).exp()
                * -(-self.kappa * (segment_end - start)).exp_m1();
            contribution += self.theta_at_time(start) * weight;
            start = segment_end;
            if start >= end {
                break;
            }
        }
        contribution
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

    /// Create a constant-level Vasicek process.
    ///
    /// # Arguments
    ///
    /// * `kappa` - Positive finite mean-reversion speed in inverse years.
    /// * `theta` - Finite long-run short-rate level as a decimal.
    /// * `sigma` - Positive finite short-rate volatility per square-root year.
    ///
    /// # Errors
    ///
    /// Returns an error when a parameter is non-finite or outside its range.
    pub fn vasicek(kappa: f64, theta: f64, sigma: f64) -> Result<Self> {
        Ok(Self::new(HullWhite1FParams::new(kappa, sigma, theta)?))
    }

    /// Borrow the validated process parameters.
    pub fn params(&self) -> &HullWhite1FParams {
        &self.params
    }

    /// Get θ(t) at a given time.
    pub fn theta_at_time(&self, t: f64) -> f64 {
        self.params.theta_at_time(t)
    }

    /// Time-average of θ over `[t, t + dt]` (see
    /// [`HullWhite1FParams::theta_average`]).
    pub fn theta_average(&self, t: f64, dt: f64) -> f64 {
        self.params.theta_average(t, dt)
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

    fn diffusion(&self, t: f64, _x: &[f64], out: &mut [f64]) {
        out[0] = self.params.sigma_at_time(t);
    }

    /// Publish the short rate under both `SHORT_RATE` and `SPOT`.
    ///
    /// `SPOT` is aliased to the short rate so generic payoffs that read
    /// `state.spot()` can attach to this process. A vanilla payoff on
    /// [`HullWhite1FProcess`] is therefore a **short-rate option**, not an
    /// equity option on a traded spot.
    ///
    /// # Arguments
    ///
    /// * `x` - Process state vector; `x[0]` is the short rate `r_t`.
    /// * `state` - Path-state bag written for the current time step.
    fn populate_path_state(&self, x: &[f64], state: &mut PathState) {
        if !x.is_empty() {
            state.set_key(StateKey::ShortRate, x[0]);
            state.set_key(StateKey::Spot, x[0]);
        }
    }
}

/// Build Hull-White 1F parameters with θ(t) fitted to a discount curve.
///
/// This crate uses the **Vasicek-style mean-reversion-level convention**
/// for θ: the drift is `κ·(θ(t) - r)`, so θ(t) is the time-dependent
/// stationary level the short rate is pulled toward.
///
/// # Fit
///
/// A market-consistent HW1F short rate is `r(t) = x(t) + α(t)`, where `x` is
/// a zero-mean Ornstein-Uhlenbeck state and the deterministic shift is
/// (Brigo–Mercurio 2006, §3.3.1, eq. 3.36)
///
/// ```text
/// α(t) = f(0,t) + σ²/(2κ²)·(1 − e^{−κt})²
/// ```
///
/// with `f(0,t) = -d/dt ln P(0,t)` the market instantaneous forward. The
/// returned θ is piecewise-constant on the intervals between consecutive
/// `theta_times` and is chosen so the exact OU mean recursion
///
/// ```text
/// E[r(b)] = E[r(a)]·e^{−κ(b−a)} + θ·(1 − e^{−κ(b−a)})
/// ```
///
/// reproduces `α` at every boundary, starting from `r(0) = f(0,0)`:
///
/// ```text
/// θ_i = [α(t_{i+1}) − α(t_i)·e^{−κΔ_i}] / (1 − e^{−κΔ_i})
/// ```
///
/// Only forward *levels* enter, never the forward slope `∂f/∂t`. The
/// differential form `θ = f + (1/κ)·∂f/∂t + …` is exact for a smooth curve but
/// loses every forward jump of a log-linear or linear discount curve, whose
/// slope is zero between knots: the simulated rate then relaxes toward each
/// new forward at speed κ instead of following it.
///
/// The first interval starts at time zero whatever `theta_times[0]` is, and
/// the last θ is fitted over one more interval of the preceding width.
/// Simulation steps that end on a θ boundary carry the exact mean; a step
/// that ends inside an interval carries the exponential interpolation of
/// `α` between the surrounding boundaries.
///
/// For a flat curve at rate `r_flat`, `α(t) ≈ r_flat` and so is θ.
///
/// # Arguments
///
/// * `kappa` - Positive finite mean-reversion speed in inverse years.
/// * `sigma` - Positive finite short-rate volatility per square-root year.
/// * `discount_curve_fn` - Function mapping time in years to `P(0,t)`.
/// * `theta_times` - Strictly increasing non-negative θ(t) boundary times in
///   years; empty produces a single level equal to `f(0,0)`.
///
/// # Errors
///
/// Returns an error when the mean reversion, volatility, or theta schedule is invalid.
///
/// ```
/// use finstack_quant_models::monte_carlo::process::ou::calibrate_theta_from_curve;
///
/// let discount_fn = |t: f64| (-0.03 * t).exp();
/// let params = calibrate_theta_from_curve(0.03, 0.01, discount_fn, &[0.5, 1.0, 2.0, 5.0])?;
/// # let _ = params;
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
pub fn calibrate_theta_from_curve<F>(
    kappa: f64,
    sigma: f64,
    discount_curve_fn: F,
    theta_times: &[f64],
) -> Result<HullWhite1FParams>
where
    F: Fn(f64) -> f64,
{
    if theta_times.is_empty() {
        let f_0 = instantaneous_forward(&discount_curve_fn, 0.0);
        return HullWhite1FParams::new(kappa, sigma, f_0);
    }

    let model = HullWhiteParams::constant(kappa, sigma)?;
    let theta_curve = fit_theta_levels(kappa, theta_times, |t| {
        let decay = -(-kappa * t).exp_m1();
        Ok(instantaneous_forward(&discount_curve_fn, t)
            + sigma * sigma / (2.0 * kappa * kappa) * decay * decay)
    })?;
    HullWhite1FParams::from_parts(model, theta_curve, theta_times.to_vec())
}

/// Fit θ(t) to a discount curve under a piecewise-constant HW short-rate
/// volatility schedule.
///
/// This is [`calibrate_theta_from_curve`] with the deterministic shift
/// generalised to a volatility schedule:
///
/// ```text
/// α(t) = f(0,t) + (1/κ)·∫₀ᵗ σ(s)²·(e^{−κ(t−s)} − e^{−2κ(t−s)}) ds
/// ```
///
/// which reduces to `f(0,t) + σ²/(2κ²)·(1 − e^{−κt})²` for a constant σ.
///
/// `sigma_times` and `sigma_values` define the piecewise-constant annualized
/// short-rate-volatility curve. `theta_times` gives the year-fraction
/// boundaries of the returned piecewise-constant mean-reversion-level curve;
/// an empty slice produces one value at time zero. `discount_curve_fn(t)` must
/// return the discount factor `P(0,t)` on the same time basis. The forward
/// rate is obtained with a finite difference of width `1e-4` years.
///
/// If a sampled discount factor is non-positive, the finite-difference helper
/// logs a warning and substitutes a zero forward contribution rather than
/// returning an error. Production callers should therefore provide positive,
/// finite discount factors and treat warnings as a market-data failure.
///
/// # Arguments
///
/// * `kappa` - Positive Hull-White mean-reversion speed in inverse years.
/// * `sigma_times` - Increasing time boundaries in years for the piecewise
///   constant annualized short-rate-volatility schedule.
/// * `sigma_values` - Annualized short-rate volatilities aligned with
///   `sigma_times` according to [`PiecewiseConstantCurve`] semantics.
/// * `discount_curve_fn` - Function returning the discount factor `P(0,t)` for
///   a year-fraction input `t` on the same time basis as the schedules.
/// * `theta_times` - Requested theta-curve boundary times in years; empty
///   produces a single time-zero theta value.
///
/// # Errors
///
/// Returns validation errors for a non-positive or non-finite `kappa`, an
/// invalid volatility schedule, or an incompatible output theta schedule.
pub fn calibrate_theta_from_curve_with_piecewise_sigma<F>(
    kappa: f64,
    sigma_times: Vec<f64>,
    sigma_values: Vec<f64>,
    discount_curve_fn: F,
    theta_times: &[f64],
) -> Result<HullWhite1FParams>
where
    F: Fn(f64) -> f64,
{
    let sigma_curve = PiecewiseConstantCurve::new(sigma_times, sigma_values)?;
    if !kappa.is_finite() || kappa <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Hull-White kappa must be positive and finite, got {kappa}"
        )));
    }
    let theta_boundaries = if theta_times.is_empty() {
        vec![0.0]
    } else {
        theta_times.to_vec()
    };
    let theta_curve = fit_theta_levels(kappa, &theta_boundaries, |t| {
        // ∫σ²·e^{−κ(t−s)} ds is the variance kernel evaluated at κ/2.
        let single_decay = sigma_curve.integrate_squared_exp_weight(0.5 * kappa, t, 0.0, t)?;
        let double_decay = sigma_curve.integrate_squared_exp_weight(kappa, t, 0.0, t)?;
        Ok(instantaneous_forward(&discount_curve_fn, t) + (single_decay - double_decay) / kappa)
    })?;
    HullWhite1FParams::with_piecewise_sigma(
        kappa,
        sigma_curve.times().to_vec(),
        sigma_curve.values().to_vec(),
        theta_curve,
        theta_boundaries,
    )
}

/// Piecewise-constant θ levels whose exact OU mean equals `alpha` at every
/// interval boundary.
///
/// Interval `i` runs from boundary `i` to boundary `i + 1`. The first boundary
/// is time zero, because [`HullWhite1FParams::theta_at_time`] applies the
/// first level from the start of the simulation. The last level has no
/// following knot, so it is fitted over one more interval as wide as the
/// preceding one (one year when there is a single knot at zero).
fn fit_theta_levels(
    kappa: f64,
    theta_times: &[f64],
    alpha: impl Fn(f64) -> Result<f64>,
) -> Result<Vec<f64>> {
    let mut boundaries = Vec::with_capacity(theta_times.len() + 1);
    boundaries.push(0.0);
    boundaries.extend_from_slice(&theta_times[1..]);
    let last = boundaries[boundaries.len() - 1];
    let last_width = match boundaries.len() {
        1 if theta_times[0] > 0.0 => theta_times[0],
        1 => 1.0,
        n => last - boundaries[n - 2],
    };
    boundaries.push(last + last_width);

    let mut alpha_start = alpha(0.0)?;
    boundaries
        .windows(2)
        .map(|interval| {
            let width = interval[1] - interval[0];
            if width.is_nan() || width <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "Hull-White theta times must increase strictly".into(),
                ));
            }
            let alpha_end = alpha(interval[1])?;
            let decay = (-kappa * width).exp();
            let theta = (alpha_end - alpha_start * decay) / (1.0 - decay);
            alpha_start = alpha_end;
            Ok(theta)
        })
        .collect()
}

/// Instantaneous forward `f(0,t)` via [`fd_instantaneous_forward`], falling
/// back to `0.0` (with a warning) on a non-positive discount factor.
fn instantaneous_forward<F>(discount_curve_fn: &F, t: f64) -> f64
where
    F: Fn(f64) -> f64,
{
    fd_instantaneous_forward(discount_curve_fn, t).unwrap_or_else(|| {
        warn!(
            t,
            "instantaneous_forward: non-positive discount factor; returning 0.0. Check market data quality."
        );
        0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hw1f_params_constant_theta() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid parameters");

        assert_eq!(params.kappa, 0.1);
        assert_eq!(params.sigma_at_time(0.0), 0.01);
        assert_eq!(params.theta_at_time(0.0), 0.03);
        assert_eq!(params.theta_at_time(1.0), 0.03);
        assert_eq!(params.theta_at_time(10.0), 0.03);
    }

    #[test]
    fn test_hw1f_params_time_dependent_theta() {
        let theta_curve = vec![0.02, 0.03, 0.04];
        let theta_times = vec![0.0, 1.0, 2.0];

        let params =
            HullWhite1FParams::with_time_dependent_theta(0.1, 0.01, theta_curve, theta_times)
                .expect("valid theta schedule");

        assert_eq!(params.theta_at_time(0.0), 0.02);
        assert_eq!(params.theta_at_time(0.5), 0.02);
        assert_eq!(params.theta_at_time(1.0), 0.03);
        assert_eq!(params.theta_at_time(1.5), 0.03);
        assert_eq!(params.theta_at_time(2.0), 0.04);
        assert_eq!(params.theta_at_time(10.0), 0.04);
    }

    #[test]
    fn test_hw1f_drift() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid parameters");
        let process = HullWhite1FProcess::new(params);

        let x = vec![0.04]; // Rate above mean
        let mut drift = vec![0.0];

        process.drift(0.0, &x, &mut drift);

        assert!(drift[0] < 0.0);
        assert_eq!(drift[0], 0.1 * (0.03 - 0.04));
    }

    #[test]
    fn test_hw1f_diffusion() {
        let params = HullWhite1FParams::new(0.1, 0.01, 0.03).expect("valid parameters");
        let process = HullWhite1FProcess::new(params);

        let x = vec![0.05];
        let mut diffusion = vec![0.0];

        process.diffusion(0.0, &x, &mut diffusion);

        assert_eq!(diffusion[0], 0.01);
    }

    #[test]
    fn test_vasicek_alias() {
        let process =
            HullWhite1FProcess::vasicek(0.1, 0.03, 0.01).expect("valid Vasicek parameters");

        assert_eq!(process.params().kappa, 0.1);
        assert_eq!(process.params().sigma_at_time(0.0), 0.01);
        assert_eq!(process.theta_at_time(0.0), 0.03);
    }

    #[test]
    fn test_calibrate_theta_from_flat_curve() {
        // Flat 3% curve: P(0,t) = exp(-0.03 * t)
        let discount_fn = |t: f64| (-0.03 * t).exp();
        let theta_times = vec![0.0, 1.0, 2.0, 5.0, 10.0];

        let params = calibrate_theta_from_curve(0.03, 0.01, discount_fn, &theta_times)
            .expect("valid theta calibration");

        // For a flat curve, f(0,t) = 0.03 and ∂f/∂t ≈ 0
        // θ(t) ≈ κ·f + volatility term
        // At t=0, vol term = 0, so θ(0) ≈ κ·r = 0.03 * 0.03 = 0.0009
        // But we add the forward derivative which is near zero
        // Actually for flat curve: θ(t) = κ·f + σ²/(2κ²)·(1-e^{-κt})²

        assert_eq!(params.kappa, 0.03);
        assert_eq!(params.sigma_at_time(0.0), 0.01);
        assert_eq!(params.theta_times().len(), 5);

        for &theta in params.theta_values() {
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
        let params = calibrate_theta_from_curve(0.03, 0.01, discount_fn, &[])
            .expect("valid theta calibration");

        assert_eq!(params.theta_values().len(), 1);
        assert_eq!(params.theta_times().len(), 1);
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

    /// The fitted θ makes the exact OU mean equal the curve-implied shift
    /// `α(t) = f(0,t) + σ²/(2κ²)·(1 − e^{−κt})²` (Brigo & Mercurio 2006,
    /// eq. 3.36) at every θ boundary, including across forward jumps.
    ///
    /// The kinked curve has piecewise-flat forwards (2% → 4% → 3%), the shape
    /// a log-linear discount curve produces. A slope-based θ sees `∂f/∂t = 0`
    /// between knots and leaves the rate near 2%.
    #[test]
    fn fitted_theta_follows_the_curve_shift_across_forward_jumps() {
        use super::super::super::discretization::exact_hw1f::ExactHullWhite1F;
        use super::super::super::traits::Discretization;

        let kappa = 0.03_f64;
        let sigma = 0.01_f64;
        let flat = |t: f64| (-0.05 * t).exp();
        let kinked = |t: f64| {
            let integral =
                0.02 * t.min(1.0) + 0.04 * (t.min(3.0) - 1.0).max(0.0) + 0.03 * (t - 3.0).max(0.0);
            (-integral).exp()
        };
        let times: Vec<f64> = (0..60).map(|i| f64::from(i) / 12.0).collect();
        let shift = |discount: &dyn Fn(f64) -> f64, t: f64| {
            let decay = 1.0 - (-kappa * t).exp();
            instantaneous_forward(&discount, t)
                + sigma * sigma / (2.0 * kappa * kappa) * decay * decay
        };

        let curves: [&dyn Fn(f64) -> f64; 2] = [&flat, &kinked];
        for discount in curves {
            let params = calibrate_theta_from_curve(kappa, sigma, discount, &times)
                .expect("valid theta calibration");
            let process = HullWhite1FProcess::new(params);
            let disc = ExactHullWhite1F::new();
            let mut work = vec![0.0_f64; disc.work_size(&process)];
            let mut rate = vec![instantaneous_forward(&discount, 0.0)];
            for pair in times.windows(2) {
                disc.step(
                    &process,
                    pair[0],
                    pair[1] - pair[0],
                    &mut rate,
                    &[0.0],
                    &mut work,
                );
                let expected = shift(discount, pair[1]);
                assert!(
                    (rate[0] - expected).abs() < 1e-12,
                    "mean short rate {} at t={} should equal the curve shift {expected}",
                    rate[0],
                    pair[1],
                );
            }
        }

        // Mid-way through the 4% segment the mean rate has followed the jump.
        let params = calibrate_theta_from_curve(kappa, sigma, kinked, &times).expect("fit");
        let process = HullWhite1FProcess::new(params);
        let disc = ExactHullWhite1F::new();
        let mut work = vec![0.0_f64; disc.work_size(&process)];
        let mut rate = vec![0.02];
        for pair in times.windows(2).take(24) {
            disc.step(
                &process,
                pair[0],
                pair[1] - pair[0],
                &mut rate,
                &[0.0],
                &mut work,
            );
        }
        assert!(
            (rate[0] - 0.04).abs() < 1e-3,
            "mean short rate at t=2 should sit on the 4% forward, got {}",
            rate[0]
        );
    }

    // Hull-White drift initial-curve fit

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
        let params = calibrate_theta_from_curve(kappa, sigma, discount_fn, &times)
            .expect("valid theta calibration");
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
        let params = calibrate_theta_from_curve(kappa, sigma, discount_fn, &times)
            .expect("valid theta calibration");
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

    #[test]
    fn piecewise_sigma_uses_the_active_segment() {
        let params = HullWhite1FParams::with_piecewise_sigma(
            0.1,
            vec![0.0, 1.0],
            vec![0.01, 0.02],
            vec![0.03],
            vec![0.0],
        )
        .expect("valid schedule");

        assert_eq!(params.sigma_at_time(0.5), 0.01);
        assert_eq!(params.sigma_at_time(1.0), 0.02);
        assert_eq!(params.sigma_at_time(10.0), 0.02);
    }

    #[test]
    fn constant_piecewise_sigma_matches_scalar_theta_calibration() {
        let discount = |time: f64| (-0.03 * time).exp();
        let scalar = calibrate_theta_from_curve(0.05, 0.01, discount, &[0.5, 1.0, 2.0])
            .expect("scalar theta calibration");
        let scheduled = calibrate_theta_from_curve_with_piecewise_sigma(
            0.05,
            vec![0.0],
            vec![0.01],
            discount,
            &[0.5, 1.0, 2.0],
        )
        .expect("scheduled theta calibration");

        assert_eq!(scheduled.theta_times(), scalar.theta_times());
        for (actual, expected) in scheduled.theta_values().iter().zip(scalar.theta_values()) {
            assert!((actual - expected).abs() < 1.0e-12);
        }
    }
}
