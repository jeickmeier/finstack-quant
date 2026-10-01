//! Shared pricing and metric helpers for FX instruments.
//!
use crate::instruments::common_impl::helpers::zero_rate_from_df;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::pricing::variance_replication::carr_madan_forward_variance;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::fx_variance_swap::FxVarianceSwap;
use finstack_quant_models::closed_form::vanilla::bs_price_unchecked;

type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
use finstack_quant_core::{
    dates::{Date, DayCountContext},
    market_data::{
        context::MarketContext,
        surfaces::{VolQuoteType, VolSurfaceAxis},
    },
    math::stats::realized_variance,
    money::Money,
    Result,
};

pub(crate) fn compute_pv(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    inst.validate()?;
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
    Ok(inst.payoff(expected_var)? * df)
}

pub(crate) fn observation_dates(inst: &FxVarianceSwap) -> Result<Vec<Date>> {
    crate::instruments::common_impl::pricing::variance_observations::variance_observation_dates(
        inst.start_date,
        inst.maturity,
        inst.observation_frequency,
        inst.observation_business_day_convention,
        inst.observation_end_of_month,
        crate::instruments::common_impl::pricing::variance_observations::VarianceCalendar::Joint {
            base: &inst.base_calendar_id,
            quote: &inst.quote_calendar_id,
        },
    )
}

pub(crate) fn annualization_factor(inst: &FxVarianceSwap) -> f64 {
    use finstack_quant_core::dates::TenorUnit;
    if let Some(months) = inst.observation_frequency.months() {
        return 12.0 / months as f64;
    }
    if inst.observation_frequency.unit() == TenorUnit::Weeks {
        return 52.0 / f64::from(inst.observation_frequency.count());
    }
    if inst.observation_frequency.unit() == TenorUnit::Days {
        return inst.trading_days_per_year / f64::from(inst.observation_frequency.count());
    }
    inst.trading_days_per_year
}

pub(crate) fn realized_fraction_by_observations(inst: &FxVarianceSwap, as_of: Date) -> Result<f64> {
    Ok(realized_fraction_by_observations_with_dates(
        inst,
        as_of,
        &observation_dates(inst)?,
    ))
}

/// Fraction of the observation period elapsed at `as_of`, measured by the
/// instrument's day-count convention.
///
/// This descriptive calendar-time fraction is not the contractual variance
/// weight. Pricing normalizes expected squared returns by the full number of
/// scheduled return samples, including weekends and irregular observation gaps.
pub(crate) fn time_elapsed_fraction(inst: &FxVarianceSwap, as_of: Date) -> Result<f64> {
    let final_observation_date = observation_dates(inst)?
        .last()
        .copied()
        .unwrap_or(inst.maturity);
    if as_of <= inst.start_date {
        return Ok(0.0);
    }
    if as_of >= final_observation_date {
        return Ok(1.0);
    }
    let total = inst.day_count.year_fraction(
        inst.start_date,
        final_observation_date,
        DayCountContext::default(),
    )?;
    if total <= 0.0 {
        return Ok(0.0);
    }
    let elapsed = inst
        .day_count
        .year_fraction(inst.start_date, as_of, DayCountContext::default())?
        .clamp(0.0, total);
    Ok((elapsed / total).clamp(0.0, 1.0))
}

fn realized_fraction_by_observations_with_dates(
    inst: &FxVarianceSwap,
    as_of: Date,
    all: &[Date],
) -> f64 {
    if all.is_empty() {
        return 0.0;
    }
    if as_of >= all.last().copied().unwrap_or(inst.maturity) {
        return 1.0;
    }
    let accrued = all.iter().filter(|&&d| d <= as_of).count();
    use finstack_quant_core::math::stats::RealizedVarMethod;
    let (realized, total) = match inst.realized_var_method {
        RealizedVarMethod::CloseToClose | RealizedVarMethod::YangZhang => {
            // Yang-Zhang's first close anchors the overnight return of the
            // next bar; it is not itself an estimator sample.
            (accrued.saturating_sub(1), all.len().saturating_sub(1))
        }
        RealizedVarMethod::Parkinson
        | RealizedVarMethod::GarmanKlass
        | RealizedVarMethod::RogersSatchell => (accrued, all.len()),
    };
    if total == 0 {
        0.0
    } else {
        realized as f64 / total as f64
    }
}

pub(crate) fn get_historical_prices(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Vec<f64>> {
    get_historical_prices_with_dates(inst, context, as_of, &observation_dates(inst)?)
}

fn get_historical_prices_with_dates(
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
                .map(|&date| series.value_on_exact(date))
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
fn get_historical_ohlc_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<OhlcVecs> {
    let default_close = inst
        .close_series_id
        .clone()
        .unwrap_or_else(|| inst.series_id());

    let method_label = inst.realized_var_method.label();
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
            .map(|&date| series.value_on_exact(date))
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

fn partial_realized_variance_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    realized_variance_with_factor(inst, context, as_of, obs_dates, annualization_factor(inst))
}

fn realized_variance_with_factor(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
    annualization_factor: f64,
) -> Result<f64> {
    if inst.realized_var_method.requires_ohlc() {
        let (open, high, low, close) =
            get_historical_ohlc_with_dates(inst, context, as_of, obs_dates)?;
        if close.is_empty() {
            return Ok(0.0);
        }
        return finstack_quant_core::math::stats::realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            inst.realized_var_method,
            annualization_factor,
        );
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, obs_dates)?;
    if prices.len() < 2 {
        return Ok(0.0);
    }
    realized_variance(&prices, inst.realized_var_method, annualization_factor)
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

fn expected_variance_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    if obs_dates.last().is_some_and(|&last| as_of >= last) {
        return partial_realized_variance_with_dates(inst, context, as_of, obs_dates);
    }
    require_close_to_close_forecast(inst)?;
    let total_returns = obs_dates.len().saturating_sub(1);
    if total_returns == 0 {
        return Err(finstack_quant_core::Error::Validation(
            "FX variance requires at least two distinct adjusted observations".into(),
        ));
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, obs_dates)?;
    // Calling the canonical estimator also validates historical prices.
    let realized = realized_variance(&prices, inst.realized_var_method, 1.0)?;
    let known_sum = realized * prices.len().saturating_sub(1) as f64;
    let (future_sum, _) = remaining_squared_returns(inst, context, as_of, obs_dates, &prices)?;
    Ok(annualization_factor(inst) * (known_sum + future_sum) / total_returns as f64)
}

fn require_close_to_close_forecast(inst: &FxVarianceSwap) -> Result<()> {
    if inst.realized_var_method.requires_ohlc() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FX variance swap '{}': pre-settlement forecasting is supported only for close_to_close; {} requires an estimator-specific OHLC forecast model. Fully observed settlement remains supported.",
            inst.id, inst.realized_var_method.label(),
        )));
    }
    Ok(())
}

/// Gaussian independent-increment projection calibrated to the smile-replicated
/// cumulative expected quadratic variation. Rates are deterministic domestic
/// and foreign discount curves. For each unsettled return, E[R²] = q + m²,
/// where m includes log FX carry, -q/2, and the observed partial return.
///
/// This is exact for deterministic instantaneous variance and deterministic
/// rates. A vanilla smile alone does not identify forward log-return second
/// moments under general stochastic volatility or stochastic rates; there is
/// no claim that this projection supplies that missing joint distribution.
fn remaining_squared_returns(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
    historical_prices: &[f64],
) -> Result<(f64, usize)> {
    require_close_to_close_forecast(inst)?;
    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;
    let foreign = context.get_discount(inst.foreign_discount_curve_id.as_str())?;
    let spot = inst.spot_rate(context, as_of)?;
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "FX variance forecast requires finite positive spot".into(),
        ));
    }
    let mut total = finstack_quant_core::math::NeumaierAccumulator::new();
    let mut count = 0;
    let mut prior_boundary = None;
    for window in obs_dates.windows(2).filter(|window| window[1] > as_of) {
        let start = window[0].max(as_of);
        let end = window[1];
        let start_variation = match prior_boundary {
            Some((date, variation)) if date == start => variation,
            _ => cumulative_variation(inst, context, as_of, start)?,
        };
        let end_variation = cumulative_variation(inst, context, as_of, end)?;
        prior_boundary = Some((end, end_variation));
        let raw_variation = end_variation - start_variation;
        let tolerance = 1e-12 * end_variation.abs().max(start_variation.abs()).max(1.0);
        if raw_variation < -tolerance {
            return Err(finstack_quant_core::Error::Validation(format!(
                "FX variance swap '{}': negative forward variation {raw_variation} over {start} to {end}; check smile calendar consistency and replication grid",
                inst.id,
            )));
        }
        let variation = raw_variation.max(0.0);
        let carry =
            (foreign.df_between_dates(start, end)? / dom.df_between_dates(start, end)?).ln();
        let partial_return = if window[0] <= as_of {
            let last_fixing = historical_prices.last().copied().ok_or_else(|| {
                finstack_quant_core::Error::Validation("FX variance missing latest fixing".into())
            })?;
            (spot / last_fixing).ln()
        } else {
            0.0
        };
        let mean = partial_return + carry - 0.5 * variation;
        total.add(variation + mean * mean);
        count += 1;
    }
    Ok((total.total(), count))
}

fn cumulative_variation(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    end: Date,
) -> Result<f64> {
    if end <= as_of {
        return Ok(0.0);
    }
    let t = inst
        .day_count
        .year_fraction(as_of, end, DayCountContext::default())?;
    Ok(replicated_variance_to(inst, context, as_of, end)? * t)
}

pub(crate) fn remaining_forward_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let dates = observation_dates(inst)?;
    if dates.last().is_some_and(|&date| as_of >= date) {
        return Ok(0.0);
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, &dates)?;
    let (sum, count) = remaining_squared_returns(inst, context, as_of, &dates, &prices)?;
    if count == 0 {
        return Ok(0.0);
    }
    Ok(annualization_factor(inst) * sum / count as f64)
}

/// Smile-replicated annualized quadratic variation from as-of to one boundary.
fn replicated_variance_to(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    final_observation_date: Date,
) -> Result<f64> {
    let t =
        inst.day_count
            .year_fraction(as_of, final_observation_date, DayCountContext::default())?;
    if t <= 0.0 {
        return Ok(0.0);
    }

    let spot = inst.spot_rate(context, as_of)?;
    let surface = context.get_surface(inst.vol_surface_id.as_str())?;
    surface.require_quote_type(VolQuoteType::BlackLognormal)?;
    surface.require_secondary_axis(VolSurfaceAxis::Strike)?;
    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;
    let for_curve = context.get_discount(inst.foreign_discount_curve_id.as_str())?;
    // Date-based discount factors: `df_between_dates(as_of, maturity)` resolves the
    // year fraction on the curve's own time axis as a ratio `df(to)/df(from)`, so it
    // correctly represents the forward DF over `[as_of, maturity]` regardless of
    // where `as_of` sits relative to the curve's `base_date`.
    //
    // The previous code called `curve.df(yf(as_of, maturity))`, which looks up the
    // *spot* DF at `yf(as_of, maturity)` years from the curve's `base_date` — i.e.
    // the DF from `base_date` to roughly `base_date + yf(as_of, mat)`, not from
    // `as_of` to `mat`.  For a non-flat term structure (or any `as_of != base_date`)
    // this gives the wrong rate and therefore the wrong GK forward.  The terminal-PV
    // discount in this same function was already corrected to use `df_between_dates`
    // (see lines 66, 83); this aligns the forward-recovery path with that fix.
    let df_dom = dom.df_between_dates(as_of, final_observation_date)?;
    let df_for = for_curve.df_between_dates(as_of, final_observation_date)?;

    let r_d = zero_rate_from_df(df_dom, t, "FxVarianceSwap domestic discount")?;
    let r_f = zero_rate_from_df(df_for, t, "FxVarianceSwap foreign discount")?;
    let fwd = spot * ((r_d - r_f) * t).exp();
    let strikes = surface.strikes();
    let vol_fn = |t_exp: f64, k: f64| {
        finstack_quant_models::volatility::get_surface_vol_clamped(&surface, t_exp, k)
    };
    let bs_fn = |k: f64, v: f64, opt: OptionType| -> f64 {
        bs_price_unchecked(spot, k, r_d, r_f, v, t, opt)
    };
    carr_madan_forward_variance(strikes, fwd, r_d, t, vol_fn, bs_fn).ok_or_else(|| {
        finstack_quant_core::Error::Calibration {
            message: format!(
                "FX variance swap '{}': Carr-Madan replication failed. The supplied vol surface \
             must contain valid OTM put and call wings around forward {fwd} at maturity {t:.4}y \
             (received {} strikes); ATM-vol² fallback is disabled because it discards smile risk.",
                inst.id(),
                strikes.len()
            ),
            category: "fx_variance_swap_replication".to_string(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use std::sync::Arc;
    use time::macros::date;

    fn build_market(as_of: Date) -> MarketContext {
        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.01_f64).exp())])
            .build()
            .expect("eur curve");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        let fx = FxMatrix::new(Arc::new(provider));
        MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(fx)
    }

    fn zero_rate_market(as_of: Date, volatility: f64) -> MarketContext {
        use finstack_quant_core::market_data::surfaces::VolSurface;
        let mut market = build_market(as_of);
        for id in ["USD-OIS", "EUR-OIS"] {
            market = market.insert(
                DiscountCurve::builder(id)
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (5.0, 1.0)])
                    .build()
                    .expect("curve"),
            );
        }
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.0)
            .expect("spot");
        let strikes: Vec<_> = (1..=400).map(|i| f64::from(i) * 0.01).collect();
        let vols = vec![volatility; strikes.len()];
        market
            .insert_fx(FxMatrix::new(Arc::new(provider)))
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[1.0, 3.0])
                    .strikes(&strikes)
                    .row(&vols)
                    .row(&vols)
                    .build()
                    .expect("surface"),
            )
    }

    #[test]
    fn replication_rejects_non_black_and_non_strike_surfaces() {
        let swap = FxVarianceSwap::example().expect("swap");
        let market = zero_rate_market(swap.start_date, 0.2);
        let base = market
            .get_surface(swap.vol_surface_id.as_str())
            .expect("surface");
        for surface in [
            base.as_ref().clone().with_displacements(&[0.1; 2]).unwrap(),
            base.as_ref()
                .clone()
                .with_quote_type(VolQuoteType::Normal)
                .unwrap(),
            base.as_ref()
                .clone()
                .with_secondary_axis(VolSurfaceAxis::Tenor),
        ] {
            let context = market.clone().insert_surface(surface);
            let error = replicated_variance_to(&swap, &context, swap.start_date, swap.maturity)
                .expect_err("Carr-Madan Black replication must enforce surface metadata");
            let text = error.to_string();
            assert!(
                text.contains("quotes") || text.contains("secondary axis"),
                "{text}"
            );
        }
    }

    #[test]
    fn fixed_returns_keep_the_same_denominator_before_final_fixing() {
        use finstack_quant_core::dates::Tenor;
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        for frequency in [Tenor::daily(), Tenor::weekly(), Tenor::monthly()] {
            for annualization in [252.0, 365.0] {
                let mut swap = FxVarianceSwap::example().expect("example");
                swap.start_date = date!(2025 - 01 - 08);
                swap.maturity = if frequency == Tenor::daily() {
                    date!(2025 - 01 - 13)
                } else {
                    date!(2025 - 05 - 08)
                };
                swap.observation_frequency = frequency;
                swap.trading_days_per_year = annualization;
                swap.strike_variance = 0.0;
                let dates = observation_dates(&swap).expect("dates");
                let final_date = *dates.last().expect("last");
                let as_of = final_date - finstack_quant_core::dates::Duration::days(1);
                let market = zero_rate_market(as_of, 0.00001);
                // Exactly two known returns, +1% then -1%; all remaining
                // historical closes and the final close are 1.0.
                let observations: Vec<_> = dates
                    .iter()
                    .enumerate()
                    .map(|(i, &date)| (date, if i == 1 { 0.01_f64.exp() } else { 1.0 }))
                    .collect();
                let control_observations = dates.iter().map(|&date| (date, 1.0)).collect();
                let known = market.clone().insert_series(
                    ScalarTimeSeries::new("EURUSD", observations, None).expect("series"),
                );
                let control = market.insert_series(
                    ScalarTimeSeries::new("EURUSD", control_observations, None).expect("series"),
                );
                let before_difference = expected_variance(&swap, &known, as_of).expect("known")
                    - expected_variance(&swap, &control, as_of).expect("control");
                let exact = annualization_factor(&swap) * 0.0002 / (dates.len() - 1) as f64;
                assert!((before_difference - exact).abs() < 1e-12);
                let settled = expected_variance(&swap, &known, final_date).expect("settled");
                assert!((settled - exact).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn forward_start_uses_only_the_observed_window_and_discrete_mean_correction() {
        use finstack_quant_core::dates::Tenor;
        use finstack_quant_core::market_data::surfaces::VolSurface;
        let as_of = date!(2025 - 01 - 06);
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2026 - 01 - 06);
        swap.maturity = date!(2027 - 01 - 06);
        swap.observation_frequency = Tenor::monthly();
        let dates = observation_dates(&swap).expect("dates");
        let times: Vec<_> = dates
            .iter()
            .map(|&date| {
                swap.day_count
                    .year_fraction(as_of, date, DayCountContext::default())
                    .expect("time")
            })
            .collect();
        // Deterministic instantaneous variance: 1% until year 1, then 17%.
        // Both intervals together imply 30% annual vol at year 2.
        let integrated = |time: f64| 0.01 * time.min(1.0) + 0.17 * (time - 1.0).max(0.0);
        let strikes: Vec<_> = (1..=800).map(|i| f64::from(i) * 0.005).collect();
        let mut surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&times)
            .strikes(&strikes);
        for &time in &times {
            surface = surface.row(&vec![(integrated(time) / time).sqrt(); strikes.len()]);
        }
        let market = zero_rate_market(as_of, 0.1).insert_surface(surface.build().expect("surface"));
        let expected = times
            .windows(2)
            .map(|window| {
                let q = integrated(window[1]) - integrated(window[0]);
                q + 0.25 * q * q
            })
            .sum::<f64>()
            * annualization_factor(&swap)
            / (dates.len() - 1) as f64;
        let actual = expected_variance(&swap, &market, as_of).expect("forward variance");
        assert!(
            (actual - expected).abs() < 0.00005,
            "{actual} vs {expected}"
        );
        let mut earlier = swap;
        earlier.start_date = as_of;
        let earlier_value = expected_variance(&earlier, &market, as_of).expect("earlier start");
        assert!(actual > earlier_value + 0.04);
    }

    #[test]
    fn fx_variance_vega_reprices_the_same_sample_normalized_model() {
        use crate::instruments::fx::fx_variance_swap::metrics::VegaCalculator;
        use crate::metrics::bump_surface_vol_absolute;
        use crate::metrics::{MetricCalculator, MetricContext};
        use finstack_quant_core::dates::Tenor;
        let as_of = date!(2025 - 01 - 06);
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2026 - 01 - 06);
        swap.maturity = date!(2027 - 01 - 06);
        swap.observation_frequency = Tenor::monthly();
        let market = zero_rate_market(as_of, 0.2);
        let up = bump_surface_vol_absolute(&market, "EURUSD-VOL", 0.01).expect("up");
        let down = bump_surface_vol_absolute(&market, "EURUSD-VOL", -0.01).expect("down");
        let expected = (swap.value(&up, as_of).expect("up PV").amount()
            - swap.value(&down, as_of).expect("down PV").amount())
            / 2.0;
        let base = swap.value(&market, as_of).expect("base");
        let mut context = MetricContext::new(
            Arc::new(swap),
            Arc::new(market),
            as_of,
            base,
            MetricContext::default_config(),
        );
        let actual = VegaCalculator.calculate(&mut context).expect("vega");
        assert!((actual - expected).abs() < 1e-6);
    }

    #[test]
    fn negative_forward_variation_is_rejected() {
        use finstack_quant_core::dates::Tenor;
        use finstack_quant_core::market_data::surfaces::VolSurface;
        let as_of = date!(2025 - 01 - 06);
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2026 - 01 - 06);
        swap.maturity = date!(2027 - 01 - 06);
        swap.observation_frequency = Tenor::annual();
        let strikes: Vec<_> = (1..=400).map(|i| f64::from(i) * 0.01).collect();
        let high = vec![0.4; strikes.len()];
        let low = vec![0.1; strikes.len()];
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0, 2.0])
            .strikes(&strikes)
            .row(&high)
            .row(&low)
            .build()
            .expect("surface");
        let market = zero_rate_market(as_of, 0.1).insert_surface(surface);
        assert!(expected_variance(&swap, &market, as_of)
            .expect_err("calendar arbitrage")
            .to_string()
            .contains("negative forward variation"));
    }

    #[test]
    fn ohlc_forecast_is_explicitly_unsupported_but_single_bar_realization_works() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use finstack_quant_core::math::stats::RealizedVarMethod;
        let as_of = date!(2025 - 01 - 06);
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = as_of;
        swap.maturity = date!(2025 - 01 - 08);
        swap.realized_var_method = RealizedVarMethod::Parkinson;
        swap.open_series_id = Some("OPEN".into());
        swap.high_series_id = Some("HIGH".into());
        swap.low_series_id = Some("LOW".into());
        let mut market = zero_rate_market(as_of, 0.2);
        for (name, value) in [
            ("OPEN", 1.0),
            ("HIGH", 1.01),
            ("LOW", 0.99),
            ("EURUSD", 1.0),
        ] {
            market = market.insert_series(
                ScalarTimeSeries::new(name, vec![(as_of, value)], None).expect("series"),
            );
        }
        let realized = partial_realized_variance(&swap, &market, as_of).expect("single bar");
        let exact =
            (1.01_f64 / 0.99).ln().powi(2) * annualization_factor(&swap) / (4.0 * 2.0_f64.ln());
        assert!((realized - exact).abs() < 1e-12);
        assert!(expected_variance(&swap, &market, as_of)
            .expect_err("no OHLC model")
            .to_string()
            .contains("estimator-specific"));
    }

    #[test]
    fn realized_fraction_uses_each_estimators_sample_count() {
        use finstack_quant_core::math::stats::RealizedVarMethod;
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2025 - 01 - 06);
        swap.maturity = date!(2025 - 01 - 13);
        let dates = observation_dates(&swap).expect("six business dates");
        assert_eq!(dates.len(), 6);
        for method in [
            RealizedVarMethod::CloseToClose,
            RealizedVarMethod::YangZhang,
            RealizedVarMethod::Parkinson,
            RealizedVarMethod::GarmanKlass,
            RealizedVarMethod::RogersSatchell,
        ] {
            swap.realized_var_method = method;
            let anchored = matches!(
                method,
                RealizedVarMethod::CloseToClose | RealizedVarMethod::YangZhang
            );
            for (index, &date) in dates.iter().enumerate() {
                let expected = if anchored {
                    index as f64 / 5.0
                } else {
                    (index + 1) as f64 / 6.0
                };
                let actual = realized_fraction_by_observations(&swap, date).expect("fraction");
                assert!(
                    (actual - expected).abs() < 1e-12,
                    "{method:?}: {actual} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn unfinished_return_includes_the_move_since_the_last_fixing() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        let as_of = date!(2025 - 01 - 12);
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2025 - 01 - 08);
        swap.maturity = date!(2025 - 01 - 13);
        let dates = observation_dates(&swap).expect("dates");
        let observations = dates
            .iter()
            .copied()
            .filter(|&date| date <= as_of)
            .map(|date| (date, 1.0))
            .collect();
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.02)
            .expect("spot");
        let market = zero_rate_market(as_of, 0.2)
            .insert_fx(FxMatrix::new(Arc::new(provider)))
            .insert_series(ScalarTimeSeries::new("EURUSD", observations, None).expect("series"));
        let q =
            cumulative_variation(&swap, &market, as_of, swap.maturity).expect("future variation");
        let mean = 1.02_f64.ln() - 0.5 * q;
        let expected = annualization_factor(&swap) * (q + mean * mean) / 3.0;
        let actual = expected_variance(&swap, &market, as_of).expect("variance");
        assert!((actual - expected).abs() < 1e-12);
        let without_partial_move = annualization_factor(&swap) * (q + 0.25 * q * q) / 3.0;
        assert!((actual - without_partial_move).abs() > 0.01);
    }

    #[test]
    fn fully_observed_ohlc_settlement_preserves_estimator_definitions() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use finstack_quant_core::math::stats::RealizedVarMethod;
        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2025 - 01 - 06);
        swap.maturity = date!(2025 - 01 - 09);
        swap.open_series_id = Some("OPEN".into());
        swap.high_series_id = Some("HIGH".into());
        swap.low_series_id = Some("LOW".into());
        let dates = observation_dates(&swap).expect("four bars");
        assert_eq!(dates.len(), 4);
        let mut market = zero_rate_market(swap.start_date, 0.2);
        for (id, first, later) in [
            ("OPEN", 1.0, 1.0),
            ("EURUSD", 1.0, 1.0),
            ("HIGH", 1.2, 1.01),
            ("LOW", 0.8, 0.99),
        ] {
            let observations = dates
                .iter()
                .enumerate()
                .map(|(i, &date)| (date, if i == 0 { first } else { later }))
                .collect();
            market = market
                .insert_series(ScalarTimeSeries::new(id, observations, None).expect("series"));
        }
        let range_mean =
            ((1.2_f64 / 0.8).ln().powi(2) + 3.0 * (1.01_f64 / 0.99).ln().powi(2)) / 4.0;
        let rs_first = 1.2_f64.ln().powi(2) + 0.8_f64.ln().powi(2);
        let rs_later = 1.01_f64.ln().powi(2) + 0.99_f64.ln().powi(2);
        for (method, per_period) in [
            (
                RealizedVarMethod::Parkinson,
                range_mean / (4.0 * 2.0_f64.ln()),
            ),
            (RealizedVarMethod::GarmanKlass, 0.5 * range_mean),
            (
                RealizedVarMethod::RogersSatchell,
                (rs_first + 3.0 * rs_later) / 4.0,
            ),
            // Three return periods; Yang-Zhang excludes the first anchor bar.
            (
                RealizedVarMethod::YangZhang,
                (1.0 - 0.34 / (1.34 + 4.0 / 2.0)) * rs_later,
            ),
        ] {
            swap.realized_var_method = method;
            let expected = annualization_factor(&swap) * per_period;
            let actual =
                expected_variance(&swap, &market, swap.maturity).expect("settlement variance");
            assert!(
                (actual - expected).abs() < 1e-12,
                "{method:?}: {actual} vs {expected}"
            );
            let pv = swap
                .value(&market, swap.maturity)
                .expect("settlement PV")
                .amount();
            assert!((pv - swap.payoff(expected).expect("payoff").amount()).abs() < 1e-7);
        }
    }

    #[test]
    fn fx_variance_swap_pricer_compute_pv_matches_instrument_value() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        let swap = FxVarianceSwap::example().expect("example");
        let as_of = date!(2025 - 01 - 02);
        let observations = observation_dates(&swap)
            .expect("observation schedule")
            .into_iter()
            .map(|date| (date, 1.10))
            .collect();
        let series = ScalarTimeSeries::new("EURUSD", observations, None).expect("series");
        let market = build_market(as_of).insert_series(series);

        let via_pricer = compute_pv(&swap, &market, as_of).expect("pricer pv");
        let via_instrument = swap.value(&market, as_of).expect("instrument pv");

        assert_eq!(via_pricer, via_instrument);
    }

    #[test]
    fn multi_day_tenor_steps_in_business_observations() {
        use finstack_quant_core::dates::{Tenor, TenorUnit};

        let mut swap = FxVarianceSwap::example().expect("example");
        swap.start_date = date!(2025 - 01 - 03); // Friday
        swap.maturity = date!(2025 - 01 - 15);
        swap.observation_frequency = Tenor::new(2, TenorUnit::Days).expect("valid tenor fixture");

        let dates = observation_dates(&swap).expect("observation schedule");
        assert_eq!(annualization_factor(&swap), 126.0);
        assert_eq!(dates[0], date!(2025 - 01 - 03));
        assert_eq!(dates[1], date!(2025 - 01 - 07));
        assert!(dates
            .iter()
            .all(|d| !matches!(d.weekday(), time::Weekday::Saturday | time::Weekday::Sunday)));

        swap.observation_frequency = Tenor::new(2, TenorUnit::Weeks).expect("valid tenor fixture");
        assert_eq!(annualization_factor(&swap), 26.0);
    }

    /// The full sample denominator is identical before and at settlement;
    /// calendar-time weights do not change already fixed return contributions.
    #[test]
    fn fx_seasoned_mtm_uses_contractual_sample_normalization() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::fx::fx_variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::math::stats::RealizedVarMethod;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 06 - 27); // Friday, near maturity

        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("FXVAR-SEASONED"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_id("EURUSD".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .base_calendar_id("TARGET2".into())
            .quote_calendar_id("USNY".into())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("seasoned fx swap");

        let count_w =
            realized_fraction_by_observations(&swap, as_of).expect("observation fraction");
        let time_w = time_elapsed_fraction(&swap, as_of).expect("time fraction");
        assert!(
            (count_w - time_w).abs() > 1e-4,
            "schedule must make count weight ({count_w}) differ from time weight ({time_w})"
        );

        // Close series over every past observation date with a non-trivial
        // return path so realized variance != forward variance.
        let past: Vec<Date> = observation_dates(&swap)
            .expect("observation schedule")
            .into_iter()
            .filter(|&d| d <= as_of)
            .collect();
        let obs: Vec<(Date, f64)> = past
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, 1.10 * (1.0 + 0.002 * (i as f64 % 3.0 - 1.0))))
            .collect();
        let series = ScalarTimeSeries::new("EURUSD", obs, None).expect("series");
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[0.9, 1.1, 1.3])
            .row(&[0.12, 0.10, 0.12])
            .build()
            .expect("surface");
        let market = build_market(as_of)
            .insert_series(series)
            .insert_surface(surface);

        let pv = compute_pv(&swap, &market, as_of).expect("seasoned fx pv");

        let realized = partial_realized_variance(&swap, &market, as_of).expect("realized");
        let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
        let expected_var = realized * count_w + forward * (1.0 - count_w);
        let dom = market.get_discount("USD-OIS").expect("curve");
        // Date-based discounting, matching the pricer (item 4).
        let df = dom
            .df_between_dates(as_of, swap.maturity)
            .expect("date-based df");
        let expected_pv = swap.payoff(expected_var).expect("valid payoff") * df;

        assert!(
            (pv.amount() - expected_pv.amount()).abs() < 1e-6,
            "FX seasoned MTM must preserve the sample denominator: pv={} expected={}",
            pv.amount(),
            expected_pv.amount()
        );

        let count_var = realized * time_w + forward * (1.0 - time_w);
        let count_pv = swap.payoff(count_var).expect("valid payoff") * df;
        assert!(
            (pv.amount() - count_pv.amount()).abs() > 1e-6,
            "FX seasoned MTM must differ from calendar-time weighting"
        );
    }

    /// W39 regression: `remaining_forward_variance` must recover the GK forward
    /// via date-based discount factors. The buggy code calls `curve.df(yf(as_of,
    /// mat))`, which looks up the *spot* DF at `yf(as_of, mat)` years from the
    /// curve's `base_date` — i.e. the wrong time point on the curve's axis.
    /// The correct DF for the period `[as_of, mat]` is
    /// `curve.df_between_dates(as_of, mat)` = `df(yf(base, mat)) / df(yf(base, as_of))`.
    ///
    /// These two differ whenever the curve is non-flat (i.e. the spot rate at
    /// `yf(as_of, mat)` from `base` ≠ the forward rate over `[as_of, mat]`).
    /// The fixture uses a three-knot stepped curve (r_high for [0, as_of],
    /// r_low for [as_of, mat]) so the two lookups diverge materially.
    #[test]
    fn fx_variance_swap_forward_recovery_is_date_based() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::fx::fx_variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, DayCountContext, Tenor};
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::math::stats::RealizedVarMethod;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        // Three dates:
        //   base_date  = 2025-01-02  (curve anchor)
        //   as_of      = 2025-07-01  (~0.5y from base)
        //   maturity   = 2026-01-02  (~1.0y from base, ~0.5y from as_of)
        //
        // Stepped (non-flat) curves so the spot DF at `yf(as_of, mat)` from base
        // is materially different from the forward DF over `[as_of, mat]`:
        //   Domestic: r_near = 10% for [base, as_of], r_far = 0% for [as_of, mat].
        //     df_between(as_of, mat) ≈ 1.0  (zero rate going forward)
        //     dom.df(yf(as_of,mat) ≈ 0.5) ≈ exp(-0.10*0.5) ≈ 0.951 (WRONG — reads near segment)
        //   Foreign: r_near = 0% for [base, as_of], r_far = 10% for [as_of, mat].
        //     df_between(as_of, mat) ≈ exp(-0.10*0.5) ≈ 0.951
        //     for_curve.df(0.5)      ≈ 1.0             (WRONG — reads near segment)
        //
        //   Date-based fwd  ≈ spot * exp((r_d=0 − r_f=0.10) * 0.5) ≈ 1.10 * 0.951 ≈ 1.046
        //   Axis-buggy  fwd ≈ spot * exp((r_d≈10% − r_f≈0%) * 0.5) ≈ 1.10 * 1.051 ≈ 1.156
        //   Gap ≈ 0.11 >> 1e-3.
        //
        // Vol surface has a strong strike slope so replicated variance is
        // sensitive to the recovered forward.
        let curve_base = date!(2025 - 01 - 02);
        let as_of = date!(2025 - 07 - 01);
        let start = date!(2025 - 07 - 02);
        let maturity = date!(2026 - 01 - 02);

        // Knot times in Act365F from curve_base.
        // t_near ≈ yf(2025-01-02, 2025-07-01) = 180/365 ≈ 0.4932
        // t_mat  ≈ yf(2025-01-02, 2026-01-02) = 365/365 = 1.0000
        let t_near: f64 = 0.4932;
        let t_mat: f64 = 1.0000;

        // Domestic: r_near = 10%, r_far = 0%.
        let df_near_dom = (-0.10_f64 * t_near).exp();
        let df_mat_dom = df_near_dom; // exp(-0 * segment) = 1, so no further discount

        // Foreign: r_near = 0%, r_far = 10%.
        let df_near_for = 1.0_f64; // exp(-0 * t_near) = 1
        let df_mat_for = df_near_for * (-0.10_f64 * (t_mat - t_near)).exp();

        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (t_near, df_near_dom), (t_mat, df_mat_dom)])
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (t_near, df_near_for), (t_mat, df_mat_for)])
            .build()
            .expect("eur curve");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        // Dense wings support Carr-Madan replication while preserving a strong slope.
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[0.80, 0.90, 1.00, 1.10, 1.20, 1.30])
            .row(&[0.34, 0.30, 0.24, 0.18, 0.10, 0.05])
            .build()
            .expect("surface");
        let market = MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(FxMatrix::new(Arc::new(provider)))
            .insert_surface(surface);

        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("FXVAR-FWD-W39"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_id("EURUSD".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .base_calendar_id("TARGET2".into())
            .quote_calendar_id("USNY".into())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("pre-start fx swap");

        // ── Verify the fixture exposes a meaningful gap ───────────────────────
        let dom = market.get_discount("USD-OIS").expect("usd curve");
        let for_curve = market.get_discount("EUR-OIS").expect("eur curve");

        // Correct: forward DF from as_of to maturity.
        let df_dom_date = dom.df_between_dates(as_of, maturity).expect("date df dom");
        let df_for_date = for_curve
            .df_between_dates(as_of, maturity)
            .expect("date df for");

        // Buggy: spot DF at yf(as_of, mat) from base_date.
        let t_dom_axis = dom
            .day_count()
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("yf dom");
        let t_for_axis = for_curve
            .day_count()
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("yf for");
        let df_dom_bug = dom.df(t_dom_axis.max(0.0));
        let df_for_bug = for_curve.df(t_for_axis.max(0.0));

        assert!(
            (df_dom_date - df_dom_bug).abs() > 1e-3,
            "fixture must expose domestic DF gap: date={df_dom_date} axis={df_dom_bug}"
        );
        assert!(
            (df_for_date - df_for_bug).abs() > 1e-3,
            "fixture must expose foreign DF gap: date={df_for_date} axis={df_for_bug}"
        );

        // ── Derive expected (date-based) and buggy forwards ───────────────────
        let spot = 1.10_f64;
        let t = swap
            .day_count
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("yf");
        let r_d_date =
            crate::instruments::common_impl::helpers::zero_rate_from_df(df_dom_date, t, "dom")
                .expect("r_d date");
        let r_f_date =
            crate::instruments::common_impl::helpers::zero_rate_from_df(df_for_date, t, "for")
                .expect("r_f date");
        let fwd_expected = spot * ((r_d_date - r_f_date) * t).exp();

        let r_d_bug =
            crate::instruments::common_impl::helpers::zero_rate_from_df(df_dom_bug, t, "dom")
                .expect("r_d bug");
        let r_f_bug =
            crate::instruments::common_impl::helpers::zero_rate_from_df(df_for_bug, t, "for")
                .expect("r_f bug");
        let fwd_bug = spot * ((r_d_bug - r_f_bug) * t).exp();

        assert!(
            (fwd_expected - fwd_bug).abs() > 1e-3,
            "fixture must produce different GK forwards: date={fwd_expected} axis={fwd_bug}"
        );

        // ── Assert the fixed pricer uses the date-based forward ───────────────
        // Recompute Carr-Madan with each candidate forward and assert the pricer
        // follows the date-based one.
        let surface_ref = market.get_surface("EURUSD-VOL").expect("surface");
        let strikes = surface_ref.strikes();
        let vol_fn = |t_exp: f64, k: f64| {
            finstack_quant_models::volatility::get_surface_vol_clamped(&surface_ref, t_exp, k)
        };
        let expected_variance =
            carr_madan_forward_variance(strikes, fwd_expected, r_d_date, t, vol_fn, |k, v, opt| {
                bs_price_unchecked(spot, k, r_d_date, r_f_date, v, t, opt)
            })
            .expect("date-based replication");
        let bug_variance =
            carr_madan_forward_variance(strikes, fwd_bug, r_d_bug, t, vol_fn, |k, v, opt| {
                bs_price_unchecked(spot, k, r_d_bug, r_f_bug, v, t, opt)
            })
            .expect("axis-bug replication");

        let actual = replicated_variance_to(&swap, &market, as_of, maturity)
            .expect("forward variance must succeed");

        if (expected_variance - bug_variance).abs() > 1e-8 {
            assert!(
                (actual - expected_variance).abs() < (actual - bug_variance).abs(),
                "remaining_forward_variance must use date-based forward: \
                 actual={actual} date_expected={expected_variance} axis_bug={bug_variance}"
            );
        }
    }

    /// Item 4 regression: the terminal PV discount must be date-based. When the
    /// discount curve's `base_date` precedes `as_of`, `dom.df(yf(as_of, mat))`
    /// reads the curve at the wrong point on its time axis; the correct factor
    /// is `dom.df_between_dates(as_of, maturity)`.
    #[test]
    fn fx_variance_swap_terminal_discount_is_date_based() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::fx::fx_variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::market_data::surfaces::VolSurface;
        use finstack_quant_core::math::stats::RealizedVarMethod;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        // Curve base date is well before the valuation date (a stale-but-valid
        // curve). `df(t)` is anchored at base_date, so feeding it
        // `yf(as_of, maturity)` mis-discounts.
        let curve_base = date!(2025 - 01 - 02);
        let as_of = date!(2025 - 07 - 01);
        let start = date!(2025 - 07 - 02);
        let maturity = date!(2026 - 01 - 02);

        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (2.0, (-0.06_f64).exp())])
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(curve_base)
            .day_count(DayCount::Act360)
            .knots([(0.0, 1.0), (2.0, (-0.02_f64).exp())])
            .build()
            .expect("eur curve");
        let provider = SimpleFxProvider::new();
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[0.9, 1.1, 1.3])
            .row(&[0.12, 0.10, 0.12])
            .build()
            .expect("surface");
        let market = MarketContext::new()
            .insert(usd_curve)
            .insert(eur_curve)
            .insert_fx(FxMatrix::new(Arc::new(provider)))
            .insert_surface(surface);

        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("FXVAR-DISC"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_id("EURUSD".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .base_calendar_id("TARGET2".into())
            .quote_calendar_id("USNY".into())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("pre-start fx swap");

        let pv = compute_pv(&swap, &market, as_of).expect("pre-start pv");

        let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
        let dom = market.get_discount("USD-OIS").expect("curve");
        let df_correct = dom
            .df_between_dates(as_of, maturity)
            .expect("date-based df");
        let expected_pv = swap.payoff(forward).expect("valid payoff") * df_correct;
        assert!(
            (pv.amount() - expected_pv.amount()).abs() < 1e-6,
            "terminal PV must use date-based discounting: pv={} expected={}",
            pv.amount(),
            expected_pv.amount()
        );

        // The buggy time-axis lookup gives a materially different factor.
        let t_bug = swap
            .day_count
            .year_fraction(as_of, maturity, DayCountContext::default())
            .expect("yf");
        let df_bug = dom.df(t_bug.max(0.0));
        assert!(
            (df_correct - df_bug).abs() > 1e-4,
            "fixture must expose the discount-axis bug: df_correct={df_correct} df_bug={df_bug}"
        );
    }
}
