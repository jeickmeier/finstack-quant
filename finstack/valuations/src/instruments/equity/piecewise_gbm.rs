//! Piecewise-constant GBM process and forward-vol/rate bootstrap shared by the
//! equity path-dependent Monte Carlo pricers (cliquet, autocallable).
//!
//! A single flat-volatility GBM misprices path-dependent equity payoffs whose
//! value depends on the volatility *term structure* — e.g. an autocallable's
//! knock-in put or a cliquet's per-period returns. This module bootstraps a
//! piecewise-constant forward volatility and forward (short) rate over the
//! product's observation/reset schedule from the discount curve and vol surface,
//! following a calendar-arbitrage-free total-variance bootstrap.

use crate::instruments::common_impl::vol_resolution::resolve_sigma_at;
use crate::instruments::pricing_overrides::MarketQuoteOverrides;
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::Result;
use finstack_monte_carlo::paths::ProcessParams;
use finstack_monte_carlo::process::metadata::ProcessMetadata;
use finstack_monte_carlo::traits::{Discretization, StochasticProcess};

/// Piecewise-constant GBM process.
///
/// The forward rate, dividend yield, and volatility are constant within each
/// interval `[times[i-1], times[i])` (with `times[-1] = 0`). This captures a
/// term structure of rates and volatility between observation/reset dates that
/// a single-parameter [`finstack_monte_carlo`] GBM cannot.
#[derive(Debug, Clone)]
pub(crate) struct PiecewiseGbmProcess {
    /// Interval end times (years from valuation), sorted ascending.
    pub(crate) times: Vec<f64>,
    /// Risk-free (forward) rate per interval.
    pub(crate) rs: Vec<f64>,
    /// Dividend yield per interval.
    pub(crate) qs: Vec<f64>,
    /// Volatility per interval.
    pub(crate) sigmas: Vec<f64>,
}

impl PiecewiseGbmProcess {
    /// Index of the interval whose parameters apply at time `t`.
    #[inline]
    fn interval(&self, t: f64) -> usize {
        let idx = self.times.partition_point(|&time| time < t);
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
pub(crate) struct PiecewiseExactGbm;

impl PiecewiseExactGbm {
    pub(crate) fn new() -> Self {
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

/// Bootstrap a piecewise-constant forward GBM from the discount curve and vol
/// surface over the supplied `check_points` (period-boundary times in years from
/// `as_of`, strictly positive and sorted ascending; the caller must include the
/// final maturity so the process covers the whole horizon).
///
/// For each interval `[prev_t, curr_t]`:
/// - **forward rate** `f = ln(DF(prev_t) / DF(curr_t)) / dt`;
/// - **forward volatility** from the total-variance increment
///   `σ²(curr_t)·curr_t − σ²(prev_t)·prev_t` (the surface is sampled at the ATM
///   forward `F(0, curr_t)`). A non-monotone (calendar-arbitrageable) surface can
///   produce a negative forward variance; it is floored at zero with a warning.
///
/// # Errors
///
/// Returns an error if a discount factor is non-positive/non-finite (degenerate
/// or over-extrapolated curve) or if two check points coincide.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bootstrap_forward_gbm(
    disc_curve: &DiscountCurve,
    curves: &MarketContext,
    market_quotes: &MarketQuoteOverrides,
    vol_surface_id: &str,
    as_of: Date,
    initial_spot: f64,
    div_yield: f64,
    check_points: &[f64],
    context_label: &str,
) -> Result<PiecewiseGbmProcess> {
    let mut times = Vec::new();
    let mut rs = Vec::new();
    let mut qs = Vec::new();
    let mut sigmas = Vec::new();

    let mut prev_t = 0.0;
    let mut prev_var = 0.0;

    // `disc_curve.df(t)` takes a curve-time relative to the curve base date, so
    // shift period offsets by year_fraction(base_date, as_of).
    let t_base_to_as_of = disc_curve.day_count().year_fraction(
        disc_curve.base_date(),
        as_of,
        DayCountContext::default(),
    )?;
    let df_base_to_as_of = disc_curve.df(t_base_to_as_of);

    for &curr_t in check_points {
        if curr_t <= prev_t {
            continue;
        }

        let df_prev = disc_curve.df(t_base_to_as_of + prev_t);
        let df_curr = disc_curve.df(t_base_to_as_of + curr_t);
        let dt = curr_t - prev_t;

        if df_curr <= 0.0 || !df_curr.is_finite() {
            return Err(finstack_core::Error::Validation(format!(
                "{context_label}: discount factor at t={curr_t} is non-positive ({df_curr}); \
                 the curve is degenerate or extrapolated past its valid range"
            )));
        }
        if dt <= 1e-6 {
            return Err(finstack_core::Error::Validation(format!(
                "{context_label}: degenerate time step dt={dt} between periods at t_prev={prev_t} \
                 and t_curr={curr_t}; check that schedule dates are distinct"
            )));
        }

        // Forward rate over [prev_t, curr_t].
        let fwd_r = (df_prev / df_curr).ln() / dt;

        // ATM forward for the surface lookup:
        //   F(0, curr_t) = S_0 · exp(-q·curr_t) / DF(as_of, curr_t),
        //   DF(as_of, curr_t) = df_curr / df_base_to_as_of.
        let forward_price = initial_spot * (-div_yield * curr_t).exp() / df_curr * df_base_to_as_of;

        let vol_curr =
            resolve_sigma_at(market_quotes, curves, vol_surface_id, curr_t, forward_price)?;
        let var_curr = vol_curr * vol_curr * curr_t;

        let fwd_var = var_curr - prev_var;
        let fwd_sigma = if fwd_var >= 0.0 {
            (fwd_var / dt).sqrt()
        } else {
            tracing::warn!(
                context = %context_label,
                surface_id = %vol_surface_id,
                t_prev = prev_t,
                t_curr = curr_t,
                total_var_prev = prev_var,
                total_var_curr = var_curr,
                forward_variance = fwd_var,
                "forward-vol bootstrap: total-variance surface is non-monotone over \
                 [t_prev, t_curr] (calendar-spread arbitrage); flooring forward variance to zero"
            );
            0.0
        };

        times.push(curr_t);
        rs.push(fwd_r);
        qs.push(div_yield);
        sigmas.push(fwd_sigma);

        prev_t = curr_t;
        prev_var = var_curr;
    }

    Ok(PiecewiseGbmProcess {
        times,
        rs,
        qs,
        sigmas,
    })
}
