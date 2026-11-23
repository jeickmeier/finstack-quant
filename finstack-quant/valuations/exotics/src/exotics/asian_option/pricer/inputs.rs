//! Shared fixing-schedule grids, carry schedules, market-input extraction, and
//! expired-option payoff helpers for Asian option pricers.

use crate::exotics::asian_option::types::{AsianOption, average_intrinsic, realized_average};
use crate::exotics::bs_gbm::BlackScholesInputsDf;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::traits::Discounting;
#[cfg(test)]
use finstack_quant_core::math::stats::OnlineCovariance;
use finstack_quant_core::money::Money;
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{PricingError, PricingErrorContext};
use finstack_quant_valuations_core::results::ValuationResult;

/// Exact simulation grid and one event index for every future fixing.
/// Repeated indices retain contractual observations sharing a model time.
pub(in crate::exotics::asian_option) struct FixingGrid {
    pub(in crate::exotics::asian_option) time_grid: finstack_quant_core::math::time_grid::TimeGrid,
    pub(in crate::exotics::asian_option) fixing_steps: Vec<usize>,
}

impl FixingGrid {
    pub(super) fn future_times(&self) -> Vec<f64> {
        self.fixing_steps
            .iter()
            .map(|&step| self.time_grid.times()[step])
            .collect()
    }
}

/// Insert contractual fixing times into the simulation grid, retaining their
/// multiplicity when the instrument's day count maps dates to the same time.
pub(in crate::exotics::asian_option) fn map_fixings_to_steps(
    fixing_dates: &[Date],
    day_count: finstack_quant_core::dates::DayCount,
    as_of: Date,
    t: f64,
    base_num_steps: usize,
) -> finstack_quant_core::Result<FixingGrid> {
    let fixing_times = fixing_dates
        .iter()
        .filter(|&&date| date > as_of)
        .map(|&date| day_count.year_fraction(as_of, date, DayCountContext::default()))
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    let time_grid = finstack_quant_core::math::time_grid::TimeGrid::uniform_with_required_times(
        t,
        base_num_steps.max(1),
        &fixing_times,
    )?;
    let fixing_steps = fixing_times
        .iter()
        .map(|&time| time_grid.get_index(time))
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    Ok(FixingGrid {
        time_grid,
        fixing_steps,
    })
}

/// Welford moments of the paired per-path payoffs `(xs[i], ys[i])`.
#[cfg(test)]
pub(super) fn paired_moments(xs: &[f64], ys: &[f64]) -> OnlineCovariance {
    let mut moments = OnlineCovariance::new();
    for (&x, &y) in xs.iter().zip(ys) {
        moments.update(x, y).expect("finite paired payoff");
    }
    moments
}

/// Cumulative asset carry at each contractual date, in the instrument's clock.
/// The same schedule drives the simulation and geometric log moments.
pub(super) fn asian_drift_and_fixing_multipliers(
    asian: &AsianOption,
    market: &MarketContext,
    as_of: Date,
    q: f64,
) -> finstack_quant_core::Result<(
    finstack_quant_models::monte_carlo::process::gbm::DriftSchedule,
    Vec<f64>,
)> {
    let discount = market.get_discount(asian.discount_curve_id.as_str())?;
    let mut dates: Vec<Date> = asian
        .fixing_dates
        .iter()
        .copied()
        .filter(|&date| date > as_of)
        .collect();
    dates.push(asian.expiry);
    dates.sort_unstable();
    dates.dedup();
    let mut times = vec![0.0];
    let mut cumulative = vec![0.0];
    for date in dates {
        let time = asian
            .day_count
            .year_fraction(as_of, date, DayCountContext::default())?;
        let carry = -discount.df_between_dates(as_of, date)?.ln() - q * time;
        let last = times.len() - 1;
        if time.total_cmp(&times[last]) != std::cmp::Ordering::Equal {
            times.push(time);
            cumulative.push(carry);
        }
    }
    let drift =
        finstack_quant_models::monte_carlo::process::gbm::DriftSchedule::new(times, cumulative)?;
    let multipliers = asian
        .fixing_dates
        .iter()
        .filter(|&&date| date > as_of)
        .map(|&date| {
            let time = asian
                .day_count
                .year_fraction(as_of, date, DayCountContext::default())?;
            let carry = -discount.df_between_dates(as_of, date)?.ln() - q * time;
            Ok((carry - drift.cumulative(time)).exp())
        })
        .collect::<finstack_quant_core::Result<Vec<_>>>()?;
    Ok((drift, multipliers))
}

/// Value of an expired Asian option: the intrinsic of the realized average
/// (current spot when nothing was recorded) times quantity, undiscounted.
///
/// Shared by the GBM and Heston Monte Carlo pricers.
pub(in crate::exotics::asian_option) fn expired_value(
    inst: &AsianOption,
    curves: &MarketContext,
    hist_sum: f64,
    hist_prod_log: f64,
    hist_count: usize,
) -> finstack_quant_core::Result<Money> {
    expired_value_for_method(
        inst,
        curves,
        inst.averaging_method,
        hist_sum,
        hist_prod_log,
        hist_count,
    )
}

fn expired_value_for_method(
    inst: &AsianOption,
    curves: &MarketContext,
    method: AveragingMethod,
    hist_sum: f64,
    hist_prod_log: f64,
    hist_count: usize,
) -> finstack_quant_core::Result<Money> {
    let average = match realized_average(method, hist_sum, hist_prod_log, hist_count) {
        Some(average) => average,
        None => resolve_spot(inst, curves)?,
    };
    let intrinsic = average_intrinsic(inst.option_type, average, inst.strike);
    Money::new(intrinsic * inst.quantity, inst.currency)
}

pub(super) fn expired_valuation(
    asian: &AsianOption,
    market: &MarketContext,
    as_of: Date,
    method: AveragingMethod,
    hist_sum: f64,
    hist_prod_log: f64,
    hist_count: usize,
) -> std::result::Result<ValuationResult, PricingError> {
    let money =
        expired_value_for_method(asian, market, method, hist_sum, hist_prod_log, hist_count)
            .map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(asian))
            })?;
    Ok(ValuationResult::stamped(asian.id(), as_of, money))
}

pub(super) fn expiry_year_fraction_or_zero(
    asian: &AsianOption,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of >= asian.expiry {
        Ok(0.0)
    } else {
        asian
            .day_count
            .year_fraction(as_of, asian.expiry, DayCountContext::default())
    }
}

/// Underlying spot from `spot_id`, in the option currency when quoted as a price.
pub(crate) fn resolve_spot(
    inst: &AsianOption,
    curves: &MarketContext,
) -> finstack_quant_core::Result<f64> {
    finstack_quant_valuations_core::instruments::pricing::market_inputs::scalar_price_amount(
        curves.get_price(&inst.spot_id)?,
        inst.currency,
    )
}

/// Inputs of the analytic Asian pricers: the dated discount factor to expiry,
/// spot, dividend yield, and σ at the strike (implied-vol override, else the
/// vol source) on the instrument day count, which is also the returned `t`.
pub(super) fn analytic_inputs(
    inst: &AsianOption,
    market: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<BlackScholesInputsDf> {
    let disc_curve = market.get_discount(inst.discount_curve_id.as_str())?;
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;
    let df = disc_curve.df_between_dates(as_of, inst.expiry)?;
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Invalid discount factor ({df:.6e}) between {as_of} and {}",
            inst.expiry
        )));
    }
    let spot = resolve_spot(inst, market)?;
    let q = finstack_quant_valuations_core::instruments::pricing::market_inputs::resolve_optional_dividend_yield(
        market,
        inst.div_yield_id.as_ref(),
    )?;
    let sigma = finstack_quant_valuations_core::instruments::pricing::volatility::resolve_sigma_at(
        &inst.instrument_pricing_overrides.market_quotes,
        market,
        inst.vol_surface_id.as_str(),
        t,
        inst.strike,
    )?;
    Ok(BlackScholesInputsDf {
        spot,
        df,
        q,
        sigma,
        t,
    })
}

/// Year fractions (instrument day count) from `as_of` to each strictly-future
/// fixing date, sorted ascending (fixing dates are stored sorted).
pub(super) fn fixing_year_fractions(
    asian: &AsianOption,
    as_of: Date,
) -> finstack_quant_core::Result<Vec<f64>> {
    let mut times = Vec::with_capacity(asian.fixing_dates.len());
    for date in &asian.fixing_dates {
        if *date > as_of {
            times.push(
                asian
                    .day_count
                    .year_fraction(as_of, *date, DayCountContext::default())?,
            );
        }
    }
    Ok(times)
}
