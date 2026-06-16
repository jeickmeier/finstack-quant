//! Lookback option pricers (Monte Carlo and analytical).

// Common imports for all pricers
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::lookback_option::types::{LookbackOption, LookbackType};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

// MC-specific imports
use finstack_quant_monte_carlo::payoff::lookback::{
    FloatingStrikeLookbackCall, FloatingStrikeLookbackPut, Lookback, LookbackDirection,
};
use finstack_quant_monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_monte_carlo::process::gbm::{GbmParams, GbmProcess};

/// Lookback option Monte Carlo pricer.
pub struct LookbackOptionMcPricer {
    config: PathDependentPricerConfig,
}

impl LookbackOptionMcPricer {
    /// Create a new lookback option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    fn merged_path_config(&self, inst: &LookbackOption) -> PathDependentPricerConfig {
        let mut c = self.config.clone();
        if let Some(n) = inst.pricing_overrides.model_config.mc_paths {
            if n > 0 {
                c.num_paths = n;
            }
        }
        c
    }

    /// Price a lookback option using Monte Carlo.
    fn price_internal(
        &self,
        inst: &LookbackOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        if as_of >= inst.expiry {
            let payoff = expired_lookback_payoff(inst, lookback_spot(curves, &inst.spot_id)?)?;
            return Ok(finstack_quant_core::money::Money::new(
                payoff * inst.notional.amount(),
                inst.notional.currency(),
            ));
        }

        let t = inst
            .day_count
            .year_fraction(as_of, inst.expiry, DayCountContext::default())?;
        if t <= 0.0 {
            let payoff = expired_lookback_payoff(inst, lookback_spot(curves, &inst.spot_id)?)?;
            return Ok(finstack_quant_core::money::Money::new(
                payoff * inst.notional.amount(),
                inst.notional.currency(),
            ));
        }

        let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
        let discount_factor = disc_curve.df_between_dates(as_of, inst.expiry)?;
        // Keep drift consistent with date-based discounting for MC simulation.
        let r = if t > 0.0 && discount_factor > 0.0 {
            -discount_factor.ln() / t
        } else {
            0.0
        };

        let spot_scalar = curves.get_price(&inst.spot_id)?;
        let spot = match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        };

        let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
            curves,
            inst.div_yield_id.as_ref(),
        )?;

        let strike_val = inst.strike.unwrap_or(spot);
        let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
            &inst.pricing_overrides.market_quotes,
            curves,
            inst.vol_surface_id.as_str(),
            t,
            strike_val,
        )?;

        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let base_cfg = self.merged_path_config(inst);

        let steps_per_year = base_cfg.steps_per_year;
        let num_steps = ((t * steps_per_year).round() as usize).max(base_cfg.min_steps);

        // Time-varying drift: project each MC step with the curve-implied
        // forward drift, so the running extremum is sampled from unbiased
        // per-step spots on a non-flat rate curve. On a flat curve this is
        // bit-equivalent to the constant `(r - q)` drift.
        let process = process.with_drift_schedule(std::sync::Arc::new(
            crate::instruments::common_impl::helpers::build_gbm_drift_schedule(
                disc_curve.as_ref(),
                r,
                q,
                t,
                num_steps,
            )?,
        ));

        // `maturity_step` must equal `num_steps`: the engine fires `on_event` with
        // `state.step = num_steps` on the last iteration (after the final diffusion step),
        // so both the terminal-spot capture and the running-extremum update must include
        // that final step.  Using `num_steps - 1` would read the spot one step before
        // maturity (at time T·(N-1)/N) and exclude the last spot from the tracked extremum.
        let maturity_step = num_steps;

        let currency = inst.notional.currency();

        // Derive deterministic seed from instrument ID and scenario

        use finstack_quant_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let mut config = base_cfg;
        config.seed = seed;
        let pricer = PathDependentPricer::new(config);
        let result = match (inst.lookback_type, inst.option_type) {
            (LookbackType::FloatingStrike, crate::instruments::OptionType::Call) => {
                // Floating Strike Call: Payoff = S_T - S_min
                // Seed initial minimum from observed_min if seasoned
                let initial_min = inst
                    .observed_min
                    .as_ref()
                    .map(|m| m.amount())
                    .unwrap_or(f64::INFINITY);
                let payoff = FloatingStrikeLookbackCall::with_initial_min(
                    inst.notional.amount(),
                    maturity_step,
                    initial_min,
                );
                pricer.price(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &payoff,
                    currency,
                    discount_factor,
                )?
            }
            (LookbackType::FloatingStrike, crate::instruments::OptionType::Put) => {
                // Floating Strike Put: Payoff = S_max - S_T
                // Seed initial maximum from observed_max if seasoned
                let initial_max = inst
                    .observed_max
                    .as_ref()
                    .map(|m| m.amount())
                    .unwrap_or(f64::NEG_INFINITY);
                let payoff = FloatingStrikeLookbackPut::with_initial_max(
                    inst.notional.amount(),
                    maturity_step,
                    initial_max,
                );
                pricer.price(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &payoff,
                    currency,
                    discount_factor,
                )?
            }
            (LookbackType::FixedStrike, crate::instruments::OptionType::Call) => {
                // Fixed Strike Call: Payoff = max(S_max - K, 0)
                // Seed initial maximum from observed_max if seasoned
                let initial_max = inst
                    .observed_max
                    .as_ref()
                    .map(|m| m.amount())
                    .unwrap_or(f64::NEG_INFINITY);
                let strike = inst.strike.as_ref().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "FixedStrike lookback requires a strike".into(),
                    )
                })?;
                let payoff = Lookback::with_initial_extremum(
                    LookbackDirection::Call,
                    *strike,
                    inst.notional.amount(),
                    maturity_step,
                    initial_max,
                );
                pricer.price(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &payoff,
                    currency,
                    discount_factor,
                )?
            }
            (LookbackType::FixedStrike, crate::instruments::OptionType::Put) => {
                // Fixed Strike Put: Payoff = max(K - S_min, 0)
                // Seed initial minimum from observed_min if seasoned
                let initial_min = inst
                    .observed_min
                    .as_ref()
                    .map(|m| m.amount())
                    .unwrap_or(f64::INFINITY);
                let strike = inst.strike.as_ref().ok_or_else(|| {
                    finstack_quant_core::Error::Validation(
                        "FixedStrike lookback requires a strike".into(),
                    )
                })?;
                let payoff = Lookback::with_initial_extremum(
                    LookbackDirection::Put,
                    *strike,
                    inst.notional.amount(),
                    maturity_step,
                    initial_min,
                );
                pricer.price(
                    &process,
                    spot,
                    t,
                    num_steps,
                    &payoff,
                    currency,
                    discount_factor,
                )?
            }
        };

        Ok(result.mean)
    }
}

impl Default for LookbackOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for LookbackOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::LookbackOption, ModelKey::MonteCarloGBM)
    }

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let lookback = instrument
            .as_any()
            .downcast_ref::<LookbackOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::LookbackOption, instrument.key())
            })?;

        let pv = self.price_internal(lookback, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(lookback.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &LookbackOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    let pricer = LookbackOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

// ========================= ANALYTICAL PRICER =========================

use crate::models::closed_form::lookback::{
    fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_call,
    floating_strike_lookback_put,
};

fn lookback_spot(
    curves: &MarketContext,
    spot_id: &finstack_quant_core::types::PriceId,
) -> finstack_quant_core::Result<f64> {
    let spot_scalar = curves.get_price(spot_id)?;
    Ok(match spot_scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    })
}

fn expired_lookback_payoff(inst: &LookbackOption, spot: f64) -> finstack_quant_core::Result<f64> {
    let payoff = match inst.lookback_type {
        LookbackType::FixedStrike => {
            let strike = inst.strike.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "FixedStrike lookback requires a strike".to_string(),
                )
            })?;
            match inst.option_type {
                crate::instruments::OptionType::Call => {
                    let observed_max = inst
                        .observed_max
                        .as_ref()
                        .map(|m| m.amount())
                        .unwrap_or(spot);
                    (observed_max.max(spot) - strike).max(0.0)
                }
                crate::instruments::OptionType::Put => {
                    let observed_min = inst
                        .observed_min
                        .as_ref()
                        .map(|m| m.amount())
                        .unwrap_or(spot);
                    (strike - observed_min.min(spot)).max(0.0)
                }
            }
        }
        LookbackType::FloatingStrike => match inst.option_type {
            crate::instruments::OptionType::Call => {
                let observed_min = inst
                    .observed_min
                    .as_ref()
                    .map(|m| m.amount())
                    .unwrap_or(spot);
                spot - observed_min.min(spot)
            }
            crate::instruments::OptionType::Put => {
                let observed_max = inst
                    .observed_max
                    .as_ref()
                    .map(|m| m.amount())
                    .unwrap_or(spot);
                observed_max.max(spot) - spot
            }
        },
    };
    Ok(payoff)
}

/// Helper to collect inputs for lookback option pricing.
fn collect_lookback_inputs(
    inst: &LookbackOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(f64, f64, f64, f64, f64)> {
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;

    let disc_curve = curves.get_discount(inst.discount_curve_id.as_str())?;
    let df = disc_curve.df_between_dates(as_of, inst.expiry)?;
    // Keep analytical rate consistent with date-based discounting used by the curve.
    let r = if t > 0.0 && df > 0.0 {
        -df.ln() / t
    } else {
        0.0
    };

    let spot_scalar = curves.get_price(&inst.spot_id)?;
    let spot = match spot_scalar {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
    };

    let q = crate::instruments::common_impl::helpers::resolve_optional_dividend_yield(
        curves,
        inst.div_yield_id.as_ref(),
    )?;

    let strike_val = inst.strike.unwrap_or(spot);
    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &inst.pricing_overrides.market_quotes,
        curves,
        inst.vol_surface_id.as_str(),
        t,
        strike_val,
    )?;

    Ok((spot, r, q, sigma, t))
}

/// Lookback option analytical pricer (continuous monitoring).
pub(crate) struct LookbackOptionAnalyticalPricer;

impl LookbackOptionAnalyticalPricer {
    /// Create a new analytical lookback option pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for LookbackOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for LookbackOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::LookbackOption,
            ModelKey::LookbackBSContinuous,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let lookback = instrument
            .as_any()
            .downcast_ref::<LookbackOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::LookbackOption, instrument.key())
            })?;

        if as_of >= lookback.expiry {
            let spot = lookback_spot(market, &lookback.spot_id).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            let payoff = expired_lookback_payoff(lookback, spot).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            return Ok(ValuationResult::stamped(
                lookback.id(),
                as_of,
                Money::new(
                    payoff * lookback.notional.amount(),
                    lookback.notional.currency(),
                ),
            ));
        }

        let (spot, r, q, sigma, t) =
            collect_lookback_inputs(lookback, market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if t <= 0.0 {
            let payoff = expired_lookback_payoff(lookback, spot).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            return Ok(ValuationResult::stamped(
                lookback.id(),
                as_of,
                Money::new(
                    payoff * lookback.notional.amount(),
                    lookback.notional.currency(),
                ),
            ));
        }

        // Determine current extremum based on option type
        let spot_extremum = match lookback.lookback_type {
            LookbackType::FixedStrike => match lookback.option_type {
                crate::instruments::OptionType::Call => {
                    // Fixed Strike Call: Payoff = max(S_max - K, 0)
                    // Need max(observed_max, current_spot)
                    let obs_max = lookback
                        .observed_max
                        .as_ref()
                        .map(|m| m.amount())
                        .unwrap_or(spot);
                    obs_max.max(spot)
                }
                crate::instruments::OptionType::Put => {
                    // Fixed Strike Put: Payoff = max(K - S_min, 0)
                    // Need min(observed_min, current_spot)
                    let obs_min = lookback
                        .observed_min
                        .as_ref()
                        .map(|m| m.amount())
                        .unwrap_or(spot);
                    obs_min.min(spot)
                }
            },
            LookbackType::FloatingStrike => match lookback.option_type {
                crate::instruments::OptionType::Call => {
                    // Floating Strike Call: Payoff = S_T - S_min
                    // Need min(observed_min, current_spot)
                    let obs_min = lookback
                        .observed_min
                        .as_ref()
                        .map(|m| m.amount())
                        .unwrap_or(spot);
                    obs_min.min(spot)
                }
                crate::instruments::OptionType::Put => {
                    // Floating Strike Put: Payoff = S_max - S_T
                    // Need max(observed_max, current_spot)
                    let obs_max = lookback
                        .observed_max
                        .as_ref()
                        .map(|m| m.amount())
                        .unwrap_or(spot);
                    obs_max.max(spot)
                }
            },
        };

        let price = match lookback.lookback_type {
            LookbackType::FixedStrike => {
                let strike = lookback.strike.as_ref().ok_or_else(|| {
                    PricingError::model_failure_with_context(
                        "FixedStrike lookback requires a strike",
                        PricingErrorContext::default(),
                    )
                })?;
                match lookback.option_type {
                    crate::instruments::OptionType::Call => {
                        fixed_strike_lookback_call(spot, *strike, t, r, q, sigma, spot_extremum)
                    }
                    crate::instruments::OptionType::Put => {
                        fixed_strike_lookback_put(spot, *strike, t, r, q, sigma, spot_extremum)
                    }
                }
            }
            LookbackType::FloatingStrike => match lookback.option_type {
                crate::instruments::OptionType::Call => {
                    floating_strike_lookback_call(spot, t, r, q, sigma, spot_extremum)
                }
                crate::instruments::OptionType::Put => {
                    floating_strike_lookback_put(spot, t, r, q, sigma, spot_extremum)
                }
            },
        };

        let currency = lookback.notional.currency();
        let pv = Money::new(price * lookback.notional.amount(), currency);
        Ok(ValuationResult::stamped(lookback.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::lookback_option::{LookbackOption, LookbackType};
    use crate::instruments::{Attributes, OptionType, PricingOverrides};
    use crate::models::closed_form::lookback::{
        fixed_strike_lookback_call, fixed_strike_lookback_put, floating_strike_lookback_call,
        floating_strike_lookback_put,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (10.0, (-rate * 10.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX-VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 100.0, 120.0, 150.0])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol])
            .build()
            .expect("vol surface");

        MarketContext::new()
            .insert(curve)
            .insert_surface(surface)
            .insert_price(
                "SPX-SPOT",
                MarketScalar::Price(Money::new(spot, Currency::USD)),
            )
            .insert_price("SPX-DIV", MarketScalar::Unitless(div_yield))
    }

    fn fixed_strike_call(expiry: Date, strike: f64, observed_max: Option<f64>) -> LookbackOption {
        LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-FIXED-CALL"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(Some(strike))
            .option_type(OptionType::Call)
            .lookback_type(LookbackType::FixedStrike)
            .expiry(expiry)
            .notional(Money::new(1.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .observed_max_opt(observed_max.map(|value| Money::new(value, Currency::USD)))
            .attributes(Attributes::new())
            .build()
            .expect("lookback option")
    }

    fn fixed_strike_put(expiry: Date, strike: f64, observed_min: Option<f64>) -> LookbackOption {
        LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-FIXED-PUT"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(Some(strike))
            .option_type(OptionType::Put)
            .lookback_type(LookbackType::FixedStrike)
            .expiry(expiry)
            .notional(Money::new(1.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .observed_min_opt(observed_min.map(|value| Money::new(value, Currency::USD)))
            .attributes(Attributes::new())
            .build()
            .expect("lookback option")
    }

    fn floating_strike_call(expiry: Date, observed_min: Option<f64>) -> LookbackOption {
        LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-FLOAT-CALL"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(None)
            .option_type(OptionType::Call)
            .lookback_type(LookbackType::FloatingStrike)
            .expiry(expiry)
            .notional(Money::new(1.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .observed_min_opt(observed_min.map(|value| Money::new(value, Currency::USD)))
            .attributes(Attributes::new())
            .build()
            .expect("lookback option")
    }

    fn floating_strike_put(expiry: Date, observed_max: Option<f64>) -> LookbackOption {
        LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-FLOAT-PUT"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(None)
            .option_type(OptionType::Put)
            .lookback_type(LookbackType::FloatingStrike)
            .expiry(expiry)
            .notional(Money::new(1.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .observed_max_opt(observed_max.map(|value| Money::new(value, Currency::USD)))
            .attributes(Attributes::new())
            .build()
            .expect("lookback option")
    }

    #[test]
    fn analytical_pricer_matches_fixed_strike_lookback_call_benchmark() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        let observed_max = 120.0;
        let rate = 0.05;
        let div_yield = 0.0;
        let vol = 0.20;

        let option = fixed_strike_call(expiry, strike, Some(observed_max));
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("lookback pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let expected = fixed_strike_lookback_call(
            spot,
            strike,
            t,
            rate,
            div_yield,
            vol,
            observed_max.max(spot),
        );

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn analytical_pricer_matches_floating_strike_lookback_put_benchmark() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let spot = 100.0;
        let observed_max = 130.0;
        let rate = 0.03;
        let div_yield = 0.01;
        let vol = 0.25;

        let option = floating_strike_put(expiry, Some(observed_max));
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("lookback pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let expected =
            floating_strike_lookback_put(spot, t, rate, div_yield, vol, observed_max.max(spot));

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn analytical_pricer_matches_fixed_strike_lookback_put_benchmark() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        let observed_min = 82.0;
        let rate = 0.03;
        let div_yield = 0.01;
        let vol = 0.25;

        let option = fixed_strike_put(expiry, strike, Some(observed_min));
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("lookback pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let expected = fixed_strike_lookback_put(
            spot,
            strike,
            t,
            rate,
            div_yield,
            vol,
            observed_min.min(spot),
        );

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn analytical_pricer_matches_floating_strike_lookback_call_benchmark() {
        let as_of = date(2025, 1, 1);
        let expiry = date(2026, 1, 1);
        let spot = 100.0;
        let observed_min = 85.0;
        let rate = 0.04;
        let div_yield = 0.0;
        let vol = 0.18;

        let option = floating_strike_call(expiry, Some(observed_min));
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("lookback pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let expected =
            floating_strike_lookback_call(spot, t, rate, div_yield, vol, observed_min.min(spot));

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn expired_lookback_payoff_covers_remaining_option_shapes() {
        let expiry = date(2025, 1, 1);

        let fixed_put = fixed_strike_put(expiry, 100.0, Some(82.0));
        let floating_call = floating_strike_call(expiry, Some(84.0));
        let floating_put = floating_strike_put(expiry, Some(130.0));

        let fixed_put_payoff = expired_lookback_payoff(&fixed_put, 95.0).expect("fixed put payoff");
        let floating_call_payoff =
            expired_lookback_payoff(&floating_call, 96.0).expect("floating call payoff");
        let floating_put_payoff =
            expired_lookback_payoff(&floating_put, 96.0).expect("floating put payoff");

        assert!((fixed_put_payoff - 18.0).abs() < 1e-12);
        assert!((floating_call_payoff - 12.0).abs() < 1e-12);
        assert!((floating_put_payoff - 34.0).abs() < 1e-12);
    }

    #[test]
    fn expired_lookback_payoff_requires_strike_for_fixed_strike_options() {
        let option = LookbackOption::builder()
            .id(InstrumentId::new("LOOKBACK-MISSING-STRIKE"))
            .underlying_ticker("SPX".to_string())
            .strike_opt(None)
            .option_type(OptionType::Put)
            .lookback_type(LookbackType::FixedStrike)
            .expiry(date(2025, 1, 1))
            .notional(Money::new(1.0, Currency::USD))
            .day_count(DayCount::Act365F)
            .discount_curve_id(CurveId::new("USD-OIS"))
            .spot_id("SPX-SPOT".into())
            .vol_surface_id(CurveId::new("SPX-VOL"))
            .div_yield_id_opt(Some(CurveId::new("SPX-DIV")))
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("lookback option");

        let err = expired_lookback_payoff(&option, 100.0).expect_err("missing strike should error");
        assert!(err.to_string().contains("requires a strike"));
    }

    /// Verify that the lookback MC pricer includes the terminal step in the running extremum.
    ///
    /// A fixed-strike lookback call tracks S_max over the whole path; with a 2-step grid
    /// (`steps_per_year=2`, `min_steps=2`, T=1yr) the engine fires `on_event` at steps
    /// {0, 1, 2}.  The payoff struct gates on `step <= maturity_step`:
    ///
    /// - **Pre-fix** (`maturity_step = num_steps - 1 = 1`): extremum tracked only at steps
    ///   {0, 1}; terminal spot S_T (step 2) is excluded from the running maximum.
    ///   Measured pre-fix MC price (200k paths, seed 20240101): ≈ 6.72.
    /// - **Post-fix** (`maturity_step = num_steps = 2`): all three steps {0, 1, 2} included.
    ///   Measured post-fix MC price (200k paths, seed 20240101): ≈ 12.21.
    ///
    /// Gap ≈ 5.5 price units; 5-σ MC noise ≈ 12/sqrt(200_000) * 5 ≈ 0.13.
    /// The sentinel floor at 9.0 (midpoint) is 24 σ above the pre-fix result
    /// and 12 σ below the post-fix result — a clean, noise-immune pass/fail boundary.
    ///
    /// Note: the 2-step MC (discrete monitoring at 3 points) is naturally below the
    /// analytical continuous-monitoring price (≈ 17.24); both the floor and ceiling
    /// are calibrated to the discrete-monitoring regime, not the continuous formula.
    #[test]
    fn lookback_mc_includes_terminal_step_in_extremum() {
        use finstack_quant_monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1-year
        let spot = 100.0_f64;
        let strike = 100.0_f64;
        let vol = 0.20_f64;
        let rate = 0.05_f64;
        let q = 0.0_f64;

        // Analytical continuous-monitoring reference for context only (Goldman-Sosin-Gatto).
        // The 2-step MC price is below this because discrete monitoring observes fewer highs.
        let t_analytical = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let analytical_ref =
            fixed_strike_lookback_call(spot, strike, t_analytical, rate, q, vol, spot);

        // 2-step grid: num_steps = max(round(t * 2.0), 2) = 2.
        // Pre-fix (maturity_step=1): S_T at step 2 excluded from max. MC price ≈ 6.72.
        // Post-fix (maturity_step=2): S_T included. MC price ≈ 12.21.
        let mc_pricer = LookbackOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 200_000,
                seed: 20240101,
                steps_per_year: 2.0, // forces num_steps = 2
                min_steps: 2,
                ..Default::default()
            },
        };

        let option = fixed_strike_call(expiry, strike, None);
        let mkt = market(as_of, spot, vol, rate, q);

        let mc_pv = mc_pricer
            .price_internal(&option, &mkt, as_of)
            .expect("mc price")
            .amount();

        // Floor at 9.0: midpoint of [6.72, 12.21]; fails on pre-fix, passes on post-fix.
        // Ceiling at 16.0: sanity bound well above post-fix (≈ 12.21).
        let floor = 9.0_f64;
        let ceil = 16.0_f64;

        println!("Analytical (continuous) ref:    {analytical_ref:.6}");
        println!("MC price (2-step, 200k paths):  {mc_pv:.6}");
        println!("Floor (pre-fix ≈ 6.72 fails):   {floor:.6}");
        println!("Ceil  (sanity, pre-fix passes):  {ceil:.6}");

        assert!(
            mc_pv > floor,
            "MC fixed-strike lookback call {mc_pv:.6} is at or below floor {floor:.6}. \
             Likely cause: maturity_step set one step too early (pre-fix ≈ 6.72), \
             so terminal spot S_T is excluded from the running maximum. \
             Analytical continuous-monitoring ref: {analytical_ref:.6}",
        );
        assert!(
            mc_pv < ceil,
            "MC fixed-strike lookback call {mc_pv:.6} exceeds ceiling {ceil:.6}; \
             possible model error. Analytical ref: {analytical_ref:.6}"
        );
    }
}
