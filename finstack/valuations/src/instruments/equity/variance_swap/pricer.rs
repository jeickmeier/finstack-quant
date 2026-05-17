use crate::instruments::common_impl::models::closed_form::vanilla::bs_price;
use crate::instruments::common_impl::parameters::market::OptionType;
use crate::instruments::common_impl::pricing::variance_replication::carr_madan_forward_variance;
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::equity::variance_swap::VarianceSwap;

type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_core::{
    dates::Date, market_data::context::MarketContext, math::stats::realized_variance, money::Money,
    Result,
};

/// Registry-facing pricer for variance swaps.
pub struct SimpleVarianceSwapDiscountingPricer;

fn smile_convexity_adjusted_variance(
    surface: &finstack_core::market_data::surfaces::VolSurface,
    time_to_expiry: f64,
    forward: f64,
) -> Option<f64> {
    if !time_to_expiry.is_finite()
        || time_to_expiry <= 0.0
        || !forward.is_finite()
        || forward <= 0.0
    {
        return None;
    }

    let vol_atm = surface.value_clamped(time_to_expiry, forward);
    if !vol_atm.is_finite() || vol_atm <= 0.0 {
        return None;
    }
    let atm_variance = vol_atm * vol_atm;

    let strikes = surface.strikes();
    let lower = strikes.iter().copied().rfind(|&k| k < forward);
    let upper = strikes.iter().copied().find(|&k| k > forward);

    let (Some(k_lo), Some(k_hi)) = (lower, upper) else {
        return Some(atm_variance);
    };

    let vol_lo = surface.value_clamped(time_to_expiry, k_lo);
    let vol_hi = surface.value_clamped(time_to_expiry, k_hi);
    if !(vol_lo.is_finite() && vol_lo > 0.0 && vol_hi.is_finite() && vol_hi > 0.0) {
        return Some(atm_variance);
    }

    // Use the local butterfly in variance space as a cheap convexity proxy when
    // full Carr-Madan replication is unavailable.
    let wing_average_variance = 0.5 * (vol_lo * vol_lo + vol_hi * vol_hi);
    Some(atm_variance + (wing_average_variance - atm_variance).max(0.0))
}

pub(crate) fn compute_pv(
    inst: &VarianceSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if inst.strike_variance < 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "VarianceSwap strike_variance ({:.6}) must be non-negative",
            inst.strike_variance
        )));
    }

    inst.validate_as_of(curves, as_of)?;
    let disc = curves.get_discount(inst.discount_curve_id.as_str())?;

    if as_of >= inst.maturity {
        let realized_var = if inst.realized_var_method.requires_ohlc() {
            let (open, high, low, close) = get_historical_ohlc(inst, curves, as_of)?;
            if close.is_empty() {
                return Ok(Money::new(0.0, inst.notional.currency()));
            }
            finstack_core::math::stats::realized_variance_ohlc(
                &open,
                &high,
                &low,
                &close,
                inst.realized_var_method,
                annualization_factor_with_policy(inst, curves),
            )?
        } else {
            let prices = get_historical_prices(inst, curves, as_of)?;
            if prices.is_empty() {
                return Ok(Money::new(0.0, inst.notional.currency()));
            }
            realized_variance(
                &prices,
                inst.realized_var_method,
                annualization_factor_with_policy(inst, curves),
            )?
        };
        return Ok(inst.payoff(realized_var));
    }

    if as_of < inst.start_date {
        let forward_var = remaining_forward_variance(inst, curves, as_of)?;
        let undiscounted = inst.payoff(forward_var);
        let t = inst
            .day_count
            .year_fraction(as_of, inst.maturity, Default::default())?;
        return Ok(undiscounted * disc.df(t));
    }

    let realized = partial_realized_variance(inst, curves, as_of)?;
    let forward = remaining_forward_variance(inst, curves, as_of)?;
    // Seasoned MTM blends already-annualized realized and forward variance.
    // The accrued-variance identity time-weights the un-annualized total
    // variance: σ²_expected = (V_accrued + E[V_fwd]·τ) / T, which reduces to
    // realized·(t_elapsed/T) + forward·(τ_remaining/T). Use the day-count
    // `time_elapsed_fraction` rather than an observation-count fraction, which
    // only coincides for perfectly uniform schedules (W-32).
    let w = inst.time_elapsed_fraction(as_of);
    let expected_var = realized * w + forward * (1.0 - w);
    let undiscounted = inst.payoff(expected_var);
    let t = inst
        .day_count
        .year_fraction(as_of, inst.maturity, Default::default())?;
    Ok(undiscounted * disc.df(t))
}

pub(crate) fn observation_dates(inst: &VarianceSwap) -> Vec<Date> {
    use finstack_core::dates::DateExt;

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
        while current <= inst.maturity {
            if !matches!(
                current.weekday(),
                time::Weekday::Saturday | time::Weekday::Sunday
            ) {
                dates.push(current);
            }
            current += time::Duration::days(days_step as i64);
            if current > inst.maturity {
                break;
            }
        }
    } else {
        while current <= inst.maturity {
            dates.push(current);
            current += time::Duration::days(1);
            if current > inst.maturity {
                break;
            }
        }
    }

    if dates.is_empty() || dates.last() != Some(&inst.maturity) {
        dates.push(inst.maturity);
    }

    dates
}

pub(crate) fn annualization_factor(inst: &VarianceSwap) -> f64 {
    const TRADING_DAYS_PER_YEAR: f64 = 252.0;

    if let Some(months) = inst.observation_freq.months() {
        match months {
            1 => 12.0,
            3 => 4.0,
            6 => 2.0,
            12 => 1.0,
            _ => TRADING_DAYS_PER_YEAR,
        }
    } else if let Some(days) = inst.observation_freq.days() {
        match days {
            1 => TRADING_DAYS_PER_YEAR,
            7 => TRADING_DAYS_PER_YEAR / 7.0,
            14 => TRADING_DAYS_PER_YEAR / 14.0,
            _ => TRADING_DAYS_PER_YEAR / days as f64,
        }
    } else {
        TRADING_DAYS_PER_YEAR
    }
}

pub(crate) fn annualization_factor_with_policy(
    inst: &VarianceSwap,
    context: &MarketContext,
) -> f64 {
    let tdy_override = context
        .get_price(format!("{}_TRADING_DAYS_PER_YEAR", inst.underlying_ticker))
        .ok()
        .and_then(|s| match s {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => Some(*v),
            _ => None,
        })
        .or_else(|| {
            context
                .get_price("TRADING_DAYS_PER_YEAR")
                .ok()
                .and_then(|s| match s {
                    finstack_core::market_data::scalars::MarketScalar::Unitless(v) => Some(*v),
                    _ => None,
                })
        })
        .unwrap_or(252.0);

    if let Some(months) = inst.observation_freq.months() {
        return match months {
            1 => 12.0,
            3 => 4.0,
            6 => 2.0,
            12 => 1.0,
            _ => tdy_override,
        };
    }
    if let Some(days) = inst.observation_freq.days() {
        return match days {
            1 => tdy_override,
            7 => tdy_override / 7.0,
            14 => tdy_override / 14.0,
            _ => tdy_override / days as f64,
        };
    }
    tdy_override
}

pub(crate) fn realized_fraction_by_observations(inst: &VarianceSwap, as_of: Date) -> f64 {
    let all = observation_dates(inst);
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
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<Vec<f64>> {
    let close_id = inst
        .close_series_id
        .as_deref()
        .unwrap_or(&inst.underlying_ticker);
    let past_dates: Vec<Date> = observation_dates(inst)
        .into_iter()
        .filter(|&d| d <= as_of)
        .collect();

    if let Ok(series) = context.get_series(close_id) {
        if past_dates.len() >= 2 {
            return series.values_on(&past_dates);
        }
    }
    if let Ok(scalar) = context.get_price(&inst.underlying_ticker) {
        let spot = match scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(p) => p.amount(),
        };
        return Ok(vec![spot]);
    }

    // Only acceptable in two cases: (a) the swap hasn't accrued any past
    // observations yet, or (b) it has accrued ≤ 1, in which case realised
    // variance is 0 by definition. Otherwise this is a data-availability
    // failure and must error rather than silently mark the swap to zero
    // realised variance.
    if past_dates.len() >= 2 {
        return Err(finstack_core::Error::Validation(format!(
            "VarianceSwap '{}' has {} past observation dates but no historical price \
             data is available (looked for series '{}' and spot scalar '{}'). \
             Provide the time series to compute realised variance, or remove past \
             observations before pricing.",
            inst.id.as_str(),
            past_dates.len(),
            close_id,
            inst.underlying_ticker.as_str()
        )));
    }
    Ok(vec![])
}

/// Load aligned OHLC histories from the market context for OHLC-based estimators.
///
/// Returns `Err(Validation)` if any required series ID is missing.
pub(crate) fn get_historical_ohlc(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<OhlcVecs> {
    let default_close = inst
        .close_series_id
        .as_deref()
        .unwrap_or(&inst.underlying_ticker);

    let method_label = inst.realized_var_method.label();
    let inst_id = inst.id.as_str().to_owned();

    let open_id = inst.open_series_id.as_deref().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "VarianceSwap '{inst_id}': 'open_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let high_id = inst.high_series_id.as_deref().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "VarianceSwap '{inst_id}': 'high_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let low_id = inst.low_series_id.as_deref().ok_or_else(|| {
        finstack_core::Error::Validation(format!(
            "VarianceSwap '{inst_id}': 'low_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;

    let dates: Vec<Date> = observation_dates(inst)
        .into_iter()
        .filter(|&d| d <= as_of)
        .collect();

    if dates.len() < 2 {
        return Ok((vec![], vec![], vec![], vec![]));
    }

    let open_vals = context.get_series(open_id)?.values_on(&dates)?;
    let high_vals = context.get_series(high_id)?.values_on(&dates)?;
    let low_vals = context.get_series(low_id)?.values_on(&dates)?;
    let close_vals = context.get_series(default_close)?.values_on(&dates)?;

    Ok((open_vals, high_vals, low_vals, close_vals))
}

pub(crate) fn partial_realized_variance(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    if inst.realized_var_method.requires_ohlc() {
        let (open, high, low, close) = get_historical_ohlc(inst, context, as_of)?;
        if close.len() < 2 {
            return Ok(0.0);
        }
        return finstack_core::math::stats::realized_variance_ohlc(
            &open,
            &high,
            &low,
            &close,
            inst.realized_var_method,
            annualization_factor_with_policy(inst, context),
        );
    }
    let prices = get_historical_prices(inst, context, as_of)?;
    if prices.len() < 2 {
        return Ok(0.0);
    }
    realized_variance(
        &prices,
        inst.realized_var_method,
        annualization_factor_with_policy(inst, context),
    )
}

/// Forward (remaining) variance from `as_of` to `maturity`.
///
/// # Fallback cascade
///
/// Sourced market data is checked in priority order. Each successful step
/// **stops** the cascade and returns immediately. Lower-priority fallbacks
/// emit a `tracing::warn!` so operators can see the dispersion in market
/// data quality.
///
/// 1. **Carr–Madan replication** from a vol surface (preferred). Uses the
///    full smile via OTM put/call strip.
/// 2. **ATM variance + local-smile convexity** (`smile_convexity_adjusted_variance`).
///    Used when Carr-Madan can't replicate (e.g. sparse strikes); logged at WARN.
/// 3. **Scalar implied vol** under key `{ticker}_IMPL_VOL`. Crude — squared
///    to a flat variance; logged at WARN.
/// 4. **Strike variance from the contract itself** (`inst.strike_variance`).
///    This is a last-resort sentinel that prices the swap at zero (mark-to-market
///    equals strike). Logged at WARN with the instrument id so it's visible
///    in production logs.
pub(crate) fn remaining_forward_variance(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let t = inst
        .day_count
        .year_fraction(as_of, inst.maturity, Default::default())?;

    for sid in [
        inst.underlying_ticker.as_str(),
        &format!("{}_VOL", inst.underlying_ticker),
        &format!("{}_IMPL_VOL", inst.underlying_ticker),
    ] {
        if let Ok(surface) = context.get_surface(sid) {
            if let Ok(disc) = context.get_discount(&inst.discount_curve_id) {
                if let Ok(spot_scalar) = context.get_price(&inst.underlying_ticker) {
                    let spot = match spot_scalar {
                        finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                        finstack_core::market_data::scalars::MarketScalar::Price(p) => p.amount(),
                    };
                    let r = disc.zero(t.max(1e-8));
                    let q = context
                        .get_price(format!("{}-DIVYIELD", inst.underlying_ticker))
                        .ok()
                        .and_then(|s| match s {
                            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => {
                                Some(*v)
                            }
                            _ => None,
                        })
                        .unwrap_or(0.0);
                    let fwd = spot * ((r - q) * t).exp();
                    let strikes = surface.strikes();
                    if t > 0.0 {
                        let vol_fn = |t_exp: f64, k: f64| surface.value_clamped(t_exp, k);
                        let bs_fn = |k: f64, v: f64, opt: OptionType| -> f64 {
                            bs_price(spot, k, r, q, v, t, opt)
                        };
                        if let Some(variance) =
                            carr_madan_forward_variance(strikes, fwd, r, t, vol_fn, bs_fn)
                        {
                            return Ok(variance);
                        }
                    }
                    if let Some(fallback_variance) =
                        smile_convexity_adjusted_variance(&surface, t.max(1e-8), fwd)
                    {
                        let vol_atm = surface.value_clamped(t.max(1e-8), fwd);
                        tracing::warn!(
                            instrument_id = %inst.id,
                            surface_id = %sid,
                            vol_atm = vol_atm,
                            fallback_variance = fallback_variance,
                            "VarianceSwap forward variance: Carr-Madan replication failed; \
                             falling back to ATM variance plus local smile convexity (level 2/4)"
                        );
                        return Ok(fallback_variance);
                    }
                }
            }
        }
    }

    if let Ok(scalar) = context.get_price(format!("{}_IMPL_VOL", inst.underlying_ticker)) {
        let vol = match scalar {
            finstack_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_core::market_data::scalars::MarketScalar::Price(p) => p.amount(),
        };
        let fallback_variance = vol * vol;
        tracing::warn!(
            instrument_id = %inst.id,
            ticker = %inst.underlying_ticker,
            vol = vol,
            fallback_variance = fallback_variance,
            "VarianceSwap forward variance: no vol surface available; falling back to \
             scalar {ticker}_IMPL_VOL (level 3/4)",
            ticker = inst.underlying_ticker.as_str()
        );
        Ok(fallback_variance)
    } else {
        tracing::warn!(
            instrument_id = %inst.id,
            ticker = %inst.underlying_ticker,
            strike_variance = inst.strike_variance,
            "VarianceSwap forward variance: no surface or scalar implied vol found; \
             falling back to contract strike_variance (level 4/4 — swap will mark to zero)"
        );
        Ok(inst.strike_variance)
    }
}

impl Default for SimpleVarianceSwapDiscountingPricer {
    fn default() -> Self {
        Self
    }
}

impl Pricer for SimpleVarianceSwapDiscountingPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::VarianceSwap, ModelKey::Discounting)
    }

    #[tracing::instrument(
        name = "variance_swap.discounting.price_dyn",
        level = "debug",
        skip(self, instrument, market),
        fields(inst_id = %instrument.id(), as_of = %as_of),
        err,
    )]
    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let swap = instrument
            .as_any()
            .downcast_ref::<VarianceSwap>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::VarianceSwap, instrument.key())
            })?;

        let pv = compute_pv(swap, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(
                e.to_string(),
                PricingErrorContext::from_instrument(swap).model(ModelKey::Discounting),
            )
        })?;

        Ok(ValuationResult::stamped(swap.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    fn build_market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("curve");
        MarketContext::new().insert(curve)
    }

    #[test]
    fn variance_swap_pricer_compute_pv_matches_instrument_value() {
        // Use as_of strictly before start_date so the swap has no past
        // observations; this exercises the pricer/instrument parity check
        // without requiring historical price data.
        let swap = VarianceSwap::example().expect("example swap");
        let as_of = date!(2023 - 12 - 31); // before example's start_date 2024-01-01
        let market = build_market(as_of);

        let via_pricer = compute_pv(&swap, &market, as_of).expect("pricer pv");
        let via_instrument = swap.value(&market, as_of).expect("instrument pv");

        assert_eq!(via_pricer, via_instrument);
    }

    /// Regression: when the swap has accrued past observations but no
    /// historical price data is in the market context, the pricer must
    /// error rather than silently mark to zero realised variance.
    #[test]
    fn pricer_errors_when_past_observations_have_no_data() {
        let swap = VarianceSwap::example().expect("example swap");
        // as_of well after start_date so past_dates.len() >= 2
        let as_of = date!(2024 - 06 - 01);
        let market = build_market(as_of); // intentionally no series provided

        let err = compute_pv(&swap, &market, as_of)
            .expect_err("missing historical data must error, not silently mark to zero");
        let msg = err.to_string();
        assert!(
            msg.contains("no historical price data") || msg.contains("realised variance"),
            "expected explicit data-availability error, got: {}",
            msg
        );
    }

    /// W-32: a seasoned variance swap on a weekend-skipping daily schedule
    /// near maturity must blend realized and forward variance by the
    /// day-count `time_elapsed_fraction`, not by observation count. The two
    /// fractions diverge for non-uniform schedules and the MTM error is
    /// first-order near maturity.
    #[test]
    fn seasoned_mtm_uses_time_weighting_not_observation_count() {
        use finstack_core::dates::{DayCount, Tenor};
        use finstack_core::market_data::scalars::ScalarTimeSeries;
        use finstack_core::money::Money;
        use finstack_core::types::{CurveId, InstrumentId};
        use crate::instruments::equity::variance_swap::types::PayReceive;
        use crate::instruments::common_impl::traits::Attributes;

        // Weekend-skipping daily schedule, ~6 months, valued near maturity.
        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 06 - 27); // Friday, near maturity

        let swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-SEASONED"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(1_000_000.0, finstack_core::currency::Currency::USD))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .realized_var_method(finstack_core::math::stats::RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("seasoned swap");

        // The count fraction must genuinely diverge from the time fraction;
        // otherwise the test would not distinguish the two weightings.
        let count_w = realized_fraction_by_observations(&swap, as_of);
        let time_w = swap.time_elapsed_fraction(as_of);
        assert!(
            (count_w - time_w).abs() > 1e-4,
            "schedule must make count weight ({count_w}) differ from time weight ({time_w})"
        );

        // Close series on every past observation date with a non-trivial
        // return path so realized variance != forward variance (= strike, as
        // no vol surface is supplied).
        let past: Vec<Date> = observation_dates(&swap)
            .into_iter()
            .filter(|&d| d <= as_of)
            .collect();
        let obs: Vec<(Date, f64)> = past
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, 100.0 * (1.0 + 0.001 * (i as f64 % 3.0 - 1.0))))
            .collect();
        let series = ScalarTimeSeries::new("SPX", obs, None).expect("series");
        let market = build_market(as_of).insert_series(series);

        let pv = compute_pv(&swap, &market, as_of).expect("seasoned pv");

        // Recompute the identity from the same building blocks.
        let realized = partial_realized_variance(&swap, &market, as_of).expect("realized");
        let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
        let expected_var = realized * time_w + forward * (1.0 - time_w);
        let t = swap
            .day_count
            .year_fraction(as_of, swap.maturity, Default::default())
            .expect("yf");
        let disc = market.get_discount("USD-OIS").expect("curve");
        let expected_pv = swap.payoff(expected_var) * disc.df(t);

        assert!(
            (pv.amount() - expected_pv.amount()).abs() < 1e-6,
            "seasoned MTM must use time-weighted identity: pv={} expected={}",
            pv.amount(),
            expected_pv.amount()
        );

        // And it must NOT match the (wrong) observation-count weighting.
        let count_var = realized * count_w + forward * (1.0 - count_w);
        let count_pv = swap.payoff(count_var) * disc.df(t);
        assert!(
            (pv.amount() - count_pv.amount()).abs() > 1e-6,
            "seasoned MTM must differ from observation-count weighting"
        );
    }

    #[test]
    fn smile_convexity_fallback_lifts_forward_variance_above_atm_variance() {
        let surface = VolSurface::builder("SPX")
            .expiries(&[1.0])
            .strikes(&[90.0, 100.0, 110.0])
            .row(&[0.25, 0.20, 0.25])
            .build()
            .expect("surface");

        let fallback =
            smile_convexity_adjusted_variance(&surface, 1.0, 100.0).expect("fallback variance");

        assert!(
            fallback > 0.04,
            "expected smile correction above ATM variance"
        );
    }
}
