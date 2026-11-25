//! Black-Scholes DF-first inputs and curve-consistent GBM drift schedule for exotic pricers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::types::{CurveId, PriceId};
use finstack_quant_core::{Error, Result};
use finstack_quant_models::monte_carlo::process::gbm::DriftSchedule;
use finstack_quant_valuations_core::instruments::pricing::discounting::zero_rate_from_df;
use finstack_quant_valuations_core::instruments::pricing_overrides::MarketQuoteOverrides;

/// Build a curve-consistent cumulative drift schedule for `GbmProcess::with_drift_schedule`.
///
/// Uses `disc_curve` directly at each step `t_k = k * t / num_steps` rather than
/// taking a flat rate `r`, rescaling so the terminal cumulative rate equals `r * t`
/// for consistency with date-based discounting.
///
/// # Arguments
///
/// * `disc_curve` - Discount curve providing term-structure shape between `as_of` and `as_of + t`.
/// * `as_of` - Valuation date anchoring step `t_k = 0`.
/// * `r` - Effective continuously-compounded terminal risk-free rate consistent with `df(t)`.
/// * `q` - Continuous dividend yield / foreign carry rate.
/// * `t` - Option expiry time in years from `as_of`.
/// * `num_steps` - Number of Monte Carlo time steps (at least 1).
///
/// # Errors
///
/// Returns an error if curve evaluation fails, `df(as_of)` is non-positive/non-finite,
/// or the resulting [`DriftSchedule`] fails validation.
pub(crate) fn build_gbm_drift_schedule(
    disc_curve: &DiscountCurve,
    as_of: Date,
    r: f64,
    q: f64,
    t: f64,
    num_steps: usize,
) -> Result<DriftSchedule> {
    let knots = num_steps.max(1);
    let as_of_curve_time = disc_curve.day_count().signed_year_fraction(
        disc_curve.base_date(),
        as_of,
        DayCountContext::default(),
    )?;
    let as_of_df = disc_curve.df(as_of_curve_time)?;
    if !as_of_df.is_finite() || as_of_df <= 0.0 {
        return Err(Error::Validation(format!(
            "GBM drift schedule: invalid discount factor at as_of {as_of}: {as_of_df}"
        )));
    }
    let terminal_rate_cum = -(disc_curve.df(as_of_curve_time + t)? / as_of_df).ln();
    let scale = if terminal_rate_cum.abs() > 1e-12 {
        (r * t) / terminal_rate_cum
    } else {
        1.0
    };

    let mut times = Vec::with_capacity(knots + 1);
    let mut cumulative = Vec::with_capacity(knots + 1);
    for k in 0..=knots {
        let tk = t * (k as f64) / (knots as f64);
        let rate_cum = if tk > 0.0 {
            -(disc_curve.df(as_of_curve_time + tk)? / as_of_df).ln() * scale
        } else {
            0.0
        };
        times.push(tk);
        cumulative.push(rate_cum - q * tk);
    }
    DriftSchedule::new(times, cumulative)
}

/// Black-Scholes inputs with discount factor (DF-first approach).
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlackScholesInputsDf {
    /// Current spot price
    pub spot: f64,
    /// Discount factor from `as_of` to expiry (date-based)
    pub df: f64,
    /// Dividend yield / foreign rate (continuous)
    pub q: f64,
    /// Implied volatility
    pub sigma: f64,
    /// Time to expiry in years (vol surface basis)
    pub t: f64,
}

impl BlackScholesInputsDf {
    /// Derive an effective continuously-compounded rate consistent with `t` and `df`.
    ///
    /// Returns `0.0` at expiry (`t == 0.0`) after validating that `df` is finite
    /// and strictly positive, and delegates to [`zero_rate_from_df`] when `t > 0.0`.
    ///
    /// # Errors
    /// Returns [`Error::Validation`] if `t` is non-finite or negative, or if `df`
    /// is non-finite or non-positive.
    #[inline]
    pub(crate) fn r_eff(&self) -> Result<f64> {
        if !self.t.is_finite() || self.t < 0.0 {
            return Err(Error::Validation(format!(
                "BlackScholesInputsDf: time to expiry must be finite and non-negative, got {}",
                self.t
            )));
        }
        if !self.df.is_finite() || self.df <= 0.0 {
            return Err(Error::Validation(format!(
                "BlackScholesInputsDf: discount factor must be finite and > 0, got {:.6e} for t={:.6}",
                self.df, self.t
            )));
        }
        if self.t == 0.0 {
            return Ok(0.0);
        }
        zero_rate_from_df(self.df, self.t, "BlackScholesInputsDf")
    }
}

/// Resolve a strictly positive spot level from `spot_id`, enforcing currency consistency.
pub(crate) fn resolve_positive_spot(
    market: &MarketContext,
    spot_id: &PriceId,
    currency: Currency,
    context_label: &str,
) -> Result<f64> {
    let spot =
        finstack_quant_valuations_core::instruments::pricing::market_inputs::scalar_price_amount(
            market.get_price(spot_id)?,
            currency,
        )?;
    finstack_quant_core::validation::validate_f64_positive(
        spot,
        &format!("{context_label} asset spot"),
    )?;
    Ok(spot)
}

/// Specification for collecting standard DF-first Black-Scholes market inputs.
pub(crate) struct BsMarketInputSpec<'a> {
    pub context_label: &'a str,
    pub day_count: DayCount,
    pub expiry: Date,
    pub discount_curve_id: &'a CurveId,
    pub spot_id: &'a PriceId,
    pub vol_surface_id: &'a CurveId,
    pub div_yield_id: Option<&'a PriceId>,
    pub currency: Currency,
    pub market_quotes: &'a MarketQuoteOverrides,
    /// Explicit volatility lookup strike; `None` falls back to the resolved positive spot.
    pub vol_strike: Option<f64>,
}

/// Collect DF-first Black-Scholes inputs in a fixed order: time/discount curve,
/// positive spot, optional dividend yield, and volatility at strike.
pub(crate) fn collect_bs_inputs(
    market: &MarketContext,
    as_of: Date,
    spec: BsMarketInputSpec<'_>,
) -> Result<BlackScholesInputsDf> {
    let t = spec
        .day_count
        .year_fraction(as_of, spec.expiry, DayCountContext::default())?;
    let df = market
        .get_discount(spec.discount_curve_id.as_str())?
        .df_between_dates(as_of, spec.expiry)?;
    if !df.is_finite() || df <= 0.0 {
        return Err(Error::Validation(format!(
            "{}: invalid discount factor ({df:.6e}) between {as_of} and {}",
            spec.context_label, spec.expiry
        )));
    }
    let spot = resolve_positive_spot(market, spec.spot_id, spec.currency, spec.context_label)?;
    let q = finstack_quant_valuations_core::instruments::pricing::market_inputs::resolve_optional_dividend_yield(
        market,
        spec.div_yield_id,
    )?;
    let vol_strike = spec.vol_strike.unwrap_or(spot);
    let sigma = finstack_quant_valuations_core::instruments::pricing::volatility::resolve_sigma_at(
        spec.market_quotes,
        market,
        spec.vol_surface_id.as_str(),
        t,
        vol_strike,
    )?;
    Ok(BlackScholesInputsDf {
        spot,
        df,
        q,
        sigma,
        t,
    })
}

#[cfg(test)]
mod tests {
    use super::BlackScholesInputsDf;

    #[test]
    fn black_scholes_inputs_df_r_eff_consistency() {
        let inputs = BlackScholesInputsDf {
            spot: 100.0,
            df: 0.95,
            q: 0.02,
            sigma: 0.20,
            t: 1.0,
        };
        let r_eff = inputs.r_eff().expect("valid inputs");
        let expected_r = -0.95_f64.ln() / 1.0;
        assert!((r_eff - expected_r).abs() < 1e-10);
        let reconstructed_df = (-r_eff * inputs.t).exp();
        assert!((reconstructed_df - inputs.df).abs() < 1e-10);
    }

    #[test]
    fn black_scholes_inputs_df_edge_cases() {
        let at_expiry = BlackScholesInputsDf {
            spot: 100.0,
            df: 1.0,
            q: 0.02,
            sigma: 0.20,
            t: 0.0,
        };
        assert_eq!(at_expiry.r_eff().expect("valid at expiry"), 0.0);

        let short_horizon = BlackScholesInputsDf {
            spot: 100.0,
            df: 0.9999,
            q: 0.0,
            sigma: 0.20,
            t: 0.001,
        };
        let r_short = short_horizon.r_eff().expect("valid short horizon");
        assert!(r_short > 0.0 && r_short.is_finite());

        for invalid in [
            BlackScholesInputsDf {
                spot: 100.0,
                df: -0.95,
                q: 0.0,
                sigma: 0.2,
                t: 1.0,
            },
            BlackScholesInputsDf {
                spot: 100.0,
                df: f64::NAN,
                q: 0.0,
                sigma: 0.2,
                t: 0.0,
            },
            BlackScholesInputsDf {
                spot: 100.0,
                df: 0.95,
                q: 0.0,
                sigma: 0.2,
                t: -0.1,
            },
        ] {
            assert!(invalid.r_eff().is_err());
        }
    }
}
