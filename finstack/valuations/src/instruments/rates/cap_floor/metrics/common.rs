//! Common utilities for interest rate option metrics.
//!
//! Provides a DRY aggregation helper to iterate caplets/floorlets and sum
//! contributions for a given functional form (e.g., delta/gamma/vega/theta).

use crate::instruments::common_impl::vol_resolution::resolve_sigma_at;
use crate::instruments::rates::cap_floor::pricing::{black, normal};
use crate::instruments::rates::cap_floor::CapFloor;
use crate::instruments::rates::swaption::types::lognormal_to_normal_vol;
use crate::metrics::MetricContext;

const MIN_EFFECTIVE_FIXING_TIME: f64 = 1e-6;

/// Per-caplet inputs passed to the aggregation closure.
///
/// `fixing_t` is the year fraction from `as_of` to the option's fixing date
/// (floored at `MIN_EFFECTIVE_FIXING_TIME`) — the same time the pricer uses for
/// both the vol-surface lookup and the model `T`. Greeks use this single time so
/// they remain consistent with the reported price (a finite-difference Greek
/// reconciles with the analytic one).
pub(crate) struct CapletInputs {
    /// Atomic forward rate for the accrual period.
    pub forward: f64,
    /// Resolved implied volatility (overrides → surface lookup).
    pub sigma: f64,
    /// Year fraction to the option fixing date (clamped above `MIN_EFFECTIVE_FIXING_TIME`).
    pub fixing_t: f64,
}

/// Lognormal-convention forward delta with graceful Bachelier fallback.
///
/// Mirrors the pricer's `Lognormal`/`Auto` path: uses Black-76 where the model
/// is well-defined (`forward > 0` and `strike > 0`); otherwise converts the
/// lognormal vol to an equivalent normal vol and uses Bachelier so the Greek
/// stays finite and consistent with the price. Shared by the `Lognormal` and
/// `Auto` vol types.
pub(crate) fn lognormal_delta_with_fallback(
    is_cap: bool,
    strike: f64,
    forward: f64,
    sigma: f64,
    t: f64,
) -> f64 {
    if forward > 0.0 && strike > 0.0 {
        black::delta(is_cap, strike, forward, sigma, t)
    } else {
        let normal_vol = lognormal_to_normal_vol(sigma, forward, strike, t, None);
        normal::delta(is_cap, strike, forward, normal_vol, t)
    }
}

/// Lognormal-convention forward gamma with graceful Bachelier fallback.
///
/// See [`lognormal_delta_with_fallback`] for the model-selection rationale.
pub(crate) fn lognormal_gamma_with_fallback(strike: f64, forward: f64, sigma: f64, t: f64) -> f64 {
    if forward > 0.0 && strike > 0.0 {
        black::gamma(strike, forward, sigma, t)
    } else {
        let normal_vol = lognormal_to_normal_vol(sigma, forward, strike, t, None);
        normal::gamma(strike, forward, normal_vol, t)
    }
}

/// Lognormal-convention vega (per 1% vol) with graceful Bachelier fallback.
///
/// See [`lognormal_delta_with_fallback`] for the model-selection rationale.
pub(crate) fn lognormal_vega_with_fallback(strike: f64, forward: f64, sigma: f64, t: f64) -> f64 {
    if forward > 0.0 && strike > 0.0 {
        black::vega_per_pct(strike, forward, sigma, t)
    } else {
        let normal_vol = lognormal_to_normal_vol(sigma, forward, strike, t, None);
        normal::vega_per_pct(strike, forward, normal_vol, t)
    }
}

/// Iterate over caplets/floorlets and aggregate contributions.
///
/// The supplied function `f` receives a [`CapletInputs`] payload for each
/// non-expired period and returns the "per-unit" measure for that caplet.
/// The helper scales by `notional × accrual_year_fraction × discount_factor`
/// and sums across periods.
///
/// The fixing date used for vol surface lookup and `fixing_t` computation is
/// the `reset_date` when provided (matching the pricer), falling back to
/// `accrual_start`. This ensures Greeks are computed on the same dates as
/// pricing, which matters for indices with non-zero reset lags (e.g., SOFR
/// 2-day lookback).
pub(crate) fn aggregate_over_caplets<FN>(
    option: &CapFloor,
    context: &MetricContext,
    mut f: FN,
) -> finstack_core::Result<f64>
where
    FN: FnMut(CapletInputs) -> f64,
{
    let disc_curve = context
        .curves
        .get_discount(option.discount_curve_id.as_ref())?;
    let fwd_curve = context
        .curves
        .get_forward(option.forward_curve_id.as_ref())?;
    let strike = option.strike_f64()?;
    let dc_ctx = finstack_core::dates::DayCountContext::default();

    let periods = option.pricing_periods()?;
    if periods.is_empty() {
        return Ok(0.0);
    }

    let mut sum = 0.0;
    for period in &periods {
        let fixing_date = option.option_fixing_date(period);
        if fixing_date < context.as_of {
            continue;
        }

        let fixing_t = option
            .day_count
            .year_fraction(context.as_of, fixing_date, dc_ctx)?
            .max(MIN_EFFECTIVE_FIXING_TIME);

        let forward = crate::instruments::common_impl::pricing::time::rate_period_on_dates(
            fwd_curve.as_ref(),
            period.accrual_start,
            period.accrual_end,
        )?;
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc_curve.as_ref(),
            context.as_of,
            period.payment_date,
        )?;
        let sigma = resolve_sigma_at(
            &option.pricing_overrides.market_quotes,
            context.curves.as_ref(),
            option.vol_surface_id.as_str(),
            fixing_t,
            strike,
        )?;

        let per_unit = f(CapletInputs {
            forward,
            sigma,
            fixing_t,
        });
        sum += per_unit * option.notional.amount() * period.accrual_year_fraction * df;
    }
    Ok(sum)
}
