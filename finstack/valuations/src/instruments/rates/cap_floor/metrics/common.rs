//! Common utilities for interest rate option metrics.
//!
//! Provides a DRY aggregation helper to iterate caplets/floorlets and sum
//! contributions for a given functional form (e.g., delta/gamma/vega/theta).

use crate::instruments::common_impl::vol_resolution::resolve_sigma_at;
use crate::instruments::rates::cap_floor::CapFloor;
use crate::metrics::MetricContext;

const MIN_EFFECTIVE_FIXING_TIME: f64 = 1e-6;

/// Per-caplet inputs passed to the aggregation closure.
///
/// `fixing_t` is the year fraction from `as_of` to the option's fixing date
/// (floored at `MIN_EFFECTIVE_FIXING_TIME`). `risk_t` is the same time for
/// non-RFR options, and the observation-window midpoint for RFR options —
/// useful when a Greek formula's time argument needs to reflect the actual
/// rate-observation window (e.g., vega) rather than the option's fixing date.
pub(crate) struct CapletInputs {
    /// Atomic forward rate for the accrual period.
    pub forward: f64,
    /// Resolved implied volatility (overrides → surface lookup).
    pub sigma: f64,
    /// Year fraction to the option fixing date (clamped above `MIN_EFFECTIVE_FIXING_TIME`).
    pub fixing_t: f64,
    /// Risk-time year fraction. Equal to `fixing_t` except for RFR options,
    /// where it is the observation-window midpoint.
    pub risk_t: f64,
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
    let use_rfr = option.uses_overnight_rfr_index();

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
        let risk_t = if use_rfr {
            rfr_observation_midpoint_time(option, context.as_of, period, dc_ctx)?
        } else {
            fixing_t
        };

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
            risk_t,
        });
        sum += per_unit * option.notional.amount() * period.accrual_year_fraction * df;
    }
    Ok(sum)
}

/// Year fraction from `as_of` to the midpoint of an RFR option's observation
/// window. Clamped above `MIN_EFFECTIVE_FIXING_TIME` to avoid degeneracies at
/// the front stub.
pub(crate) fn rfr_observation_midpoint_time(
    option: &CapFloor,
    as_of: finstack_core::dates::Date,
    period: &crate::cashflow::builder::periods::SchedulePeriod,
    dc_ctx: finstack_core::dates::DayCountContext,
) -> finstack_core::Result<f64> {
    let observation_start = if period.accrual_start > as_of {
        period.accrual_start
    } else {
        as_of
    };
    let t_start = option
        .day_count
        .year_fraction(as_of, observation_start, dc_ctx)?
        .max(0.0);
    let t_end = option
        .day_count
        .year_fraction(as_of, period.accrual_end, dc_ctx)?
        .max(0.0);
    Ok(((t_start + t_end) * 0.5).max(MIN_EFFECTIVE_FIXING_TIME))
}
