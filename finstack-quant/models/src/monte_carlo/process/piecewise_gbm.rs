//! Piecewise-constant GBM process and forward-vol/rate bootstrap shared by the
//! equity path-dependent Monte Carlo pricers (cliquet, autocallable).
//!
//! A single flat-volatility GBM misprices path-dependent equity payoffs whose
//! value depends on the volatility *term structure* — e.g. an autocallable's
//! knock-in put or a cliquet's per-period returns. This module bootstraps a
//! piecewise-constant forward volatility and forward (short) rate over the
//! product's observation/reset schedule from the discount curve and vol surface,
//! following a calendar-arbitrage-free total-variance bootstrap.

use crate::monte_carlo::paths::ProcessParams;
use crate::monte_carlo::traits::ProcessMetadata;
use crate::monte_carlo::traits::{Discretization, StochasticProcess};
use finstack_quant_core::Result;

/// Piecewise-constant GBM process.
///
/// The forward rate, dividend yield, and volatility are constant within each
/// interval `[times[i-1], times[i])` (with `times[-1] = 0`). This captures a
/// term structure of rates and volatility between observation/reset dates that
/// a single-parameter [`GbmProcess`](crate::monte_carlo::process::gbm::GbmProcess) cannot.
#[derive(Debug, Clone)]
pub struct PiecewiseGbmProcess {
    /// Interval end times (years from valuation), sorted ascending.
    pub times: Vec<f64>,
    /// Risk-free (forward) rate per interval.
    pub rs: Vec<f64>,
    /// Dividend yield per interval.
    pub qs: Vec<f64>,
    /// Volatility per interval.
    pub sigmas: Vec<f64>,
}

impl PiecewiseGbmProcess {
    /// Index of the interval whose parameters apply at time `t`.
    ///
    /// Intervals are half-open `[times[i-1], times[i])`, so an exact boundary
    /// hit `t == times[k]` belongs to the interval *starting* at `times[k]`
    /// (index `k+1`), not the one ending there. `partition_point(time <= t)`
    /// returns the first index with `times[idx] > t`, which is exactly that
    /// interval; the clamp keeps `t` at/after the final boundary in the last
    /// interval (flat extrapolation).
    #[inline]
    fn interval(&self, t: f64) -> usize {
        let idx = self.times.partition_point(|&time| time <= t);
        idx.min(self.times.len() - 1)
    }
}

impl StochasticProcess for PiecewiseGbmProcess {
    fn dim(&self) -> usize {
        1
    }

    fn num_factors(&self) -> usize {
        1
    }

    fn drift(&self, t: f64, x: &[f64], out: &mut [f64]) {
        let idx = self.interval(t);
        // μ(S) = (r - q) S
        out[0] = (self.rs[idx] - self.qs[idx]) * x[0];
    }

    fn diffusion(&self, t: f64, x: &[f64], out: &mut [f64]) {
        let idx = self.interval(t);
        // σ(S) = σ S
        out[0] = self.sigmas[idx] * x[0];
    }
}

impl ProcessMetadata for PiecewiseGbmProcess {
    fn metadata(&self) -> ProcessParams {
        let mut params = ProcessParams::new("PiecewiseGBM");
        // Report the first interval's parameters as representative metadata.
        if !self.rs.is_empty() {
            params.add_param("r_initial", self.rs[0]);
            params.add_param("q_initial", self.qs[0]);
            params.add_param("sigma_initial", self.sigmas[0]);
        }
        params.with_factors(vec!["spot".to_string()])
    }
}

/// Exact (log-Euler) discretization for [`PiecewiseGbmProcess`].
///
/// Within an interval the GBM step is exact:
/// `S(t+dt) = S(t)·exp((r − q − ½σ²)·dt + σ·√dt·Z)`.
#[derive(Debug, Clone, Default)]
pub struct PiecewiseExactGbm;

impl PiecewiseExactGbm {
    /// Create the exact per-interval log-Euler stepper; it carries no state.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Discretization<PiecewiseGbmProcess> for PiecewiseExactGbm {
    fn step(
        &self,
        process: &PiecewiseGbmProcess,
        t: f64,
        dt: f64,
        x: &mut [f64],
        z: &[f64],
        _work: &mut [f64],
    ) {
        let idx = process.interval(t);
        let r = process.rs[idx];
        let q = process.qs[idx];
        let sigma = process.sigmas[idx];

        let drift = (r - q - 0.5 * sigma * sigma) * dt;
        let diffusion = sigma * dt.sqrt() * z[0];
        x[0] *= (drift + diffusion).exp();
    }

    fn work_size(&self, _process: &PiecewiseGbmProcess) -> usize {
        0
    }
}

/// One checkpoint of the forward bootstrap: a model time and the discount
/// factor from the valuation date to it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GbmCheckpoint {
    /// Model time in years from the valuation date; strictly increasing across checkpoints.
    pub t: f64,
    /// Discount factor from the valuation date to `t`, positive and finite.
    pub df: f64,
}

/// Bootstrap a piecewise-constant GBM from discount factors and a volatility
/// term structure.
///
/// For each checkpoint the forward rate over the interval is implied from
/// consecutive discount factors, the ATM forward
/// `S₀ · exp(−q·t) / DF(t)` is handed to `sigma_at`, and the interval
/// volatility is the forward variance `σ(t)²·t − σ(t_prev)²·t_prev` over the
/// interval length, so a path-dependent payoff sees the term structure the
/// surface implies instead of one flat volatility.
///
/// # Arguments
///
/// * `check_points` - Strictly increasing model times with their discount
///   factors; a checkpoint whose time does not exceed the previous one is skipped.
/// * `initial_spot` - Spot at the valuation date.
/// * `div_yield` - Continuous dividend yield as a decimal, applied on every interval.
/// * `sigma_at` - Total implied volatility (decimal, annualised) at a model
///   time and ATM forward price; typically a surface lookup.
/// * `context_label` - Caller name used in error messages.
///
/// # Errors
///
/// Returns `Error::Validation` when a discount factor is non-positive or
/// non-finite, when two checkpoints are closer than `1e-6` years, or when the
/// total variance decreases between checkpoints; propagates `sigma_at` errors.
pub fn bootstrap_forward_gbm(
    check_points: &[GbmCheckpoint],
    initial_spot: f64,
    div_yield: f64,
    mut sigma_at: impl FnMut(f64, f64) -> Result<f64>,
    context_label: &str,
) -> Result<PiecewiseGbmProcess> {
    let mut times = Vec::new();
    let mut rs = Vec::new();
    let mut qs = Vec::new();
    let mut sigmas = Vec::new();

    let mut prev_t = 0.0;
    let mut prev_var = 0.0;
    let mut df_prev = 1.0;

    for &GbmCheckpoint {
        t: curr_t,
        df: df_curr,
    } in check_points
    {
        if curr_t <= prev_t {
            continue;
        }
        let dt = curr_t - prev_t;

        if df_curr <= 0.0 || !df_curr.is_finite() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context_label}: discount factor at t={curr_t} is non-positive \
                 ({df_curr}); the curve is degenerate or extrapolated past its valid range"
            )));
        }
        if dt <= 1e-6 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context_label}: degenerate time step dt={dt} between periods at t_prev={prev_t} \
                 and t_curr={curr_t}; check that schedule dates are distinct"
            )));
        }

        // Forward rate over [prev_t, curr_t] on the model clock.
        let fwd_r = (df_prev / df_curr).ln() / dt;

        // ATM forward for the surface lookup: F(0, t) = S_0 · exp(-q·t) / DF(t).
        let forward_price = initial_spot * (-div_yield * curr_t).exp() / df_curr;
        let vol_curr = sigma_at(curr_t, forward_price)?;
        let var_curr = vol_curr * vol_curr * curr_t;

        let fwd_var = var_curr - prev_var;
        if fwd_var < 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{context_label}: total variance is non-monotone over \
                 [{prev_t}, {curr_t}]: previous={prev_var}, current={var_curr}, increment={fwd_var}"
            )));
        }
        let fwd_sigma = (fwd_var / dt).sqrt();

        times.push(curr_t);
        rs.push(fwd_r);
        qs.push(div_yield);
        sigmas.push(fwd_sigma);

        prev_t = curr_t;
        prev_var = var_curr;
        df_prev = df_curr;
    }

    Ok(PiecewiseGbmProcess {
        times,
        rs,
        qs,
        sigmas,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_interval_process() -> PiecewiseGbmProcess {
        PiecewiseGbmProcess {
            times: vec![1.0, 2.0, 3.0],
            rs: vec![0.01, 0.02, 0.03],
            qs: vec![0.0, 0.0, 0.0],
            sigmas: vec![0.10, 0.20, 0.30],
        }
    }

    /// Interval convention is half-open `[times[i-1], times[i])`: an exact
    /// boundary hit must select the interval STARTING at that boundary.
    #[test]
    fn interval_boundary_belongs_to_next_interval() {
        let p = three_interval_process();

        // Interior points.
        assert_eq!(p.interval(0.0), 0);
        assert_eq!(p.interval(0.5), 0);
        assert_eq!(p.interval(1.5), 1);
        assert_eq!(p.interval(2.5), 2);

        // Exact boundaries: t == times[k] starts interval k+1.
        assert_eq!(p.interval(1.0), 1, "t=1.0 is the start of [1,2)");
        assert_eq!(p.interval(2.0), 2, "t=2.0 is the start of [2,3)");

        // Final boundary and beyond clamp to the last interval.
        assert_eq!(p.interval(3.0), 2);
        assert_eq!(p.interval(10.0), 2);
    }

    /// The discretization step at an exact boundary must use the parameters of
    /// the interval starting there (e.g. σ of [1,2), not σ of [0,1)).
    #[test]
    fn step_at_boundary_uses_next_interval_params() {
        let p = three_interval_process();
        let disc = PiecewiseExactGbm::new();

        let dt = 0.25;
        let z = [0.0_f64];
        let mut work: [f64; 0] = [];

        // Drift-only step (z = 0) starting exactly at t = 1.0 must use
        // r = 0.02, sigma = 0.20 (interval [1, 2)).
        let mut x = [100.0_f64];
        disc.step(&p, 1.0, dt, &mut x, &z, &mut work);
        let expected = 100.0 * ((0.02 - 0.5 * 0.20 * 0.20) * dt).exp();
        assert!(
            (x[0] - expected).abs() < 1e-12,
            "boundary step must use interval-[1,2) params: got {}, expected {expected}",
            x[0]
        );
    }
}
