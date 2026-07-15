//! Common utilities for interest rate option metrics.
//!
//! Provides a DRY aggregation helper to iterate caplets/floorlets and sum
//! contributions for a given functional form (e.g., delta/gamma/vega/theta).

use crate::instruments::common_impl::vol_resolution::resolve_sigma_at;
use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_caplet_inputs;
use crate::instruments::rates::cap_floor::pricing::{black, normal};
use crate::instruments::rates::cap_floor::CapFloor;
use crate::instruments::rates::swaption::types::{
    lognormal_to_normal_vol, lognormal_to_normal_vol_jacobian,
};
use crate::metrics::MetricContext;

/// Per-caplet inputs passed to the aggregation closure.
///
/// `fixing_t` is the year fraction from `as_of` to the option's fixing date.
/// Fully fixed coupons are omitted because their stochastic Greeks are zero.
pub(crate) struct CapletInputs {
    /// Atomic forward rate for the accrual period.
    pub forward: f64,
    /// Resolved implied volatility (overrides → surface lookup).
    pub sigma: f64,
    /// Year fraction to the option fixing date.
    pub fixing_t: f64,
    /// Sensitivity of the optioned coupon to a parallel projected-forward shift.
    pub forward_sensitivity: f64,
    /// Second sensitivity to the same parallel projected-forward shift.
    pub forward_second_sensitivity: f64,
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
        let jacobian = lognormal_to_normal_vol_jacobian(sigma, forward, strike, t, None);
        normal::vega_per_pct(strike, forward, normal_vol, t) * jacobian
    }
}

/// Iterate over caplets/floorlets and aggregate contributions.
///
/// The supplied function `f` receives a [`CapletInputs`] payload for each
/// non-expired period and returns the "per-unit" measure for that caplet.
/// The helper scales by `notional × accrual_year_fraction × discount_factor`
/// and sums across periods.
///
/// Coupon, fixing, and payment inputs come from the same canonical projection
/// used by pricing and implied-volatility inversion. For term indices this uses
/// the reset date; for compounded overnight coupons it uses the last distinct
/// contractual observation after lookback, observation shift, or cutoff.
pub(crate) fn aggregate_over_caplets<FN>(
    option: &CapFloor,
    context: &MetricContext,
    mut f: FN,
) -> finstack_quant_core::Result<f64>
where
    FN: FnMut(CapletInputs) -> f64,
{
    let strike = option.strike_f64()?;

    let periods = option.pricing_periods()?;
    if periods.is_empty() {
        return Ok(0.0);
    }

    let mut sum = 0.0;
    for period in &periods {
        if period.payment_date <= context.as_of {
            continue;
        }
        let resolved_inputs =
            resolve_optioned_caplet_inputs(option, period, context.curves.as_ref(), context.as_of)?;
        let projection = &resolved_inputs.coupon;
        if projection.payment_date <= context.as_of {
            continue;
        }
        let fixing_date = projection.fixing_date;
        if fixing_date <= context.as_of {
            continue;
        }

        let fixing_t = resolved_inputs.time_to_fixing;

        let forward = projection.forward;
        let df = resolved_inputs.discount_factor;
        let sigma = resolve_sigma_at(
            &option.instrument_pricing_overrides.market_quotes,
            context.curves.as_ref(),
            option.vol_surface_id.as_str(),
            fixing_t,
            strike,
        )?;

        let per_unit = f(CapletInputs {
            forward,
            sigma,
            fixing_t,
            forward_sensitivity: projection.parallel_forward_sensitivity,
            forward_second_sensitivity: projection.parallel_forward_second_sensitivity,
        });
        sum += per_unit * option.notional.amount() * projection.accrual_year_fraction * df;
    }
    Ok(sum)
}
