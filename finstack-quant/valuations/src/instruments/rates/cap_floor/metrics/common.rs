//! Common utilities for interest rate option metrics.
//!
//! Provides a DRY aggregation helper to iterate caplets/floorlets and sum
//! contributions for a given functional form (e.g., delta/gamma/vega/theta).

use crate::instruments::rates::cap_floor::pricing::pricer::resolve_caplet_volatility;
use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_caplet_inputs;
use crate::instruments::rates::cap_floor::CapFloor;
use crate::metrics::MetricContext;
use finstack_quant_models::volatility::VolatilityConvention;

/// Per-caplet inputs passed to the aggregation closure.
///
/// `fixing_t` is the year fraction from `as_of` to the option's fixing date.
/// Fully fixed coupons are omitted because their stochastic Greeks are zero.
pub(crate) struct CapletInputs {
    /// Atomic forward rate for the accrual period.
    pub forward: f64,
    /// Resolved implied volatility (overrides → surface lookup).
    pub sigma: f64,
    /// Resolved model convention (including displacement).
    pub convention: VolatilityConvention,
    /// Strike after the same displacement as `forward`.
    pub strike: f64,
    /// Year fraction to the option fixing date.
    pub fixing_t: f64,
    /// Sensitivity of the optioned coupon to a parallel projected-forward shift.
    pub forward_sensitivity: f64,
    /// Second sensitivity to the same parallel projected-forward shift.
    pub forward_second_sensitivity: f64,
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
        let quote = resolve_caplet_volatility(option, context.curves.as_ref(), fixing_t, strike)?;
        let (forward, strike) = quote.model_rates(forward, strike)?;
        let per_unit = f(CapletInputs {
            forward,
            sigma: quote.sigma,
            convention: quote.convention,
            strike,
            fixing_t,
            forward_sensitivity: projection.parallel_forward_sensitivity,
            forward_second_sensitivity: projection.parallel_forward_second_sensitivity,
        });
        sum += per_unit * option.notional.amount() * projection.accrual_year_fraction * df;
    }
    Ok(sum)
}
