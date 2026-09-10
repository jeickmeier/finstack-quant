//! Pricing and metric helpers for equity instruments.
//!
use crate::instruments::common_impl::parameters::market::OptionType;
use crate::instruments::common_impl::pricing::variance_replication::carr_madan_forward_variance;
use crate::instruments::equity::variance_swap::VarianceSwap;
use finstack_quant_models::closed_form::vanilla::bs_price_unchecked;

type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
use finstack_quant_core::{
    dates::Date, market_data::context::MarketContext, math::stats::realized_variance, money::Money,
    Result,
};

pub(crate) fn compute_pv(
    inst: &VarianceSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if !inst.strike_variance.is_finite() || inst.strike_variance < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "VarianceSwap strike_variance ({:.6}) must be finite and non-negative",
            inst.strike_variance
        )));
    }

    inst.validate_as_of(curves, as_of)?;
    let disc = curves.get_discount(inst.discount_curve_id.as_str())?;
    let final_observation_date = inst.final_observation_date()?;
    let settlement_date = inst.effective_settlement_date()?;

    if as_of > settlement_date {
        return Ok(Money::from((0_i64, inst.notional.currency())));
    }

    if as_of >= final_observation_date {
        let realized_var = if inst.realized_var_method.requires_ohlc() {
            let (open, high, low, close) = get_historical_ohlc(inst, curves, as_of)?;
            if close.is_empty() {
                return Ok(Money::from((0_i64, inst.notional.currency())));
            }
            finstack_quant_core::math::stats::realized_variance_ohlc(
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
                return Ok(Money::from((0_i64, inst.notional.currency())));
            }
            realized_variance(
                &prices,
                inst.realized_var_method,
                annualization_factor_with_policy(inst, curves),
            )?
        };
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            settlement_date,
        )?;
        return Ok(inst.payoff(realized_var)? * df);
    }

    if as_of < inst.start_date {
        let forward_var = remaining_forward_variance(inst, curves, as_of)?;
        let undiscounted = inst.payoff(forward_var)?;
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            settlement_date,
        )?;
        return Ok(undiscounted * df);
    }

    // Seasoned mark-to-market: the observation-count blend of realized-to-date
    // and remaining forward variance. Shared with the `ExpectedVariance` metric via
    // `seasoned_expected_variance` so the reported metric can never drift from the
    // variance implied by this PV (W-32/W-33).
    let expected_var = seasoned_expected_variance(inst, curves, as_of)?;
    let undiscounted = inst.payoff(expected_var)?;
    let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
        disc.as_ref(),
        as_of,
        settlement_date,
    )?;
    Ok(undiscounted * df)
}

/// Expected annualized variance using the same sample-count basis as settlement.
/// The observed part uses the contractual estimator and annualization; remaining
/// samples use forward variance. A fully observed swap needs no volatility input.
pub(crate) fn seasoned_expected_variance(
    inst: &VarianceSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let w = realized_fraction_by_observations(inst, as_of)?;
    let realized = partial_realized_variance(inst, curves, as_of)?;
    if w >= 1.0 {
        return Ok(realized);
    }
    let forward = remaining_forward_variance(inst, curves, as_of)?;
    Ok(realized * w + forward * (1.0 - w))
}

pub(crate) fn observation_dates(inst: &VarianceSwap) -> Result<Vec<Date>> {
    crate::instruments::common_impl::pricing::variance_observations::variance_observation_dates(
        inst.start_date,
        inst.maturity,
        inst.observation_frequency,
        inst.observation_business_day_convention,
        inst.observation_end_of_month,
        crate::instruments::common_impl::pricing::variance_observations::VarianceCalendar::Single(
            &inst.observation_calendar_id,
        ),
    )
}

pub(crate) fn annualization_factor(inst: &VarianceSwap) -> f64 {
    use finstack_quant_core::dates::TenorUnit;
    const TRADING_DAYS_PER_YEAR: f64 = 252.0;

    if let Some(months) = inst.observation_frequency.months() {
        12.0 / months as f64
    } else if inst.observation_frequency.unit() == TenorUnit::Weeks {
        52.0 / f64::from(inst.observation_frequency.count())
    } else if inst.observation_frequency.unit() == TenorUnit::Days {
        TRADING_DAYS_PER_YEAR / f64::from(inst.observation_frequency.count())
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
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => Some(*v),
            finstack_quant_core::market_data::scalars::MarketScalar::Price(_) => None,
        })
        .or_else(|| {
            context
                .get_price("TRADING_DAYS_PER_YEAR")
                .ok()
                .and_then(|s| match s {
                    finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => {
                        Some(*v)
                    }
                    finstack_quant_core::market_data::scalars::MarketScalar::Price(_) => None,
                })
        })
        .unwrap_or(252.0);

    if let Some(months) = inst.observation_frequency.months() {
        return 12.0 / months as f64;
    }
    if inst.observation_frequency.unit() == finstack_quant_core::dates::TenorUnit::Weeks {
        return 52.0 / f64::from(inst.observation_frequency.count());
    }
    if inst.observation_frequency.unit() == finstack_quant_core::dates::TenorUnit::Days {
        return tdy_override / f64::from(inst.observation_frequency.count());
    }
    tdy_override
}

pub(crate) fn realized_fraction_by_observations(inst: &VarianceSwap, as_of: Date) -> Result<f64> {
    let all = observation_dates(inst)?;
    if all.is_empty() {
        return Ok(0.0);
    }
    if as_of < inst.start_date {
        return Ok(0.0);
    }
    if as_of >= all.last().copied().unwrap_or(inst.maturity) {
        return Ok(1.0);
    }
    // Close-to-close and Yang-Zhang need the first level as an anchor;
    // the other OHLC estimators count each bar as one sample.
    let anchor = usize::from(
        !inst.realized_var_method.requires_ohlc()
            || inst.realized_var_method
                == finstack_quant_core::math::stats::RealizedVarMethod::YangZhang,
    );
    let total = all.len().saturating_sub(anchor);
    let realized = all
        .iter()
        .filter(|&&d| d <= as_of)
        .count()
        .saturating_sub(anchor);
    if total == 0 {
        return Err(finstack_quant_core::Error::Validation(
            "variance swap has no return samples".into(),
        ));
    }
    Ok(realized as f64 / total as f64)
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
    let past_dates: Vec<Date> = observation_dates(inst)?
        .into_iter()
        .filter(|&d| d <= as_of)
        .collect();

    if let Ok(series) = context.get_series(close_id) {
        if past_dates.len() >= 2 {
            return past_dates
                .iter()
                .map(|&date| series.value_on_exact(date))
                .collect();
        }
    }
    if past_dates.len() >= 2 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "VarianceSwap '{}' has {} past observation dates but no historical price data is available in series '{}'. Provide the time series before pricing a seasoned swap.",
            inst.id.as_str(),
            past_dates.len(),
            close_id
        )));
    }
    if let Ok(scalar) = context.get_price(&inst.underlying_ticker) {
        let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
            scalar,
            inst.notional.currency(),
        )?;
        return Ok(vec![spot]);
    }

    // Only acceptable in two cases: (a) the swap hasn't accrued any past
    // observations yet, or (b) it has accrued ≤ 1, in which case realised
    // variance is 0 by definition. Otherwise this is a data-availability
    // failure and must error rather than silently mark the swap to zero
    // realised variance.
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
        finstack_quant_core::Error::Validation(format!(
            "VarianceSwap '{inst_id}': 'open_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let high_id = inst.high_series_id.as_deref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "VarianceSwap '{inst_id}': 'high_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;
    let low_id = inst.low_series_id.as_deref().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "VarianceSwap '{inst_id}': 'low_series_id' is required for \
             realized_var_method={method_label}. Set the corresponding *_series_id field."
        ))
    })?;

    let dates: Vec<Date> = observation_dates(inst)?
        .into_iter()
        .filter(|&d| d <= as_of)
        .collect();

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
    let close_vals = exact_values(default_close)?;

    Ok((open_vals, high_vals, low_vals, close_vals))
}

/// Realized variance over the elapsed window, annualized with an explicit
/// annualization factor.
///
/// Annualize the configured observed-sample estimator on the settlement basis.
fn realized_variance_with_factor(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
    annualization_factor: f64,
) -> Result<f64> {
    if inst.realized_var_method.requires_ohlc() {
        let (open, high, low, close) = get_historical_ohlc(inst, context, as_of)?;
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
    let prices = get_historical_prices(inst, context, as_of)?;
    if prices.len() < 2 {
        return Ok(0.0);
    }
    realized_variance(&prices, inst.realized_var_method, annualization_factor)
}

pub(crate) fn partial_realized_variance(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    realized_variance_with_factor(
        inst,
        context,
        as_of,
        annualization_factor_with_policy(inst, context),
    )
}

/// Minimum year-fraction below which the forward-start subtraction is skipped
/// (the pre-start gap or the accrual window is economically degenerate).
const FORWARD_START_MIN_T: f64 = 1e-6;

/// Forward (remaining) variance for the swap's accrual window.
///
/// For a live or seasoned swap (`as_of >= start_date`) this is the expected
/// variance over `[as_of, final_observation_date]`. For a FORWARD-STARTING
/// swap (`as_of < start_date`) variance accrues only from `start_date`, so the
/// spot-started replication over `[as_of, T]` must have the pre-start leg
/// removed via the total-variance identity (Demeterfi et al. 1999, forward
/// variance):
///
/// ```text
/// K²[start,T] = (t1·K²[as_of,T] − t0·K²[as_of,start]) / (t1 − t0)
/// ```
///
/// A negative result means the supplied term structure violates calendar
/// monotonicity and is rejected as an arbitrageable input.
pub(crate) fn remaining_forward_variance(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let final_observation_date = inst.final_observation_date()?;
    let var_to_end = spot_variance_to_date(inst, context, as_of, final_observation_date)?;

    if as_of >= inst.start_date {
        return Ok(var_to_end);
    }

    let t0 = inst
        .day_count
        .year_fraction(as_of, inst.start_date, Default::default())?;
    let t1 = inst
        .day_count
        .year_fraction(as_of, final_observation_date, Default::default())?;
    if t0 <= FORWARD_START_MIN_T || (t1 - t0) <= FORWARD_START_MIN_T {
        return Ok(var_to_end);
    }

    let var_to_start = spot_variance_to_date(inst, context, as_of, inst.start_date)?;
    let fwd = (var_to_end * t1 - var_to_start * t0) / (t1 - t0);
    if fwd < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "VarianceSwap '{}': forward variance is negative over [{}, {}]: \
             start_variance={var_to_start}, end_variance={var_to_end}, result={fwd}",
            inst.id, inst.start_date, final_observation_date
        )));
    }
    Ok(fwd)
}

/// Spot-started expected variance over `[as_of, target_date]`.
///
/// An explicit `market_quotes.implied_volatility` override is accepted as a
/// caller-selected flat-volatility assumption. Otherwise canonical pricing
/// requires a volatility surface with a strike grid sufficient for Carr–Madan
/// OTM option-strip replication; sparse surfaces and missing surfaces fail.
fn spot_variance_to_date(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
    target_date: Date,
) -> Result<f64> {
    if let Some(vol) = inst
        .instrument_pricing_overrides
        .market_quotes
        .implied_volatility
    {
        if !vol.is_finite() || vol <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "VarianceSwap '{}' implied-volatility override must be finite and positive, got {vol}",
                inst.id
            )));
        }
        return Ok(vol * vol);
    }

    let t = inst
        .day_count
        .year_fraction(as_of, target_date, Default::default())?;

    for sid in inst.volatility_candidate_ids() {
        if let Ok(surface) = context.get_surface(&sid) {
            let disc = context.get_discount(&inst.discount_curve_id)?;
            let spot = crate::instruments::common_impl::helpers::scalar_price_amount(
                context.get_price(&inst.underlying_ticker)?,
                inst.notional.currency(),
            )?;
            // Date-based zero rate over [as_of, target_date]: avoids the
            // axis bias of `disc.zero(t)` when curve base != as_of.
            let df_mat =
                crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                    disc.as_ref(),
                    as_of,
                    target_date,
                )?;
            let r = crate::instruments::common_impl::helpers::zero_rate_from_df(
                df_mat,
                t,
                "variance-swap replication rate",
            )?;
            let dividend_yield_id = inst.dividend_yield_scalar_id();
            let q = match context.get_price(&dividend_yield_id) {
                Ok(finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v)) => *v,
                Ok(finstack_quant_core::market_data::scalars::MarketScalar::Price(_)) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "variance-swap dividend yield '{}-DIVYIELD' must be unitless",
                        inst.underlying_ticker
                    )));
                }
                Err(error) => {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "variance-swap dividend yield '{}' is required for surface replication: {}",
                        dividend_yield_id, error
                    )));
                }
            };
            let fwd = spot / df_mat * (-q * t).exp();
            let strikes = surface.strikes();
            if t > 0.0 {
                let vol_fn = |t_exp: f64, k: f64| {
                    finstack_quant_models::volatility::get_surface_vol_clamped(&surface, t_exp, k)
                };
                let bs_fn = |k: f64, v: f64, opt: OptionType| -> f64 {
                    bs_price_unchecked(spot, k, r, q, v, t, opt)
                };
                if let Some(variance) =
                    carr_madan_forward_variance(strikes, fwd, r, t, vol_fn, bs_fn)
                {
                    return Ok(variance);
                }
            }
            return Err(finstack_quant_core::Error::Validation(format!(
                "variance-swap surface '{sid}' cannot support Carr-Madan replication; \
                 provide a wider OTM strike grid or an explicit implied-volatility override"
            )));
        }
    }
    Err(finstack_quant_core::InputError::NotFound {
        id: format!(
            "variance-swap volatility surface for '{}'; provide a replication-quality \
             surface or an explicit implied-volatility override",
            inst.underlying_ticker
        ),
    }
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use time::macros::date;

    fn build_market(as_of: Date) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("curve");
        let strikes = [50.0, 75.0, 90.0, 100.0, 110.0, 125.0, 150.0];
        let mut surface = VolSurface::builder("SPX")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&strikes);
        for _ in 0..4 {
            surface = surface.row(&[0.20; 7]);
        }
        MarketContext::new()
            .insert(curve)
            .insert_surface(surface.build().expect("surface"))
            .insert_price("SPX", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIVYIELD", MarketScalar::Unitless(0.0))
    }

    #[test]
    fn m18_final_observation_expected_variance_matches_settlement() {
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        let mut swap = VarianceSwap::example().expect("swap");
        swap.underlying_ticker = "SPX".into();
        swap.discount_curve_id = "USD-OIS".into();
        swap.start_date = date!(2025 - 01 - 06);
        swap.maturity = date!(2025 - 01 - 13);
        swap.instrument_pricing_overrides = swap.instrument_pricing_overrides.with_implied_vol(0.2);
        let dates = observation_dates(&swap).expect("dates");
        let final_date = *dates.last().expect("final observation");
        let prices: Vec<_> = dates
            .iter()
            .enumerate()
            .map(|(i, d)| (*d, 100.0 * (0.01 * i as f64).exp()))
            .collect();
        let market = build_market(final_date)
            .insert_series(ScalarTimeSeries::new("SPX", prices, None).expect("series"));
        let expected = partial_realized_variance(&swap, &market, final_date).expect("realized");
        let actual =
            seasoned_expected_variance(&swap, &market, final_date).expect("expected variance");
        assert!((actual - expected).abs() < 1e-12, "{actual} vs {expected}");
    }

    /// End-to-end wiring check of the EQUITY Carr-Madan path: a flat-in-strike,
    /// flat-in-time surface must replicate `K_var ≈ σ²` through
    /// `remaining_forward_variance`, with NONZERO rate and dividend yield so
    /// the equity-specific inputs (`fwd = spot/df·e^{−qt}`, `r = −ln df/t`,
    /// `bs_price(spot, k, r, q, …)`) are all exercised. A q-sign error, a
    /// spot/forward swap, or an inverted DF shifts the result far outside the
    /// tolerance. (The FX pricer shares the replication engine but not this
    /// wiring, so the FX test cannot catch equity-side regressions.)
    #[test]
    fn spot_started_flat_surface_replicates_sigma_squared_with_dividends() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        let as_of = date!(2025 - 01 - 02);
        let maturity = date!(2026 - 01 - 02);

        let swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-FLAT"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::from((
                1_000_000_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
            .strike_variance(0.04)
            .start_date(as_of)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .observation_calendar_id("USNY".to_string())
            .realized_var_method(finstack_quant_core::math::stats::RealizedVarMethod::CloseToClose)
            .price_series_policy(
                finstack_quant_valuations::instruments::EquityPriceSeriesPolicy::Adjusted,
            )
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("spot-started swap");

        let vol = 0.20_f64;
        let strikes: Vec<f64> = (4..=60).map(|i| 5.0 * i as f64).collect(); // 20..300
        let mut builder = VolSurface::builder("SPX")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&strikes);
        for _ in 0..4 {
            builder = builder.row(&vec![vol; strikes.len()]);
        }
        let surface = builder.build().expect("surface");

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.03_f64 * 2.0).exp())])
            .build()
            .expect("curve");
        let market = MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIVYIELD", MarketScalar::Unitless(0.015));

        let fair_var = remaining_forward_variance(&swap, &market, as_of).expect("fair variance");
        assert!(
            (fair_var - vol * vol).abs() < 5e-4,
            "flat-surface equity replication must give K_var ≈ σ² = {}: got {fair_var}",
            vol * vol
        );
    }

    /// A FORWARD-STARTING variance swap (`as_of < start_date`) accrues
    /// variance only over `[start_date, final_observation_date]`. The fair
    /// strike must therefore be the forward variance
    /// `(t1·K²[as_of,T] − t0·K²[as_of,start]) / (t1 − t0)` — not the
    /// spot-started `K²[as_of,T]`, which silently includes the pre-start
    /// window's volatility. The two coincide only for a flat vol term
    /// structure, which is why a term-structured surface is essential here.
    #[test]
    fn forward_starting_swap_excludes_pre_start_variance() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        let as_of = date!(2024 - 07 - 01);
        let start = date!(2025 - 01 - 02); // ~6 months forward (business day)
        let maturity = date!(2025 - 07 - 01); // 1y from as_of (business day)

        let swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-FWDSTART"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::from((
                1_000_000_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .observation_calendar_id("USNY".to_string())
            .realized_var_method(finstack_quant_core::math::stats::RealizedVarMethod::CloseToClose)
            .price_series_policy(
                finstack_quant_valuations::instruments::EquityPriceSeriesPolicy::Adjusted,
            )
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("forward-starting swap");

        // Term-structured, flat-in-strike surface: 30% vol to ~6m, 25% at 1y.
        // Total variance stays monotone (0.09·0.51 < 0.0625·1.0): no calendar
        // arbitrage, but a strongly downward-sloping forward vol.
        let strikes: Vec<f64> = (4..=60).map(|i| 5.0 * i as f64).collect(); // 20..300
        let vol_rows = [0.30_f64, 0.30, 0.25];
        let mut builder = VolSurface::builder("SPX")
            .expiries(&[0.25, 0.51, 1.0])
            .strikes(&strikes);
        for v in vol_rows {
            builder = builder.row(&vec![v; strikes.len()]);
        }
        let surface = builder.build().expect("surface");

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.03_f64 * 2.0).exp())])
            .build()
            .expect("curve");
        let market = MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price("SPX", MarketScalar::Unitless(100.0))
            .insert_price("SPX-DIVYIELD", MarketScalar::Unitless(0.0));

        let fwd = remaining_forward_variance(&swap, &market, as_of).expect("forward variance");

        // Independent expectation from the total-variance identity: both
        // sub-expiries sit in flat-in-t regions of the surface, so
        // K²[as_of,start] ≈ 0.30² and K²[as_of,T] ≈ 0.25² up to replication
        // discretization error.
        let t0 = DayCount::Act365F
            .year_fraction(as_of, start, Default::default())
            .expect("t0");
        let t1 = DayCount::Act365F
            .year_fraction(as_of, maturity, Default::default())
            .expect("t1");
        let expected = (0.25_f64.powi(2) * t1 - 0.30_f64.powi(2) * t0) / (t1 - t0);

        assert!(
            (fwd - expected).abs() < 2e-3,
            "forward-starting fair variance must exclude the pre-start window: \
             got {fwd}, expected ~{expected} (spot-started K²[as_of,T] would be ~0.0625)"
        );
        // And it must NOT be the spot-started variance.
        assert!(
            (fwd - 0.0625).abs() > 0.02,
            "forward variance ({fwd}) must differ from the spot-started total 0.0625"
        );
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

    #[test]
    fn missing_forward_volatility_is_a_pricing_error() {
        let swap = VarianceSwap::example().expect("example swap");
        let as_of = date!(2023 - 12 - 31);
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .knots([(0.0, 1.0), (1.0, (-0.03_f64).exp())])
            .build()
            .expect("curve");
        let market = MarketContext::new().insert(curve);

        let err = remaining_forward_variance(&swap, &market, as_of)
            .expect_err("missing volatility must not manufacture a zero mark");
        assert!(err.to_string().contains("volatility"));
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
    /// settlement observation count. The two
    /// fractions diverge for non-uniform schedules and the MTM error is
    /// first-order near maturity.
    #[test]
    fn seasoned_mtm_uses_settlement_observation_weighting() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        // Weekend-skipping daily schedule, ~6 months, valued near maturity.
        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 06 - 27); // Friday, near maturity

        let swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-SEASONED"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::from((
                1_000_000_i64,
                finstack_quant_core::currency::Currency::USD,
            )))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_frequency(Tenor::daily())
            .observation_calendar_id("USNY".to_string())
            .realized_var_method(finstack_quant_core::math::stats::RealizedVarMethod::CloseToClose)
            .price_series_policy(
                finstack_quant_valuations::instruments::EquityPriceSeriesPolicy::Adjusted,
            )
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("seasoned swap");

        // The count fraction must genuinely diverge from the time fraction;
        // otherwise the test would not distinguish the two weightings.
        let count_w =
            realized_fraction_by_observations(&swap, as_of).expect("observation fraction");
        let time_w = swap.time_elapsed_fraction(as_of);
        assert!(
            (count_w - time_w).abs() > 1e-4,
            "schedule must make count weight ({count_w}) differ from time weight ({time_w})"
        );

        // Close series on every past observation date with a non-trivial
        // return path so realized variance differs from the scalar-vol forward
        // variance supplied by `build_market`.
        let past: Vec<Date> = observation_dates(&swap)
            .expect("observation schedule")
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

        let realized = partial_realized_variance(&swap, &market, as_of).expect("realized");
        let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
        let expected_var = realized * count_w + forward * (1.0 - count_w);
        let disc = market.get_discount("USD-OIS").expect("curve");
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            swap.maturity,
        )
        .expect("df");
        let expected_pv = swap.payoff(expected_var).expect("valid payoff") * df;

        assert!(
            (pv.amount() - expected_pv.amount()).abs() < 1e-6,
            "seasoned MTM must use settlement observation weighting: pv={} expected={}",
            pv.amount(),
            expected_pv.amount()
        );

        let time_var = realized * time_w + forward * (1.0 - time_w);
        let time_pv = swap.payoff(time_var).expect("payoff") * df;
        assert!((pv.amount() - time_pv.amount()).abs() > 1e-6);
    }

    /// Weekly and biweekly sampling counts calendar weeks rather than trading-day
    /// observations. Their annualization bases must preserve that distinction.
    #[test]
    fn weekly_and_biweekly_annualization_uses_calendar_observation_counts() {
        use finstack_quant_core::dates::{Tenor, TenorUnit};

        let mut swap = VarianceSwap::example().expect("example swap");

        swap.observation_frequency = Tenor::new(1, TenorUnit::Weeks).expect("valid tenor fixture");
        assert_eq!(annualization_factor(&swap), 52.0);

        swap.observation_frequency = Tenor::new(2, TenorUnit::Weeks).expect("valid tenor fixture");
        assert_eq!(annualization_factor(&swap), 26.0);

        swap.observation_frequency = Tenor::new(1, TenorUnit::Days).expect("valid tenor fixture");
        assert_eq!(annualization_factor(&swap), 252.0);

        // The policy-aware variant must agree (no TRADING_DAYS_PER_YEAR
        // override in this market context).
        let market = MarketContext::new();
        swap.observation_frequency = Tenor::new(7, TenorUnit::Days).expect("valid tenor fixture");
        assert_eq!(annualization_factor_with_policy(&swap, &market), 36.0);
        swap.observation_frequency = Tenor::new(14, TenorUnit::Days).expect("valid tenor fixture");
        assert_eq!(annualization_factor_with_policy(&swap, &market), 18.0);

        swap.start_date = date!(2025 - 01 - 03); // Friday
        swap.maturity = date!(2025 - 01 - 15);
        swap.observation_frequency = Tenor::new(2, TenorUnit::Days).expect("valid tenor fixture");
        let dates = observation_dates(&swap).expect("observation schedule");
        assert_eq!(dates[0], date!(2025 - 01 - 03));
        assert_eq!(dates[1], date!(2025 - 01 - 07));
        assert!(dates
            .iter()
            .all(|d| !matches!(d.weekday(), time::Weekday::Saturday | time::Weekday::Sunday)));
    }
}
