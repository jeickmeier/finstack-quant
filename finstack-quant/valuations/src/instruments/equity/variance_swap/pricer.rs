use crate::instruments::common_impl::parameters::market::OptionType;
use crate::instruments::common_impl::pricing::variance_replication::carr_madan_forward_variance;
use crate::instruments::equity::variance_swap::VarianceSwap;
use crate::models::closed_form::vanilla::bs_price;

type OhlcVecs = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);
use finstack_quant_core::{
    dates::Date, market_data::context::MarketContext, math::stats::realized_variance, money::Money,
    Result,
};

/// Degraded fallback estimate of forward variance from a vol surface when full
/// Carr–Madan replication is unavailable (W-39).
///
/// # This is a DEGRADED fallback
///
/// The fair variance of a variance swap is the Carr–Madan strip
/// `(2/T)·∫ V(K)/K² dK` over the *whole* OTM smile. When that replication
/// cannot be evaluated (e.g. too few strikes) this function returns a proxy —
/// it is **not** an exact fair variance and the caller logs a WARN diagnostic
/// whenever it is used.
///
/// # Method
///
/// The previous proxy used only the two strikes immediately bracketing the
/// forward, which badly under-weights wing convexity: the variance-replication
/// kernel `1/K²` places material weight on *deep* OTM strikes that a 2-strike
/// butterfly ignores entirely.
///
/// This version instead aggregates `σ(K)²` across the **entire** available
/// strike grid, weighted by the variance-replication kernel `w(K) ∝ 1/K²`
/// (the leading term of the Carr–Madan integrand). That captures the full wing
/// span and the smile's convexity far better than two strikes, while remaining
/// a cheap closed-form proxy. The result is floored at the ATM variance so the
/// fallback never reports *less* variance than a flat-ATM assumption.
fn smile_convexity_adjusted_variance(
    surface: &finstack_quant_core::market_data::surfaces::VolSurface,
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
    if strikes.is_empty() {
        return Some(atm_variance);
    }

    // 1/K²-weighted average of σ(K)² across the full strike grid. The 1/K²
    // kernel is the leading term of the Carr–Madan variance-replication
    // integrand, so this weights the wings the way the fair-variance integral
    // does — heavily on low strikes (downside puts), capturing skew convexity.
    let mut weighted_var = 0.0;
    let mut weight_sum = 0.0;
    for &k in strikes {
        if !k.is_finite() || k <= 0.0 {
            continue;
        }
        let vol = surface.value_clamped(time_to_expiry, k);
        if !vol.is_finite() || vol <= 0.0 {
            continue;
        }
        let w = 1.0 / (k * k);
        weighted_var += w * vol * vol;
        weight_sum += w;
    }

    if weight_sum <= 0.0 {
        return Some(atm_variance);
    }

    let smile_variance = weighted_var / weight_sum;
    // Never report less than the flat-ATM variance: the smile (skew/convexity)
    // can only add fair variance relative to a flat ATM assumption.
    Some(smile_variance.max(atm_variance))
}

pub(crate) fn compute_pv(
    inst: &VarianceSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<Money> {
    if inst.strike_variance < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "VarianceSwap strike_variance ({:.6}) must be non-negative",
            inst.strike_variance
        )));
    }

    inst.validate_as_of(curves, as_of)?;
    let disc = curves.get_discount(inst.discount_curve_id.as_str())?;
    let final_observation_date = observation_dates(inst)?
        .last()
        .copied()
        .unwrap_or(inst.maturity);

    if as_of >= final_observation_date {
        let realized_var = if inst.realized_var_method.requires_ohlc() {
            let (open, high, low, close) = get_historical_ohlc(inst, curves, as_of)?;
            if close.is_empty() {
                return Ok(Money::new(0.0, inst.notional.currency()));
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
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            final_observation_date,
        )?;
        return Ok(undiscounted * df);
    }

    // Seasoned mark-to-market: the day-count time-weighted blend of realized-to-date
    // and remaining forward variance. Shared with the `ExpectedVariance` metric via
    // `seasoned_expected_variance` so the reported metric can never drift from the
    // variance implied by this PV (W-32/W-33).
    let expected_var = seasoned_expected_variance(inst, curves, as_of)?;
    let undiscounted = inst.payoff(expected_var);
    let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
        disc.as_ref(),
        as_of,
        final_observation_date,
    )?;
    Ok(undiscounted * df)
}

/// Seasoned mark-to-market expected variance: the day-count time-weighted blend
/// of realized-to-date and remaining forward variance.
///
/// Used for a partially-observed swap (`start_date <= as_of < maturity`). Both
/// the realized term and the blend weight `w = time_elapsed_fraction` are on the
/// **day-count time basis**, so the accrued-variance identity
/// `σ²_expected = (V_accrued + E[V_fwd]·τ) / T` closes exactly. The realized term
/// therefore uses [`seasoned_realized_variance`] (`V_accrued / t_elapsed`), not
/// [`partial_realized_variance`] (observation-count annualization), which would
/// disagree for non-uniform schedules (W-33).
///
/// `compute_pv` and the `ExpectedVariance` metric both call this, guaranteeing the
/// reported expected variance always equals the variance implied by the swap's PV.
pub(crate) fn seasoned_expected_variance(
    inst: &VarianceSwap,
    curves: &MarketContext,
    as_of: Date,
) -> Result<f64> {
    let forward = remaining_forward_variance(inst, curves, as_of)?;
    let final_observation_date = observation_dates(inst)?
        .last()
        .copied()
        .unwrap_or(inst.maturity);
    let total_t = inst.day_count.year_fraction(
        inst.start_date,
        final_observation_date,
        Default::default(),
    )?;
    let w = if as_of <= inst.start_date {
        0.0
    } else if as_of >= final_observation_date || total_t <= 0.0 {
        1.0
    } else {
        (inst
            .day_count
            .year_fraction(inst.start_date, as_of, Default::default())?
            / total_t)
            .clamp(0.0, 1.0)
    };
    let t_elapsed = w * total_t;
    let realized = seasoned_realized_variance(inst, curves, as_of, t_elapsed)?;
    Ok(realized * w + forward * (1.0 - w))
}

pub(crate) fn observation_dates(inst: &VarianceSwap) -> Result<Vec<Date>> {
    crate::instruments::common_impl::pricing::variance_observations::variance_observation_dates(
        inst.start_date,
        inst.maturity,
        inst.observation_freq,
        inst.observation_bdc,
        inst.observation_end_of_month,
        crate::instruments::common_impl::pricing::variance_observations::VarianceCalendar::Single(
            &inst.observation_calendar_id,
        ),
    )
}

pub(crate) fn annualization_factor(inst: &VarianceSwap) -> f64 {
    use finstack_quant_core::dates::TenorUnit;
    const TRADING_DAYS_PER_YEAR: f64 = 252.0;

    if let Some(months) = inst.observation_freq.months() {
        12.0 / months as f64
    } else if inst.observation_freq.unit == TenorUnit::Weeks {
        52.0 / f64::from(inst.observation_freq.count)
    } else if inst.observation_freq.unit == TenorUnit::Days {
        TRADING_DAYS_PER_YEAR / f64::from(inst.observation_freq.count)
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

    if let Some(months) = inst.observation_freq.months() {
        return 12.0 / months as f64;
    }
    if inst.observation_freq.unit == finstack_quant_core::dates::TenorUnit::Weeks {
        return 52.0 / f64::from(inst.observation_freq.count);
    }
    if inst.observation_freq.unit == finstack_quant_core::dates::TenorUnit::Days {
        return tdy_override / f64::from(inst.observation_freq.count);
    }
    tdy_override
}

pub(crate) fn realized_fraction_by_observations(inst: &VarianceSwap, as_of: Date) -> Result<f64> {
    let all = observation_dates(inst)?;
    if all.is_empty() {
        return Ok(0.0);
    }
    if as_of <= inst.start_date {
        return Ok(0.0);
    }
    if as_of >= all.last().copied().unwrap_or(inst.maturity) {
        return Ok(1.0);
    }
    let total = all.len() as f64;
    let realized = all.iter().filter(|&&d| d <= as_of).count() as f64;
    Ok((realized / total).clamp(0.0, 1.0))
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
        let spot = match scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(p) => p.amount(),
        };
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
/// Both close-to-close and the OHLC estimators compute a per-period variance
/// (mean over the elapsed sample) and multiply by `annualization_factor`. The
/// factor therefore selects the *time basis* of the annualization. With the
/// observation-frequency factor (~252 for daily) the result is annualized on
/// an observation-count basis; with `M / t_elapsed` (M = number of return
/// periods / OHLC bars, `t_elapsed` in years) it is annualized on a day-count
/// time basis instead — see [`seasoned_realized_variance`].
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

/// Number of per-period samples (return periods or OHLC bars) accrued by
/// `as_of`. Used to convert an observation-count annualization to a time-basis
/// annualization without re-deriving the squared-return sum.
fn realized_sample_count(inst: &VarianceSwap, context: &MarketContext, as_of: Date) -> Result<f64> {
    if inst.realized_var_method.requires_ohlc() {
        let (_, _, _, close) = get_historical_ohlc(inst, context, as_of)?;
        // OHLC estimators average over the number of bars.
        Ok((close.len() as f64).max(0.0))
    } else {
        let prices = get_historical_prices(inst, context, as_of)?;
        // Close-to-close averages over the number of returns = points − 1.
        Ok((prices.len() as f64 - 1.0).max(0.0))
    }
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

/// Realized variance for the seasoned mark-to-market blend, annualized on the
/// **day-count time basis** so it is consistent with the blend weight `w`.
///
/// `compute_pv` blends `realized·w + forward·(1−w)` with `w` the day-count
/// `time_elapsed_fraction`. The accrued-variance identity
/// `σ²_expected = (V_accrued + E[V_fwd]·τ) / T` requires `realized·w` to equal
/// `V_accrued / T`, i.e. `realized = V_accrued / t_elapsed`.
/// [`partial_realized_variance`] instead annualizes `V_accrued` on an
/// observation-count basis (`Σr²/N · AF`, AF ≈ 252), so the two time bases
/// disagree and the identity does not close for non-uniform schedules.
///
/// This function re-bases the annualization: it annualizes with
/// `AF = M / t_elapsed` (M = accrued sample count), which yields exactly
/// `V_accrued / t_elapsed` for both close-to-close and OHLC estimators. When
/// the elapsed time or sample count is degenerate (≤ 0), it falls back to the
/// observation-count annualization.
pub(crate) fn seasoned_realized_variance(
    inst: &VarianceSwap,
    context: &MarketContext,
    as_of: Date,
    t_elapsed: f64,
) -> Result<f64> {
    let m = realized_sample_count(inst, context, as_of)?;
    if t_elapsed > 0.0 && m > 0.0 {
        // AF = M / t_elapsed turns the per-period mean (÷M) into Σ(·)/t_elapsed.
        realized_variance_with_factor(inst, context, as_of, m / t_elapsed)
    } else {
        // Degenerate window: nothing meaningful accrued — fall back.
        partial_realized_variance(inst, context, as_of)
    }
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
///
/// If none of these market inputs exists, pricing fails. Substituting the
/// contract strike variance would manufacture a plausible zero mark.
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
            let disc = context.get_discount(&inst.discount_curve_id)?;
            let spot_scalar = context.get_price(&inst.underlying_ticker)?;
            let spot = match spot_scalar {
                finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
                finstack_quant_core::market_data::scalars::MarketScalar::Price(p) => p.amount(),
            };
            // Date-based zero rate over [as_of, maturity]: avoids the
            // axis bias of `disc.zero(t)` when curve base != as_of.
            let df_mat =
                crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
                    disc.as_ref(),
                    as_of,
                    inst.maturity,
                )?;
            let r = crate::instruments::common_impl::helpers::zero_rate_from_df(
                df_mat,
                t,
                "variance-swap replication rate",
            )?;
            let dividend_yield_id = format!("{}-DIVYIELD", inst.underlying_ticker);
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
                let vol_fn = |t_exp: f64, k: f64| surface.value_clamped(t_exp, k);
                let bs_fn =
                    |k: f64, v: f64, opt: OptionType| -> f64 { bs_price(spot, k, r, q, v, t, opt) };
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
                     falling back to ATM variance plus local smile convexity"
                );
                return Ok(fallback_variance);
            }
            return Err(finstack_quant_core::Error::Validation(format!(
                "variance-swap surface '{sid}' could not produce a finite forward variance"
            )));
        }
    }

    if let Ok(scalar) = context.get_price(format!("{}_IMPL_VOL", inst.underlying_ticker)) {
        let vol = match scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(_) => {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "variance-swap implied volatility '{}_IMPL_VOL' must be unitless",
                    inst.underlying_ticker
                )));
            }
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
        Err(finstack_quant_core::InputError::NotFound {
            id: format!(
                "variance-swap volatility for '{}': supply a vol surface or '{}_IMPL_VOL'",
                inst.underlying_ticker, inst.underlying_ticker
            ),
        }
        .into())
    }
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
        MarketContext::new()
            .insert(curve)
            .insert_price("SPX_IMPL_VOL", MarketScalar::Unitless(0.20))
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
    /// day-count `time_elapsed_fraction`, not by observation count. The two
    /// fractions diverge for non-uniform schedules and the MTM error is
    /// first-order near maturity.
    #[test]
    fn seasoned_mtm_uses_time_weighting_not_observation_count() {
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
            .notional(Money::new(
                1_000_000.0,
                finstack_quant_core::currency::Currency::USD,
            ))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .observation_calendar_id("USNY".to_string())
            .realized_var_method(finstack_quant_core::math::stats::RealizedVarMethod::CloseToClose)
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

        // Recompute the identity from the same building blocks. The realized
        // term must use the time-basis annualization (`seasoned_realized_variance`,
        // W-33) so it is consistent with the day-count blend weight `w`.
        let total_t = swap
            .day_count
            .year_fraction(swap.start_date, swap.maturity, Default::default())
            .expect("total yf");
        let t_elapsed = time_w * total_t;
        let realized =
            seasoned_realized_variance(&swap, &market, as_of, t_elapsed).expect("realized");
        let forward = remaining_forward_variance(&swap, &market, as_of).expect("forward");
        let expected_var = realized * time_w + forward * (1.0 - time_w);
        let disc = market.get_discount("USD-OIS").expect("curve");
        let df = crate::instruments::common_impl::pricing::time::relative_df_discount_curve(
            disc.as_ref(),
            as_of,
            swap.maturity,
        )
        .expect("df");
        let expected_pv = swap.payoff(expected_var) * df;

        assert!(
            (pv.amount() - expected_pv.amount()).abs() < 1e-6,
            "seasoned MTM must use time-weighted identity: pv={} expected={}",
            pv.amount(),
            expected_pv.amount()
        );

        // And it must NOT match the (wrong) observation-count weighting.
        let count_var = realized * count_w + forward * (1.0 - count_w);
        let count_pv = swap.payoff(count_var) * df;
        assert!(
            (pv.amount() - count_pv.amount()).abs() > 1e-6,
            "seasoned MTM must differ from observation-count weighting"
        );
    }

    /// W-33: the realized-variance term in the seasoned MTM blend must be
    /// annualized on the *day-count time basis* (`V_accrued / t_elapsed`), the
    /// same basis as the blend weight `w`. The observation-count annualization
    /// (`partial_realized_variance`, Σr²/N · ~252) uses a different time base,
    /// so the accrued-variance identity does not close. The two must differ for
    /// a non-uniform (weekend-skipping) schedule, and `compute_pv` must use the
    /// time-basis value.
    #[test]
    fn seasoned_realized_variance_uses_time_basis_not_observation_count() {
        use crate::instruments::common_impl::traits::Attributes;
        use crate::instruments::equity::variance_swap::types::PayReceive;
        use finstack_quant_core::dates::{DayCount, Tenor};
        use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
        use finstack_quant_core::money::Money;
        use finstack_quant_core::types::{CurveId, InstrumentId};

        let start = date!(2025 - 01 - 06); // Monday
        let maturity = date!(2025 - 06 - 30); // Monday
        let as_of = date!(2025 - 04 - 18); // Friday, mid-life

        let swap = VarianceSwap::builder()
            .id(InstrumentId::new("VARSPX-W33"))
            .underlying_ticker("SPX".to_string())
            .notional(Money::new(
                1_000_000.0,
                finstack_quant_core::currency::Currency::USD,
            ))
            .strike_variance(0.04)
            .start_date(start)
            .maturity(maturity)
            .observation_freq(Tenor::daily())
            .observation_calendar_id("USNY".to_string())
            .realized_var_method(finstack_quant_core::math::stats::RealizedVarMethod::CloseToClose)
            .side(PayReceive::Receive)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .day_count(DayCount::Act365F)
            .attributes(Attributes::new())
            .build()
            .expect("w33 swap");

        let past: Vec<Date> = observation_dates(&swap)
            .expect("observation schedule")
            .into_iter()
            .filter(|&d| d <= as_of)
            .collect();
        let obs: Vec<(Date, f64)> = past
            .iter()
            .enumerate()
            .map(|(i, &d)| (d, 100.0 * (1.0 + 0.002 * (i as f64 % 4.0 - 1.5))))
            .collect();
        let series = ScalarTimeSeries::new("SPX", obs, None).expect("series");
        let market = build_market(as_of).insert_series(series);

        let time_w = swap.time_elapsed_fraction(as_of);
        let total_t = swap
            .day_count
            .year_fraction(swap.start_date, swap.maturity, Default::default())
            .expect("total yf");
        let t_elapsed = time_w * total_t;

        let obs_count_realized =
            partial_realized_variance(&swap, &market, as_of).expect("obs-count realized");
        let time_basis_realized =
            seasoned_realized_variance(&swap, &market, as_of, t_elapsed).expect("time realized");

        // The two annualizations must genuinely differ (weekend-skipping
        // schedule => N_returns/AF ≠ t_elapsed).
        assert!(
            (obs_count_realized - time_basis_realized).abs() / time_basis_realized.max(1e-12)
                > 1e-3,
            "observation-count ({obs_count_realized}) and time-basis \
             ({time_basis_realized}) realized variance must differ"
        );

        // Identity check: time_basis_realized = V_accrued / t_elapsed.
        // Reconstruct V_accrued = Σr² directly from the close series.
        let prices: Vec<f64> = past
            .iter()
            .enumerate()
            .map(|(i, _)| 100.0 * (1.0 + 0.002 * (i as f64 % 4.0 - 1.5)))
            .collect();
        let v_accrued: f64 = prices
            .windows(2)
            .map(|w| {
                let r = (w[1] / w[0]).ln();
                r * r
            })
            .sum();
        let expected_time_realized = v_accrued / t_elapsed;
        assert!(
            (time_basis_realized - expected_time_realized).abs()
                / expected_time_realized.max(1e-12)
                < 1e-9,
            "seasoned realized variance must equal V_accrued / t_elapsed: \
             got {time_basis_realized}, expected {expected_time_realized}"
        );
    }

    /// Week tenors step in calendar weeks; day tenors step in business-day
    /// observations. Their annualization bases must preserve that distinction.
    #[test]
    fn weekly_and_biweekly_annualization_uses_calendar_observation_counts() {
        use finstack_quant_core::dates::{Tenor, TenorUnit};

        let mut swap = VarianceSwap::example().expect("example swap");

        swap.observation_freq = Tenor::new(1, TenorUnit::Weeks);
        assert_eq!(annualization_factor(&swap), 52.0);

        swap.observation_freq = Tenor::new(2, TenorUnit::Weeks);
        assert_eq!(annualization_factor(&swap), 26.0);

        swap.observation_freq = Tenor::new(1, TenorUnit::Days);
        assert_eq!(annualization_factor(&swap), 252.0);

        // The policy-aware variant must agree (no TRADING_DAYS_PER_YEAR
        // override in this market context).
        let market = MarketContext::new();
        swap.observation_freq = Tenor::new(7, TenorUnit::Days);
        assert_eq!(annualization_factor_with_policy(&swap, &market), 36.0);
        swap.observation_freq = Tenor::new(14, TenorUnit::Days);
        assert_eq!(annualization_factor_with_policy(&swap, &market), 18.0);

        swap.start_date = date!(2025 - 01 - 03); // Friday
        swap.maturity = date!(2025 - 01 - 15);
        swap.observation_freq = Tenor::new(2, TenorUnit::Days);
        let dates = observation_dates(&swap).expect("observation schedule");
        assert_eq!(dates[0], date!(2025 - 01 - 03));
        assert_eq!(dates[1], date!(2025 - 01 - 07));
        assert!(dates
            .iter()
            .all(|d| !matches!(d.weekday(), time::Weekday::Saturday | time::Weekday::Sunday)));
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

    /// W-39: the smile-convexity fallback must capture *deep-wing* convexity.
    ///
    /// On a surface that is flat at the strikes bracketing the forward but has
    /// elevated *deep* wings, the old 2-strike proxy only inspected the two
    /// near-forward strikes and saw a flat smile, so it reported ≈ ATM variance
    /// — under-weighting the wings. The 1/K²-weighted full-grid estimate must
    /// pick up the deep wings and report variance materially above ATM.
    #[test]
    fn smile_convexity_fallback_captures_deep_wing_convexity() {
        // Strikes bracketing the forward (90, 100, 110) are FLAT at 20% vol;
        // only the DEEP wings (60, 150) are elevated. A 2-strike proxy around
        // the 100 forward would see only the flat 20% and miss the wings.
        let surface = VolSurface::builder("SPX")
            .expiries(&[1.0])
            .strikes(&[60.0, 90.0, 100.0, 110.0, 150.0])
            .row(&[0.45, 0.20, 0.20, 0.20, 0.35])
            .build()
            .expect("surface");

        let atm_variance = 0.20_f64 * 0.20; // 0.04
        let fallback =
            smile_convexity_adjusted_variance(&surface, 1.0, 100.0).expect("fallback variance");

        // The deep wings must lift the estimate meaningfully above ATM. The old
        // 2-strike proxy (k_lo=90, k_hi=110, both 20%) returned exactly 0.04.
        assert!(
            fallback > atm_variance * 1.05,
            "deep-wing convexity must lift the fallback well above ATM variance \
             ({atm_variance}); got {fallback}"
        );
    }
}
