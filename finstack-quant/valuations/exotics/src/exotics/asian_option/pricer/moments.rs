//! Shared analytical moment-matching and schedule validation for equity/FX and
//! commodity Asian options.

use crate::exotics::asian_option::types::average_intrinsic;
use finstack_quant_core::dates::Date;
use finstack_quant_core::math::summation::{NeumaierAccumulator, neumaier_sum};
use finstack_quant_models::monte_carlo::payoff::asian::AveragingMethod;
use finstack_quant_models::types::OptionType;

/// Inputs for validating an Asian option schedule and fixing history.
pub(crate) struct AsianScheduleSpec<'a> {
    pub(crate) label: &'a str,
    pub(crate) instrument_id: &'a str,
    pub(crate) strike: f64,
    pub(crate) quantity: f64,
    pub(crate) expiry: Date,
    pub(crate) fixing_dates: &'a [Date],
    pub(crate) past_fixings: &'a [(Date, f64)],
    pub(crate) averaging_method: AveragingMethod,
    pub(crate) require_positive_all: bool,
}

/// Validate common Asian option schedule and fixing invariants shared by equity/FX
/// and commodity Asian contracts.
pub(crate) fn validate_asian_fixing_schedule(
    spec: &AsianScheduleSpec<'_>,
) -> finstack_quant_core::Result<()> {
    use finstack_quant_core::validation;

    validation::validate_f64_positive(spec.strike, &format!("{} strike", spec.label))?;
    validation::validate_f64_positive(spec.quantity, &format!("{} quantity", spec.label))?;
    if spec.fixing_dates.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} requires at least one fixing date",
            spec.label
        )));
    }
    if spec
        .fixing_dates
        .windows(2)
        .any(|dates| dates[0] >= dates[1])
    {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} fixing_dates must be strictly increasing",
            spec.label
        )));
    }
    if spec.fixing_dates.iter().any(|date| *date > spec.expiry) {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{} fixing_dates cannot extend beyond expiry",
            spec.label
        )));
    }
    let mut observed = std::collections::BTreeSet::new();
    for (date, value) in spec.past_fixings {
        if !spec.fixing_dates.contains(date) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} '{}' has a past fixing for {date}, which is not a scheduled fixing date",
                spec.label, spec.instrument_id
            )));
        }
        if !observed.insert(*date) {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} '{}' has duplicate past fixing for {date}",
                spec.label, spec.instrument_id
            )));
        }
        validation::validate_f64_finite(*value, &format!("{} realized-fixing value", spec.label))?;
        if (spec.require_positive_all
            || matches!(spec.averaging_method, AveragingMethod::Geometric))
            && *value <= 0.0
        {
            return Err(finstack_quant_core::Error::Validation(format!(
                "{} '{}' fixing on {date} must be positive and finite, got {value}",
                spec.label, spec.instrument_id
            )));
        }
    }
    Ok(())
}

/// First and log-variance moments of a seasoned geometric average over future forwards.
pub(crate) struct GeometricMoments {
    /// Sum of log future forwards.
    pub(crate) log_sum: f64,
    /// Risk-neutral expectation `E[G_all]` incorporating realized log fixings and convexity.
    pub(crate) forward: f64,
    /// Variance of `ln G_all` across all `N` total fixings.
    pub(crate) log_variance: f64,
}

/// Compute `O(m)` geometric Asian moments for a non-decreasing projection `(time, forward)`.
pub(crate) fn geometric_moments(
    future_forwards: &[(f64, f64)],
    sigma: f64,
    hist_prod_log: f64,
    total_fixings: usize,
) -> finstack_quant_core::Result<GeometricMoments> {
    let count = future_forwards.len();
    let m = count as f64;
    let n = total_fixings as f64;
    let log_sum = neumaier_sum(future_forwards.iter().map(|(_, forward)| forward.ln()))?;
    let geo_mean_fwd = (log_sum / m).exp();
    // For non-decreasing times, t_i contributes its diagonal plus pairs against later fixings.
    let weighted_times = neumaier_sum(
        future_forwards
            .iter()
            .enumerate()
            .map(|(index, (time, _))| (2.0 * (count - index) as f64 - 1.0) * time),
    )?;
    let vol_adj_sq = sigma * sigma * weighted_times / (m * m);
    let sum_t = neumaier_sum(future_forwards.iter().map(|(time, _)| *time))?;
    let mean_log_drift = -0.5 * sigma * sigma * sum_t / m;
    let realized_factor = (hist_prod_log / n).exp();
    let ratio = m / n;
    let log_variance = ratio * ratio * vol_adj_sq;
    let forward = realized_factor
        * geo_mean_fwd.powf(ratio)
        * (ratio * mean_log_drift + 0.5 * log_variance).exp();
    Ok(GeometricMoments {
        log_sum,
        forward,
        log_variance,
    })
}

/// Closed-form seasoned geometric Asian price via [`finstack_quant_models::closed_form::black76_price`].
pub(crate) fn price_geometric_asian(
    future_forwards: &[(f64, f64)],
    strike: f64,
    sigma: f64,
    df: f64,
    option_type: OptionType,
    hist_prod_log: f64,
    total_fixings: usize,
) -> finstack_quant_core::Result<f64> {
    let n = total_fixings as f64;
    let m = future_forwards.len() as f64;

    if m == 0.0 || n <= 0.0 {
        return Ok(0.0);
    }

    let GeometricMoments {
        log_sum,
        forward: fwd_x,
        log_variance: var_x,
    } = geometric_moments(future_forwards, sigma, hist_prod_log, total_fixings)?;

    let t_last = future_forwards
        .iter()
        .map(|(t, _)| *t)
        .fold(0.0_f64, f64::max);

    if !fwd_x.is_finite() || fwd_x <= 0.0 || var_x <= 0.0 || t_last <= 0.0 {
        let geo_avg_all = ((hist_prod_log + log_sum) / n).exp();
        return Ok(average_intrinsic(option_type, geo_avg_all, strike) * df);
    }

    let price = finstack_quant_models::closed_form::black76_price(
        fwd_x,
        strike,
        df,
        var_x.sqrt(),
        1.0,
        option_type,
    )?;
    Ok(price.max(0.0))
}

/// Compute `O(m)` arithmetic first moment and variance for a non-decreasing projection `(time, forward)`.
pub(crate) fn arithmetic_moments(
    future_forwards: &[(f64, f64)],
    sigma: f64,
) -> finstack_quant_core::Result<(f64, f64)> {
    let mut suffix = NeumaierAccumulator::new();
    let mut covariance = NeumaierAccumulator::new();
    let sigma_sq = sigma * sigma;
    for &(time, forward) in future_forwards.iter().rev() {
        covariance
            .add((sigma_sq * time).exp_m1() * forward * (forward + 2.0 * suffix.get_total()?))?;
        suffix.add(forward)?;
    }
    let count = future_forwards.len() as f64;
    Ok((
        suffix.get_total()? / count,
        covariance.get_total()? / (count * count),
    ))
}

/// Turnbull-Wakeman seasoned arithmetic Asian price via [`finstack_quant_models::closed_form::black76_price`].
pub(crate) fn price_arithmetic_tw_asian(
    future_forwards: &[(f64, f64)],
    strike: f64,
    sigma: f64,
    df: f64,
    option_type: OptionType,
    hist_sum: f64,
    total_fixings: usize,
) -> finstack_quant_core::Result<f64> {
    let n = total_fixings as f64;
    let m = future_forwards.len() as f64;

    if m == 0.0 || n <= 0.0 {
        return Ok(0.0);
    }

    let k_eff = (n * strike - hist_sum) / m;
    let scale = m / n;

    if k_eff <= 0.0 {
        let sum_fwd = neumaier_sum(future_forwards.iter().map(|(_, f)| *f))?;
        let avg_fwd = (hist_sum + sum_fwd) / n;
        return Ok(average_intrinsic(option_type, avg_fwd, strike) * df);
    }

    let (m1, variance) = arithmetic_moments(future_forwards, sigma)?;
    let var = (variance / m1 / m1).ln_1p();
    if var <= 0.0 {
        return Ok(df * scale * average_intrinsic(option_type, m1, k_eff));
    }

    let price = finstack_quant_models::closed_form::black76_price(
        m1,
        k_eff,
        df,
        var.sqrt(),
        1.0,
        option_type,
    )?;
    Ok((price * scale).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exotics::asian_option::pricer::AsianOptionMcPricer;
    use crate::exotics::asian_option::types::AsianOption;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{Date, DayCount};
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::{
        VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
    };
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{Attributes, CurveId, InstrumentId};
    use finstack_quant_valuations_core::instruments::Instrument;
    use time::Month;

    #[test]
    fn turnbull_wakeman_is_continuous_across_zero_effective_strike_and_matches_mc_at_zero_vol() {
        let forwards = [(0.5, 100.0), (1.0, 102.0)];
        let hist_sum = 100.0;
        let strike = 50.0; // 3 * 50 - 100 = 50 > 0; strike = 33.33 -> k_eff ~ 0
        let eps = 1e-6;
        let p_pos = price_arithmetic_tw_asian(
            &forwards,
            100.0 / 3.0 + eps,
            0.25,
            0.95,
            OptionType::Call,
            hist_sum,
            3,
        )
        .expect("k_eff > 0");
        let p_zero = price_arithmetic_tw_asian(
            &forwards,
            100.0 / 3.0 - eps,
            0.25,
            0.95,
            OptionType::Call,
            hist_sum,
            3,
        )
        .expect("k_eff <= 0");
        assert!(
            (p_pos - p_zero).abs() < 1e-4,
            "discontinuity across k_eff=0: {p_pos} vs {p_zero}"
        );
        assert!(p_pos > 0.0 && strike > 0.0);

        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("date");
        let f1 = Date::from_calendar_date(2025, Month::July, 1).expect("date");
        let expiry = Date::from_calendar_date(2026, Month::January, 1).expect("date");
        let mut asian = AsianOption::builder()
            .id(InstrumentId::new("ASIAN-ZERO-VOL"))
            .underlying_ticker("SPX".to_string())
            .strike(95.0)
            .option_type(OptionType::Call)
            .averaging_method(AveragingMethod::Arithmetic)
            .expiry(expiry)
            .fixing_dates(vec![f1, expiry])
            .quantity(10.0)
            .currency(Currency::USD)
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(None)
            .attributes(Attributes::new())
            .build()
            .expect("build");
        asian
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.0);
        asian.instrument_pricing_overrides.model_config.mc_paths = Some(32);

        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (2.0, (-0.04 * 2.0_f64).exp())])
            .interp(InterpStyle::MonotoneConvex)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .expect("curve");
        let surface = VolSurface::from_grid(
            "SPX-VOL",
            &[0.1, 2.0],
            &[50.0, 150.0],
            &[0.2, 0.2, 0.2, 0.2],
            VolGridOpts {
                secondary_axis: VolSurfaceAxis::Strike,
                quote_type: VolQuoteType::BlackLognormal,
                interpolation_mode: VolInterpolationMode::Vol,
            },
            None,
        )
        .expect("surface");
        let market = MarketContext::new()
            .try_insert(curve)
            .expect("insert")
            .insert_surface(surface)
            .insert_price(
                "SPX-SPOT",
                MarketScalar::price(Money::new(100.0, Currency::USD).expect("money")),
            );
        let tw_pv = asian.value(&market, as_of).expect("tw pv").amount();
        let mc_pv = AsianOptionMcPricer::new()
            .price_internal(&asian, &market, as_of)
            .expect("mc pv")
            .amount();
        assert!((tw_pv - mc_pv).abs() < 1e-8, "tw={tw_pv} vs mc={mc_pv}");
    }
}
