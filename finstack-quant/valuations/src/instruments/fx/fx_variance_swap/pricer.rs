use crate::instruments::common_impl::helpers::zero_rate_from_df;
use crate::instruments::common_impl::parameters::OptionType;
use crate::instruments::common_impl::pricing::variance_replication::carr_madan_forward_variance;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::fx_variance_swap::FxVarianceSwap;
use crate::models::bs_price;

type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
use finstack_quant_core::{
    dates::{Date, DayCountContext},
    market_data::context::MarketContext,
    math::stats::realized_variance,
    money::Money,
    Result,
};

pub(crate) fn compute_pv(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    inst.validate_as_of(context, as_of)?;

    let dom = context.get_discount(inst.domestic_discount_curve_id.as_str())?;

    // Compute observation dates once per pricing call. Each branch below would
    // otherwise rebuild this 1-3 times via the helper functions.
    let obs_dates = observation_dates(inst)?;
    let final_observation_date = obs_dates.last().copied().unwrap_or(inst.maturity);

    if as_of >= final_observation_date {
        let realized_var = if inst.realized_var_method.requires_ohlc() {
            let (open, high, low, close) =
                get_historical_ohlc_with_dates(inst, context, as_of, &obs_dates)?;
            if close.is_empty() {
                return Ok(Money::new(0.0, inst.notional.currency()));
            }
            finstack_quant_core::math::stats::realized_variance_ohlc(
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
        let df = dom.df_between_dates(as_of, final_observation_date)?;
        return Ok(undiscounted * df);
    }

    let expected_var = seasoned_expected_variance_with_dates(inst, context, as_of, &obs_dates)?;
    let undiscounted = inst.payoff(expected_var);
    // Date-based discounting (see the pre-start branch above): `df_between_dates`
    // resolves the year fraction on the curve's own time axis.
    let df = dom.df_between_dates(as_of, final_observation_date)?;
    Ok(undiscounted * df)
}

pub(crate) fn observation_dates(inst: &FxVarianceSwap) -> Result<Vec<Date>> {
    crate::instruments::common_impl::pricing::variance_observations::variance_observation_dates(
        inst.start_date,
        inst.maturity,
        inst.observation_freq,
        inst.observation_bdc,
        inst.observation_end_of_month,
        crate::instruments::common_impl::pricing::variance_observations::VarianceCalendar::Joint {
            base: &inst.base_calendar_id,
            quote: &inst.quote_calendar_id,
        },
    )
}

pub(crate) fn annualization_factor(inst: &FxVarianceSwap) -> f64 {
    use finstack_quant_core::dates::TenorUnit;
    if let Some(months) = inst.observation_freq.months() {
        return 12.0 / months as f64;
    }
    if inst.observation_freq.unit == TenorUnit::Weeks {
        return 52.0 / f64::from(inst.observation_freq.count);
    }
    if inst.observation_freq.unit == TenorUnit::Days {
        return 252.0 / f64::from(inst.observation_freq.count);
    }
    252.0
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
/// This is the correct weight for blending already-annualized realized and
/// forward variance in a seasoned MTM (the accrued-variance identity weights
/// the un-annualized total variance by `t_elapsed/T` and `τ_remaining/T`).
/// Observation-count fractions only coincide for perfectly uniform schedules
/// and drift first-order near maturity for weekend-skipping daily schedules.
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
    if as_of <= inst.start_date {
        return 0.0;
    }
    if as_of >= all.last().copied().unwrap_or(inst.maturity) {
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
        if dates.len() >= 2 {
            return dates
                .iter()
                .map(|&date| series.value_on_exact(date))
                .collect();
        }
    }

    let accrued = obs_dates.iter().filter(|&&date| date <= as_of).count();
    if accrued >= 2 {
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

    if dates.len() < 2 {
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
        if close.len() < 2 {
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

/// Number of per-period samples (return periods or OHLC bars) accrued by
/// `as_of`.
fn realized_sample_count(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    if inst.realized_var_method.requires_ohlc() {
        let (_, _, _, close) = get_historical_ohlc_with_dates(inst, context, as_of, obs_dates)?;
        Ok((close.len() as f64).max(0.0))
    } else {
        let prices = get_historical_prices_with_dates(inst, context, as_of, obs_dates)?;
        Ok((prices.len() as f64 - 1.0).max(0.0))
    }
}

/// Realized variance for the seasoned mark-to-market blend, annualized on the
/// **day-count time basis** (`V_accrued / t_elapsed`) so it is consistent with
/// the day-count blend weight `w` (mirrors the equity W-33 fix).
///
/// [`partial_realized_variance`] annualizes on an observation-count basis
/// (`Σr²/N · AF`), which disagrees with the day-count basis for non-uniform
/// schedules. Re-basing with `AF = M / t_elapsed` (M = accrued sample count)
/// yields exactly `V_accrued / t_elapsed` for both close-to-close and OHLC
/// estimators. Degenerate windows (no time or samples accrued) fall back to
/// the observation-count annualization.
pub(crate) fn seasoned_realized_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    t_elapsed: f64,
    obs_dates: &[Date],
) -> Result<f64> {
    let m = realized_sample_count(inst, context, as_of, obs_dates)?;
    if t_elapsed > 0.0 && m > 0.0 {
        realized_variance_with_factor(inst, context, as_of, obs_dates, m / t_elapsed)
    } else {
        partial_realized_variance_with_dates(inst, context, as_of, obs_dates)
    }
}

/// Seasoned mark-to-market expected variance: the day-count time-weighted
/// blend of realized-to-date and remaining forward variance (W-32), with the
/// realized term annualized on the same day-count basis (W-33, mirroring the
/// equity sibling).
///
/// `compute_pv` and the `ExpectedVariance` metric both call this, so the
/// reported expected variance always equals the variance implied by the
/// swap's PV.
pub(crate) fn seasoned_expected_variance(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    seasoned_expected_variance_with_dates(inst, context, as_of, &observation_dates(inst)?)
}

fn seasoned_expected_variance_with_dates(
    inst: &FxVarianceSwap,
    context: &MarketContext,
    as_of: Date,
    obs_dates: &[Date],
) -> Result<f64> {
    let w = time_elapsed_fraction(inst, as_of)?;
    let total_t =
        inst.day_count
            .year_fraction(inst.start_date, inst.maturity, DayCountContext::default())?;
    let t_elapsed = w * total_t;
    let realized = seasoned_realized_variance(inst, context, as_of, t_elapsed, obs_dates)?;
    let forward = remaining_forward_variance(inst, context, as_of)?;
    Ok(realized * w + forward * (1.0 - w))
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
    let df_dom = dom.df_between_dates(as_of, inst.maturity)?;
    let df_for = for_curve.df_between_dates(as_of, inst.maturity)?;

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
        // `finstack_quant.fx_variance_swap` target for selective enablement.
        tracing::debug!(
            target = "finstack_quant.fx_variance_swap",
            instrument_id = %inst.id(),
            t,
            num_strikes = strikes.len(),
            "Carr-Madan forward-variance replication failed; falling back to ATM-vol²"
        );
    }

    let vol_atm = surface.value_clamped(t, fwd.max(1e-12));
    if vol_atm.is_finite() && vol_atm > 0.0 {
        tracing::debug!(
            target = "finstack_quant.fx_variance_swap",
            instrument_id = %inst.id(),
            vol_atm,
            "Using ATM-vol² fallback for forward variance"
        );
        return Ok(vol_atm * vol_atm);
    }

    Err(finstack_quant_core::Error::Calibration {
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

    #[test]
    fn fx_variance_swap_pricer_compute_pv_matches_instrument_value() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        let swap = FxVarianceSwap::example();
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

        let mut swap = FxVarianceSwap::example();
        swap.start_date = date!(2025 - 01 - 03); // Friday
        swap.maturity = date!(2025 - 01 - 15);
        swap.observation_freq = Tenor::new(2, TenorUnit::Days);

        let dates = observation_dates(&swap).expect("observation schedule");
        assert_eq!(annualization_factor(&swap), 126.0);
        assert_eq!(dates[0], date!(2025 - 01 - 03));
        assert_eq!(dates[1], date!(2025 - 01 - 07));
        assert!(dates
            .iter()
            .all(|d| !matches!(d.weekday(), time::Weekday::Saturday | time::Weekday::Sunday)));

        swap.observation_freq = Tenor::new(2, TenorUnit::Weeks);
        assert_eq!(annualization_factor(&swap), 26.0);
    }

    /// W-32: the FX seasoned MTM must blend realized and forward variance by
    /// the day-count `time_elapsed_fraction`, not by observation count, which
    /// drifts for weekend-skipping daily schedules near maturity.
    #[test]
    fn fx_seasoned_mtm_uses_time_weighting_not_observation_count() {
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
            .spot_id("EURUSD".to_string())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
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

        // Recompute the identity from the same building blocks. The realized
        // term must be on the day-count time basis (`seasoned_realized_variance`,
        // W-33) so it is consistent with the day-count blend weight.
        let total_t = swap
            .day_count
            .year_fraction(swap.start_date, swap.maturity, DayCountContext::default())
            .expect("total yf");
        let t_elapsed = time_w * total_t;
        let realized = seasoned_realized_variance(
            &swap,
            &market,
            as_of,
            t_elapsed,
            &observation_dates(&swap).expect("observation schedule"),
        )
        .expect("realized");
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
        // Vol surface has a strong strike slope ([0.9→30%, 1.3→5%]) so ATM-vol²
        // is noticeably different at the two forwards.  Two-strike surface forces
        // Carr-Madan to return None → ATM-vol² fallback is used.
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
        // Two-strike surface: forces Carr-Madan fallback to ATM-vol².
        // Strong slope: 30% at 0.9, 5% at 1.3 → ATM vol is sensitive to forward.
        let surface = VolSurface::builder("EURUSD-VOL")
            .expiries(&[1.0])
            .strikes(&[0.9, 1.3])
            .row(&[0.30, 0.05])
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
            .spot_id("EURUSD".to_string())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
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
        // With a 3-strike surface the Carr-Madan replication falls back to ATM vol².
        // ATM vol is looked up at (t, fwd) — so the forward choice determines variance.
        // We assert that remaining_forward_variance matches the date-based result.
        let surface_ref = market.get_surface("EURUSD-VOL").expect("surface");
        let vol_expected = surface_ref.value_clamped(t, fwd_expected.max(1e-12));
        let expected_variance = vol_expected * vol_expected;

        let vol_bug = surface_ref.value_clamped(t, fwd_bug.max(1e-12));
        let bug_variance = vol_bug * vol_bug;

        let actual = remaining_forward_variance(&swap, &market, as_of)
            .expect("forward variance must succeed");

        // If the surface is flat the vol lookup isn't fwd-sensitive — skip the
        // forward-dependence check and just assert the call succeeds.
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
            .base_calendar_id("TARGET2".to_string())
            .quote_calendar_id("USNY".to_string())
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
