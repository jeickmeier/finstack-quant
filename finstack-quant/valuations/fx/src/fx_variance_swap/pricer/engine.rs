//! FX variance swap pricing and observation engine.

use crate::fx_variance_swap::FxVarianceSwap;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::{
    Result, dates::Date, market_data::context::MarketContext, math::stats::realized_variance,
    money::Money,
};
use finstack_quant_valuations_core::instruments::pricing::variance_observations::preceding_close;

use super::replication;

pub(super) type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);

pub(crate) fn compute_pv(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    let settlement_date = inst.effective_settlement_date()?;
    if as_of > settlement_date {
        return Ok(Money::from((0_i64, inst.notional.currency())));
    }
    inst.validate_as_of(context, as_of)?;

    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;

    // Compute observation dates once per pricing call. Each branch below would
    // otherwise rebuild this 1-3 times via the helper functions.
    let obs_dates = observation_dates(inst)?;
    let expected_var = expected_variance_with_dates(inst, context, as_of, &obs_dates)?;
    let df = dom.df_between_dates(as_of, settlement_date)?;
    inst.payoff(expected_var)?.checked_mul_f64(df)
}

pub(crate) fn observation_dates(inst: &FxVarianceSwap) -> Result<Vec<Date>> {
    finstack_quant_valuations_core::instruments::pricing::variance_observations::variance_observation_dates(
        inst.start_date,
        inst.maturity,
        inst.observation_frequency,
        inst.observation_business_day_convention,
        inst.observation_end_of_month,
        finstack_quant_valuations_core::instruments::pricing::variance_observations::VarianceCalendar::Joint {
            base: &inst.base_calendar_id,
            quote: &inst.quote_calendar_id,
        },
    )
}

pub(crate) fn annualization_factor(inst: &FxVarianceSwap) -> f64 {
    finstack_quant_valuations_core::instruments::pricing::variance_observations::annualization_factor(
        inst.observation_frequency,
        inst.trading_days_per_year,
    )
}

pub(crate) fn realized_fraction_by_observations(inst: &FxVarianceSwap, as_of: Date) -> Result<f64> {
    finstack_quant_valuations_core::instruments::pricing::variance_observations::realized_sample_fraction(
        &observation_dates(inst)?,
        as_of,
        inst.realized_var_method,
    )
}

pub(super) fn get_historical_prices_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<Vec<f64>> {
    let close_id_owned = inst
        .close_series_id
        .clone()
        .unwrap_or_else(|| inst.series_id());
    if let Ok(series) = context.get_series(&close_id_owned) {
        let dates: Vec<Date> = obs_dates.iter().copied().filter(|&d| d <= as_of).collect();
        if !dates.is_empty() {
            return dates
                .iter()
                .map(|&date| series.get_value_exact(date))
                .collect();
        }
    }

    let accrued = obs_dates.iter().filter(|&&date| date <= as_of).count();
    if accrued >= 2 || obs_dates.iter().any(|&date| date < as_of) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxVarianceSwap '{}' has {} past observation dates but no historical price data is available in series '{}'. Provide the time series before pricing a seasoned swap.",
            inst.id.as_str(),
            accrued,
            close_id_owned
        )));
    }

    let spot = inst.spot_rate(context, as_of)?;
    Ok(vec![spot])
}

/// Load aligned OHLC histories from the market context for OHLC-based estimators.
///
/// Returns `Err(Validation)` if any required series ID is missing.
pub(super) fn get_historical_ohlc_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<OhlcVecs> {
    let default_close = inst
        .close_series_id
        .clone()
        .unwrap_or_else(|| inst.series_id());

    let method_label = inst.realized_var_method;
    let inst_id = inst.id.as_str().to_owned();

    let open_id = inst.open_series_id.as_deref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "FxVarianceSwap '{inst_id}': 'open_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let high_id = inst.high_series_id.as_deref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "FxVarianceSwap '{inst_id}': 'high_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let low_id = inst.low_series_id.as_deref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "FxVarianceSwap '{inst_id}': 'low_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;

    let dates: Vec<Date> = obs_dates.iter().copied().filter(|&d| d <= as_of).collect();

    if dates.is_empty() {
        return Ok((vec![], vec![], vec![], vec![]));
    }

    let exact_values = |id: &str| -> Result<Vec<f64>> {
        let series = context.get_series(id)?;
        dates
            .iter()
            .map(|&date| series.get_value_exact(date))
            .collect()
    };
    let open_vals = exact_values(open_id)?;
    let high_vals = exact_values(high_id)?;
    let low_vals = exact_values(low_id)?;
    let close_vals = exact_values(&default_close)?;

    Ok((open_vals, high_vals, low_vals, close_vals))
}

pub(crate) fn partial_realized_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    partial_realized_variance_with_dates(inst, context, as_of, &observation_dates(inst)?)
}

pub(super) fn partial_realized_variance_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    let annualization_factor = annualization_factor(inst);
    if inst.realized_var_method.requires_ohlc() {
        if obs_dates.first().is_some_and(|first| as_of < *first) {
            return Ok(0.0);
        }
        let (open, high, low, close) =
            get_historical_ohlc_with_dates(inst, context, as_of, obs_dates)?;

        let close_id = inst
            .close_series_id
            .clone()
            .unwrap_or_else(|| inst.series_id());
        let anchor = preceding_close(
            inst.realized_var_method,
            inst.previous_close_date,
            obs_dates,
            context,
            &close_id,
        )?;
        return finstack_quant_core::math::stats::realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            Some(inst.realized_var_method),
            Some(annualization_factor),
            anchor,
        )?
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "insufficient target bars for contractual realized variance".into(),
            )
        });
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, obs_dates)?;
    // Before the first return interval, the contractual accrued contribution is zero.
    Ok(realized_variance(&prices, Some(annualization_factor))?.unwrap_or(0.0))
}

/// Expected contractual annualized variance, shared by PV and its metric.
///
/// Known close-to-close squared log returns and the model's future squared
/// log returns receive the same `annualization / total_return_count` factor.
/// OHLC estimators retain their exact settlement definitions but have no
/// pre-settlement forecast model in this pricer.
pub(crate) fn expected_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    expected_variance_with_dates(inst, context, as_of, &observation_dates(inst)?)
}

pub(super) fn expected_variance_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    if obs_dates.last().is_some_and(|&last| as_of >= last) {
        return partial_realized_variance_with_dates(inst, context, as_of, obs_dates);
    }
    replication::require_close_to_close_forecast(inst)?;
    let total_returns = obs_dates.len().saturating_sub(1);
    if total_returns == 0 {
        return Err(finstack_quant_core::Error::Validation(
            "FX variance requires at least two distinct adjusted observations".into(),
        ));
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, obs_dates)?;
    // Calling the canonical estimator also validates historical prices.
    let realized = realized_variance(&prices, Some(1.0))?.unwrap_or(0.0);
    let known_sum = realized * prices.len().saturating_sub(1) as f64;
    let (future_sum, _) =
        replication::remaining_squared_returns(inst, context, as_of, obs_dates, &prices)?;
    Ok(annualization_factor(inst) * (known_sum + future_sum) / total_returns as f64)
}
