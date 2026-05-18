use crate::instruments::common_impl::helpers::zero_rate_from_df;
use crate::instruments::common_impl::models::bs_price;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::pricing::variance_replication::carr_madan_forward_variance;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::fx_variance_swap::FxVarianceSwap;

type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::{
    dates::{Date, DateExt, DayCountContext},
    market_data::context::MarketContext,
    math::stats::realized_variance,
    money::Money,
    Result,
};

/// Registry-facing pricer for FX variance swaps.
pub(crate) struct SimpleFxVarianceSwapDiscountingPricer;

pub(crate) fn compute_pv(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    inst.validate_as_of(context, as_of)?;

    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;

    // Compute observation dates once per pricing call. Each branch below would
    // otherwise rebuild this 1-3 times via the helper functions.
    let obs_dates = observation_dates(inst);

    if as_of >= inst.maturity {
        let realized_var = if inst.realized_var_method.requires_ohlc() {
            let (open, high, low, close) =
                get_historical_ohlc_with_dates(inst, context, as_of, &obs_dates)?;
            if close.is_empty() {
                return Ok(Money::new(0.0, inst.notional.currency()));
            }
            finstack_core::math::stats::realized_variance_ohlc(
                &open,
                &high,
                &low,
                &close,
                inst.realized_var_method,
                annualization_factor(inst),
            )?
        } else {
            let prices = get_historical_prices_with_dates(inst, context, as_of, &obs_dates)?;
            if prices.is_empty() {
                return Ok(Money::new(0.0, inst.notional.currency()));
            }
            realized_variance(
                &prices,
                inst.realized_var_method,
                annualization_factor(inst),
            )?
        };
        return Ok(inst.payoff(realized_var));
    }

    if as_of < inst.start_date {
        let forward_var = remaining_forward_variance(inst, context, as_of)?;
        let undiscounted = inst.payoff(forward_var);
        // Date-based discounting: `df_between_dates` resolves the year fraction
        // on the curve's own time axis. Feeding `df()` an instrument-day-count
        // year fraction mis-discounts whenever the instrument and curve
        // day-counts differ or `as_of != base_date`.
        let df = dom.df_between_dates(as_of, inst.maturity)?;
        return Ok(undiscounted * df);
    }

    let realized = partial_realized_variance_with_dates(inst, context, as_of, &obs_dates)?;
    let forward = remaining_forward_variance(inst, context, as_of)?;
    // Seasoned MTM blends already-annualized realized and forward variance.
    // The accrued-variance identity time-weights the un-annualized total
    // variance: σ²_expected = (V_accrued + E[V_fwd]·τ) / T, i.e. weights
    // t_elapsed/T and τ_remaining/T. Use the day-count `time_elapsed_fraction`
    // rather than an observation-count fraction, which only coincides for
    // perfectly uniform schedules (W-32).
    let w = time_elapsed_fraction(inst, as_of)?;
    let expected_var = realized * w + forward * (1.0 - w);
    let undiscounted = inst.payoff(expected_var);
    // Date-based discounting (see the pre-start branch above): `df_between_dates`
    // resolves the year fraction on the curve's own time axis.
    let df = dom.df_between_dates(as_of, inst.maturity)?;
    Ok(undiscounted * df)
}

pub(crate) fn observation_dates(inst: &FxVarianceSwap) -> Vec<Date> {
    let mut dates = Vec::new();
    let mut current = inst.start_date;

    if let Some(months_step) = inst.observation_freq.months() {
        while current <= inst.maturity {
            dates.push(current);
            current = current.add_months(months_step as i32);
            if current > inst.maturity {
                break;
            }
        }
    } else if let Some(days_step) = inst.observation_freq.days() {
        if days_step == 1 {
            while current <= inst.maturity {
                if current.weekday() != time::Weekday::Saturday
                    && current.weekday() != time::Weekday::Sunday
                {
                    dates.push(current);
                }
                current += time::Duration::days(1);
            }
        } else {
            while current <= inst.maturity {
                dates.push(current);
                current += time::Duration::days(days_step as i64);
                if current > inst.maturity {
                    break;
                }
            }
        }
    } else {
        while current <= inst.maturity {
            if current.weekday() != time::Weekday::Saturday
                && current.weekday() != time::Weekday::Sunday
            {
                dates.push(current);
            }
            current += time::Duration::days(1);
        }
    }

    if (dates.is_empty() || dates.last() != Some(&inst.maturity)) && !dates.contains(&inst.maturity)
    {
        dates.push(inst.maturity);
    }

    dates
}

pub(crate) fn annualization_factor(inst: &FxVarianceSwap) -> f64 {
    if let Some(months) = inst.observation_freq.months() {
        return match months {
            1 => 12.0,
            3 => 4.0,
            6 => 2.0,
            12 => 1.0,
            _ => 252.0,
        };
    }
    if let Some(days) = inst.observation_freq.days() {
        return match days {
            1 => 252.0,
            7 => 52.0,
            14 => 26.0,
            _ => 252.0,
        };
    }
    252.0
}

pub(crate) fn realized_fraction_by_observations(inst: &FxVarianceSwap, as_of: Date) -> f64 {
    realized_fraction_by_observations_with_dates(inst, as_of, &observation_dates(inst))
}

/// Fraction of the observation period elapsed at `as_of`, measured by the
/// instrument's day-count convention.
///
/// This is the correct weight for blending already-annualized realized and
/// forward variance in a seasoned MTM (the accrued-variance identity weights
/// the un-annualized total variance by `t_elapsed/T` and `τ_remaining/T`).
/// Observation-count fractions only coincide for perfectly uniform schedules
/// and drift first-order near maturity for weekend-skipping daily schedules.
pub(crate) fn time_elapsed_fraction(inst: &FxVarianceSwap, as_of: Date) -> Result<f64> {
    if as_of <= inst.start_date {
        return Ok(0.0);
    }
    if as_of >= inst.maturity {
        return Ok(1.0);
    }
    let total =
        inst.day_count
            .year_fraction(inst.start_date, inst.maturity, DayCountContext::default())?;
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
    if as_of <= inst.start_date {
        return 0.0;
    }
    if as_of >= inst.maturity {
        return 1.0;
    }
    let total = all.len() as f64;
    let realized = all.iter().filter(|&&d| d <= as_of).count() as f64;
    (realized / total).clamp(0.0, 1.0)
}

pub(crate) fn get_historical_prices(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Vec<f64>> {
    get_historical_prices_with_dates(inst, context, as_of, &observation_dates(inst))
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
        if dates.len() >= 2 {
            return series.values_on(&dates);
        }
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
        finstack_core::Error::Validation(format!(
            "FxVarianceSwap '{inst_id}': 'open_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let high_id = inst.high_series_id.as_deref().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "FxVarianceSwap '{inst_id}': 'high_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let low_id = inst.low_series_id.as_deref().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "FxVarianceSwap '{inst_id}': 'low_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;

    let dates: Vec<Date> = obs_dates.iter().copied().filter(|&d| d <= as_of).collect();

    if dates.len() < 2 {
        return Ok((vec![], vec![], vec![], vec![]));
    }

    let open_vals = context.get_series(open_id)?.values_on(&dates)?;
    let high_vals = context.get_series(high_id)?.values_on(&dates)?;
    let low_vals = context.get_series(low_id)?.values_on(&dates)?;
    let close_vals = context.get_series(&default_close)?.values_on(&dates)?;

    Ok((open_vals, high_vals, low_vals, close_vals))
}

pub(crate) fn partial_realized_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    partial_realized_variance_with_dates(inst, context, as_of, &observation_dates(inst))
}

fn partial_realized_variance_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    if inst.realized_var_method.requires_ohlc() {
        let (open, high, low, close) =
            get_historical_ohlc_with_dates(inst, context, as_of, obs_dates)?;
        if close.len() < 2 {
            return Ok(0.0);
        }
        return finstack_core::math::stats::realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            inst.realized_var_method,
            annualization_factor(inst),
        );
    }
    let prices = get_historical_prices_with_dates(inst, context, as_of, obs_dates)?;
    if prices.len() < 2 {
        return Ok(0.0);
    }
    realized_variance(
        &prices,
        inst.realized_var_method,
        annualization_factor(inst),
    )
}

pub(crate) fn remaining_forward_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let t = inst
        .day_count
        .year_fraction(as_of, inst.maturity, DayCountContext::default())?;
    if t <= 0.0 {
        return Ok(0.0);
    }

    let spot = inst.spot_rate(context, as_of)?;
    let surface = context.get_surface(inst.vol_surface_id.as_str())?;
    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;
    let for_curve = context.get_discount(inst.foreign_discount_curve_id.as_str())?;
    let t_dom = dom
        .day_count()
        .year_fraction(as_of, inst.maturity, DayCountContext::default())?;
    let t_for =
        for_curve
            .day_count()
            .year_fraction(as_of, inst.maturity, DayCountContext::default())?;
    let df_dom = dom.df(t_dom.max(0.0));
    let df_for = for_curve.df(t_for.max(0.0));

    let r_d = zero_rate_from_df(df_dom, t, "FxVarianceSwap domestic discount")?;
    let r_f = zero_rate_from_df(df_for, t, "FxVarianceSwap foreign discount")?;
    let fwd = spot * ((r_d - r_f) * t).exp();
    let strikes = surface.strikes();
    {
        let vol_fn = |t_exp: f64, k: f64| surface.value_clamped(t_exp, k);
        let bs_fn =
            |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r_d, r_f, v, t, opt) };
        if let Some(variance) = carr_madan_forward_variance(strikes, fwd, r_d, t, vol_fn, bs_fn) {
            return Ok(variance);
        }
        // `debug` rather than `warn`: this branch fires on every PV when the
        // surface is sparse (a known steady-state, not an alert). Use the
        // `finstack.fx_variance_swap` target for selective enablement.
        tracing::debug!(
            target = "finstack.fx_variance_swap",
            instrument_id = %inst.id(),
            t,
            num_strikes = strikes.len(),
            "Carr-Madan forward-variance replication failed; falling back to ATM-vol²"
        );
    }

    let vol_atm = surface.value_clamped(t, fwd.max(1e-12));
    if vol_atm.is_finite() && vol_atm > 0.0 {
        tracing::debug!(
            target = "finstack.fx_variance_swap",
            instrument_id = %inst.id(),
            vol_atm,
            "Using ATM-vol² fallback for forward variance"
        );
        return Ok(vol_atm * vol_atm);
    }

    Err(finstack_core::Error::Calibration {
        message: format!(
            "FX variance swap '{}': both Carr-Madan replication and the ATM-vol² fallback \
             failed (vol_atm={vol_atm}). Cannot compute forward variance from the supplied \
             vol surface; check that the surface has sufficient strikes around the forward \
             {fwd} at maturity {t:.4}y.",
            inst.id()
        ),
        category: "fx_variance_swap_replication".to_string(),
    })
}

impl Default for SimpleFxVarianceSwapDiscountingPricer {
    fn default() -> Self {
        Self
    }
}

impl Pricer for SimpleFxVarianceSwapDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::FxVarianceSwap, ModelKey::Discounting)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swap = instrument
            .as_any()
            .downcast_ref::<FxVarianceSwap>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::FxVarianceSwap, instrument.key())
            })?;

        let pv = compute_pv(swap, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(swap.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
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

    #[test]
    fn fx_variance_swap_pricer_compute_pv_matches_instrument_value() {
        let swap = FxVarianceSwap::example();
        let as_of = date!(2025 - 01 - 02);
        let market = build_market(as_of);

        let via_pricer = compute_pv(&swap, &market, as_of).expect("pricer pv");
        let via_instrument = swap.value(&market, as_of).expect("instrument pv");

        assert_eq!(via_pricer, via_instrument);
    }

    /// W-32: the FX seasoned MTM must blend realized and forward variance by
    /// the day-count `time_elapsed_fraction`, not by observation count, which
    /// drifts for weekend-skipping daily schedules near maturity.
    #[test]
    fn fx_seasoned_mtm_uses_time_weighting_not_observation_count() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::fx::fx_variance_swap::types::PayReceive;
        use finstack_core::dates::{DayCount, Tenor};
        use finstack_core::market_data::scalars::ScalarTimeSeries;
        use finstack_core::market_data::surfaces::VolSurface;
        use finstack_core::math::stats::RealizedVarMethod;
        use finstack_core::types::{CurveId, InstrumentId};

        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 06 - 27); // Friday, near maturity

        let swap = FxVarianceSwap::builder()
            .id(InstrumentId::new("FXVAR-SEASONED"))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .spot_id("EURUSD".to_string())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .realized_var_method(RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .domestic_discount_curve_id(CurveId::new("USD-OIS"))
            .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
            .vol_surface_id(CurveId::new("EURUSD-VOL"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("seasoned fx swap");

        let count_w = realized_fraction_by_observations(&swap, as_of);
        let time_w = time_elapsed_fraction(&swap, as_of).expect("time fraction");
        assert!(
            (count_w - time_w).abs() > 1e-4,
            "schedule must make count weight ({count_w}) differ from time weight ({time_w})"
        );

        // Close series over every past observation date with a non-trivial
        // return path so realized variance != forward variance.
        let past: Vec<Date> = observation_dates(&swap)
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
        let expected_var = realized * time_w + forward * (1.0 - time_w);
        let dom = market.get_discount("USD-OIS").expect("curve");
        // Date-based discounting, matching the pricer (item 4).
        let df = dom
            .df_between_dates(as_of, swap.maturity)
            .expect("date-based df");
        let expected_pv = swap.payoff(expected_var) * df;

        assert!(
            (pv.amount() - expected_pv.amount()).abs() < 1e-6,
            "FX seasoned MTM must use time-weighted identity: pv={} expected={}",
            pv.amount(),
            expected_pv.amount()
        );

        let count_var = realized * count_w + forward * (1.0 - count_w);
        let count_pv = swap.payoff(count_var) * df;
        assert!(
            (pv.amount() - count_pv.amount()).abs() > 1e-6,
            "FX seasoned MTM must differ from observation-count weighting"
        );
    }

    /// Item 4 regression: the terminal PV discount must be date-based. When the
    /// discount curve's `base_date` precedes `as_of`, `dom.df(yf(as_of, mat))`
    /// reads the curve at the wrong point on its time axis; the correct factor
    /// is `dom.df_between_dates(as_of, maturity)`.
    #[test]
    fn fx_variance_swap_terminal_discount_is_date_based() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::fx::fx_variance_swap::types::PayReceive;
        use finstack_core::dates::{DayCount, Tenor};
        use finstack_core::market_data::surfaces::VolSurface;
        use finstack_core::math::stats::RealizedVarMethod;
        use finstack_core::types::{CurveId, InstrumentId};

        // Curve base date is well before the valuation date (a stale-but-valid
        // curve). `df(t)` is anchored at base_date, so feeding it
        // `yf(as_of, maturity)` mis-discounts.
        let curve_base = date!(2025 - 01 - 02);
        let as_of = date!(2025 - 07 - 01);
        let start = date!(2025 - 07 - 02);
        let maturity = date!(2026 - 01 - 02);

        let usd_curve = DiscountCurve::builder("USD-OIS")
            .base_date(curve_base)
            .knots([(0.0, 1.0), (2.0, (-0.06_f64).exp())])
            .build()
            .expect("usd curve");
        let eur_curve = DiscountCurve::builder("EUR-OIS")
            .base_date(curve_base)
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
            .spot_id("EURUSD".to_string())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
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
        let expected_pv = swap.payoff(forward) * df_correct;
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
