//! Barrier option pricers (Monte Carlo and analytical).

// Common imports for all pricers
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::exotics::barrier_option::types::BarrierOption;
use crate::pricer::{
    expect_inst, InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

// MC-specific imports
use finstack_quant_models::monte_carlo::payoff::barrier::{BarrierOptionPayoff, OptionKind};
use finstack_quant_models::monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_models::monte_carlo::process::gbm::{GbmParams, GbmProcess};

// Both engines resolve the same dated discount factor and active volatility quote.
pub(crate) fn collect_barrier_inputs(
    inst: &BarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<crate::instruments::common_impl::helpers::BlackScholesInputsDf> {
    use crate::instruments::common_impl::helpers::{
        resolve_optional_dividend_yield, BlackScholesInputsDf,
    };
    let t = inst.day_count.year_fraction(
        as_of,
        inst.expiry,
        finstack_quant_core::dates::DayCountContext::default(),
    )?;
    let df = curves
        .get_discount(inst.discount_curve_id.as_str())?
        .df_between_dates(as_of, inst.expiry)?;
    if !df.is_finite() || df <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(
            "Barrier discount factor must be positive and finite".into(),
        ));
    }
    let spot = match curves.get_price(&inst.spot_id)? {
        finstack_quant_core::market_data::scalars::MarketScalar::Unitless(value) => *value,
        finstack_quant_core::market_data::scalars::MarketScalar::Price(value) => {
            if value.currency() != inst.notional.currency() {
                return Err(finstack_quant_core::Error::CurrencyMismatch {
                    expected: inst.notional.currency(),
                    actual: value.currency(),
                });
            }
            value.amount()
        }
    };
    crate::instruments::common_impl::validation::validate_f64_positive(
        spot,
        "BarrierOption asset spot",
    )?;
    let q = resolve_optional_dividend_yield(curves, inst.div_yield_id.as_ref())?;
    let sigma = crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
        &inst.instrument_pricing_overrides.market_quotes,
        curves,
        inst.vol_surface_id.as_str(),
        t,
        inst.strike,
    )?;
    Ok(BlackScholesInputsDf {
        spot,
        df,
        q,
        sigma,
        t,
    })
}

/// Remaining known knock-out rebate, independent of spot, volatility and notional.
pub(crate) fn known_knock_out_value(
    inst: &BarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Option<Money>> {
    if inst.observed_barrier_breached != Some(true) || !inst.barrier_type.is_knock_out() {
        return Ok(None);
    }
    let mut value = match inst.rebate_timing {
        finstack_quant_models::closed_form::barrier::RebateTiming::AtHit => 0.0,
        finstack_quant_models::closed_form::barrier::RebateTiming::AtExpiry => {
            inst.rebate.map_or(0.0, |money| money.amount())
        }
    };
    if value != 0.0 && as_of < inst.expiry {
        value *= curves
            .get_discount(inst.discount_curve_id.as_str())?
            .df_between_dates(as_of, inst.expiry)?;
    }
    Money::new(value, inst.notional.currency()).map(Some)
}

/// Whether the instrument's rebate should be paid at the hit time.
///
/// When true, the MC payoff is configured via
/// [`BarrierOptionPayoff::with_rebate_at_hit`] so the rebate compounds
/// forward from the hit time τ at the flat rate and the engine's maturity
/// discount factor nets to `DF(τ)` — exact at-hit discounting, matching the
/// analytical [`finstack_quant_models::closed_form::barrier::barrier_rebate`] with
/// [`RebateTiming::AtHit`](finstack_quant_models::closed_form::barrier::RebateTiming::AtHit).
pub(crate) fn wants_at_hit_rebate(inst: &BarrierOption) -> bool {
    use finstack_quant_models::closed_form::barrier::RebateTiming;
    inst.rebate.is_some() && inst.rebate_timing == RebateTiming::AtHit
}

/// Barrier option Monte Carlo pricer.
pub struct BarrierOptionMcPricer {
    config: PathDependentPricerConfig,
}

impl BarrierOptionMcPricer {
    /// Create a new barrier option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    fn convert_option_kind(option_type: crate::instruments::OptionType) -> OptionKind {
        match option_type {
            crate::instruments::OptionType::Call => OptionKind::Call,
            crate::instruments::OptionType::Put => OptionKind::Put,
        }
    }

    /// Price a barrier option using Monte Carlo.
    ///
    /// # Day Count Convention Handling
    ///
    /// Uses separate day count bases for different purposes:
    /// - **Discounting**: Uses the discount curve's own day count for DF and zero rate calculations
    /// - **Volatility lookup**: Uses the instrument's day count (assumed to match vol surface calibration)
    /// - **Monte Carlo time grid**: Uses the vol surface time basis for proper barrier monitoring
    fn price_internal(
        &self,
        inst: &BarrierOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        inst.validate_monitoring_state(as_of)?;
        if as_of >= inst.expiry {
            return price_expired_barrier(inst, curves, as_of);
        }
        if let Some(value) = known_knock_out_value(inst, curves, as_of)? {
            return Ok(value);
        }
        let inputs = collect_barrier_inputs(inst, curves, as_of)?;
        let (spot, q, sigma, t_vol, discount_factor) =
            (inputs.spot, inputs.q, inputs.sigma, inputs.t, inputs.df);
        let r = inputs.r_eff();

        if inst.observed_barrier_breached == Some(true) {
            let unit = finstack_quant_models::closed_form::vanilla::bs_price_unchecked(
                spot,
                inst.strike,
                r,
                q,
                sigma,
                t_vol,
                inst.option_type,
            );
            let unit = finstack_quant_models::closed_form::checked_closed_form_value(
                unit,
                "barrier knocked-in vanilla price",
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            return Money::new(unit * inst.notional.amount(), inst.notional.currency());
        }

        let gbm_params = GbmParams::new(r, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let mut config = crate::instruments::common_impl::helpers::merged_path_config(
            &self.config,
            &inst.instrument_pricing_overrides,
        )?;
        let (time_grid, monitoring) =
            inst.monitoring
                .time_grid(as_of, inst.day_count, None, t_vol, &config)?;
        let maturity_step = time_grid.num_steps();

        // Create payoff (using vol surface time for barrier adjustment calculations)
        let mut payoff = BarrierOptionPayoff::new(
            inst.strike,
            inst.barrier.amount(),
            inst.barrier_type,
            Self::convert_option_kind(inst.option_type),
            inst.rebate.map(|m| m.amount() / inst.notional.amount()),
            inst.notional.amount(),
            maturity_step,
            sigma,
            &time_grid,
            monitoring,
        );
        if wants_at_hit_rebate(inst) {
            payoff = payoff.with_rebate_at_hit(r);
        }

        // Derive deterministic seed from instrument ID and scenario

        use finstack_quant_models::monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.metric_pricing_overrides.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        config.seed = seed;

        // Price using path-dependent pricer (using vol surface time basis for simulation)
        let pricer = PathDependentPricer::new(config);
        let result = pricer.price_with_grid(
            &process,
            spot,
            time_grid,
            &payoff,
            inst.notional.currency(),
            discount_factor,
        )?;

        Ok(result.mean)
    }
}

impl Default for BarrierOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for BarrierOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BarrierOption, ModelKey::MonteCarloGBM)
    }

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let barrier = expect_inst::<BarrierOption>(instrument, InstrumentType::BarrierOption)?;

        let pv = self.price_internal(barrier, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(barrier.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &BarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    let pricer = BarrierOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

/// Price an expired barrier option using explicit observed barrier state.
///
/// Terminal spot alone is insufficient to determine whether a barrier was
/// breached intralife and then later reversed, so expired contracts require
/// the caller to provide `observed_barrier_breached`.
/// The intrinsic value is `max(S - K, 0)` for calls and `max(K - S, 0)` for puts,
/// scaled by notional.
pub(crate) fn price_expired_barrier(
    inst: &BarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    inst.validate_monitoring_state(as_of)?;
    if let Some(value) = known_knock_out_value(inst, curves, as_of)? {
        return Ok(value);
    }
    let spot = if let Some(fixing) = inst.expiry_fixing {
        fixing.amount()
    } else if as_of == inst.expiry {
        let spot_scalar = curves.get_price(&inst.spot_id)?;
        match spot_scalar {
            finstack_quant_core::market_data::scalars::MarketScalar::Unitless(v) => *v,
            finstack_quant_core::market_data::scalars::MarketScalar::Price(m) => m.amount(),
        }
    } else {
        return Err(finstack_quant_core::Error::Validation(format!(
            "BarrierOption '{}' requires expiry_fixing when valued after expiry {}",
            inst.id, inst.expiry
        )));
    };

    let ccy = inst.notional.currency();
    let notional = inst.notional.amount();
    let is_knock_out = inst.barrier_type.is_knock_out();

    let barrier_breached = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Expired barrier option requires `observed_barrier_breached` to determine realized payoff"
                .to_string(),
        )
    })?;

    let intrinsic = match inst.option_type {
        crate::instruments::OptionType::Call => (spot - inst.strike).max(0.0) * notional,
        crate::instruments::OptionType::Put => (inst.strike - spot).max(0.0) * notional,
    };
    let rebate = inst.rebate.map(|m| m.amount()).unwrap_or(0.0);

    let pv = if is_knock_out {
        if barrier_breached {
            match inst.rebate_timing {
                finstack_quant_models::closed_form::barrier::RebateTiming::AtHit => 0.0,
                finstack_quant_models::closed_form::barrier::RebateTiming::AtExpiry => rebate,
            }
        } else {
            intrinsic
        }
    } else {
        // Knock-in
        if barrier_breached {
            intrinsic
        } else {
            rebate
        }
    };

    Money::new(pv, ccy)
}

use finstack_quant_models::closed_form::barrier::{
    barrier_call_continuous, barrier_put_continuous, barrier_rebate, BarrierParams,
};
/// Reiner-Rubinstein pricer for continuous contractual monitoring.
/// Discrete observation dates require the Monte Carlo or PDE engine.
pub(crate) struct BarrierOptionAnalyticalPricer;

impl BarrierOptionAnalyticalPricer {
    /// Create a new analytical barrier option pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for BarrierOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for BarrierOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::BarrierOption, ModelKey::BarrierBSContinuous)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let barrier_opt = expect_inst::<BarrierOption>(instrument, InstrumentType::BarrierOption)?;

        barrier_opt
            .validate_monitoring_state(as_of)
            .map_err(|error| {
                PricingError::from_core(error, PricingErrorContext::from_instrument(barrier_opt))
            })?;
        if matches!(
            barrier_opt.monitoring,
            crate::instruments::Monitoring::Discrete { .. }
        ) {
            return Err(PricingError::model_failure_with_context(
                "Analytical barrier pricing requires continuous monitoring; use MC or PDE for discrete observation dates".to_string(),
                PricingErrorContext::from_instrument(barrier_opt),
            ));
        }

        if as_of >= barrier_opt.expiry {
            let pv = price_expired_barrier(barrier_opt, market, as_of).map_err(|error| {
                PricingError::model_failure_with_context(
                    error.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            return Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv));
        }
        if let Some(value) = known_knock_out_value(barrier_opt, market, as_of).map_err(|error| {
            PricingError::from_core(error, PricingErrorContext::from_instrument(barrier_opt))
        })? {
            return Ok(ValuationResult::stamped(barrier_opt.id(), as_of, value));
        }
        let bs_inputs = collect_barrier_inputs(barrier_opt, market, as_of).map_err(|error| {
            PricingError::model_failure_with_context(
                error.to_string(),
                PricingErrorContext::default(),
            )
        })?;
        let (spot, q, sigma, t, df) = (
            bs_inputs.spot,
            bs_inputs.q,
            bs_inputs.sigma,
            bs_inputs.t,
            bs_inputs.df,
        );

        if barrier_opt.observed_barrier_breached == Some(true) {
            let unit = finstack_quant_models::closed_form::vanilla::bs_price_unchecked(
                spot,
                barrier_opt.strike,
                bs_inputs.r_eff(),
                q,
                sigma,
                t,
                barrier_opt.option_type,
            );
            let unit = finstack_quant_models::closed_form::checked_closed_form_value(
                unit,
                "barrier observed-breach vanilla price",
            )
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            let pv = Money::new(
                unit * barrier_opt.notional.amount(),
                barrier_opt.notional.currency(),
            )
            .map_err(|error| {
                crate::pricer::PricingError::from_core(
                    error,
                    crate::pricer::PricingErrorContext::from_instrument(barrier_opt),
                )
            })?;
            return Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv));
        }

        let analytical_barrier_type = barrier_opt.barrier_type;

        let effective_barrier = barrier_opt.barrier.amount();

        let params =
            BarrierParams::with_df(spot, barrier_opt.strike, effective_barrier, t, df, q, sigma)
                .map_err(|e| {
                    PricingError::model_failure_with_context(
                        e.to_string(),
                        PricingErrorContext::default(),
                    )
                })?;
        let price = match barrier_opt.option_type {
            crate::instruments::OptionType::Call => {
                barrier_call_continuous(&params, analytical_barrier_type)
            }
            crate::instruments::OptionType::Put => {
                barrier_put_continuous(&params, analytical_barrier_type)
            }
        };

        let rebate_val = if let Some(rebate) = barrier_opt.rebate {
            barrier_rebate(
                &params,
                rebate.amount(),
                analytical_barrier_type,
                barrier_opt.rebate_timing,
            )
        } else {
            0.0
        };
        // The closed-form leaves return NaN sentinels for out-of-domain input;
        // convert that to an error before `Money::new` panics on non-finite.
        let price = finstack_quant_models::closed_form::checked_closed_form_value(
            price * barrier_opt.notional.amount() + rebate_val,
            "barrier closed-form price",
        )
        .map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let pv = Money::new(price, barrier_opt.notional.currency()).map_err(|error| {
            crate::pricer::PricingError::from_core(
                error,
                crate::pricer::PricingErrorContext::from_instrument(barrier_opt),
            )
        })?;
        Ok(ValuationResult::stamped(barrier_opt.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::barrier_option::types::BarrierOption;
    use crate::instruments::{Attributes, OptionType};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{DayCount, DayCountContext};
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::types::BarrierType;
    use finstack_quant_core::types::BarrierType as AnalyticalBarrierType;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_models::closed_form::barrier::{
        barrier_put_continuous, barrier_rebate, down_out_call, BarrierParams, RebateTiming,
    };
    use time::Month;

    fn date(year: i32, month: u8, day: u8) -> Date {
        Date::from_calendar_date(year, Month::try_from(month).expect("valid month"), day)
            .expect("valid date")
    }

    fn market(as_of: Date, spot: f64, vol: f64, rate: f64, div_yield: f64) -> MarketContext {
        let discount = DiscountCurve::builder("USD_DISC")
            .base_date(as_of)
            .day_count(DayCount::Act365F)
            .knots([(0.0, 1.0), (5.0, (-rate * 5.0).exp())])
            .build()
            .expect("discount curve");
        let surface = VolSurface::builder("SPX_VOL")
            .expiries(&[0.25, 0.5, 1.0, 2.0])
            .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .row(&[vol, vol, vol, vol, vol])
            .build()
            .expect("vol surface");

        MarketContext::new()
            .insert(discount)
            .insert_surface(surface)
            .insert_price(
                "SPX",
                MarketScalar::Price(Money::new(spot, Currency::USD).expect("valid money fixture")),
            )
            .insert_price("SPX_DIV", MarketScalar::Unitless(div_yield))
    }

    fn down_and_out_call(expiry: Date, strike: f64, barrier: f64) -> BarrierOption {
        BarrierOption {
            id: InstrumentId::new("BARRIER-BENCH"),
            underlying_ticker: "SPX".to_string(),
            strike,
            barrier: Money::new(barrier, Currency::USD).expect("valid money fixture"),
            rebate: None,
            rebate_timing: Default::default(),
            option_type: OptionType::Call,
            barrier_type: BarrierType::DownAndOut,
            expiry,
            observed_barrier_breached: None,
            expiry_fixing: None,
            notional: Money::from((1_i64, Currency::USD)),
            day_count: DayCount::Act365F,
            monitoring: crate::instruments::Monitoring::Continuous,
            discount_curve_id: "USD_DISC".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX_VOL".into(),
            div_yield_id: Some("SPX_DIV".into()),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        }
    }

    #[test]
    fn analytical_pricer_matches_reiner_rubinstein_down_and_out_call() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2024, 7, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 80.0;
        let vol = 0.20;
        let rate = 0.05;
        let div_yield = 0.0;

        let option = down_and_out_call(expiry, strike, barrier);
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("barrier pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let expected = down_out_call(spot, strike, barrier, t, rate, div_yield, vol);

        assert!((pv - expected).abs() < 1e-12);
    }

    #[test]
    fn analytical_pricer_adds_reiner_rubinstein_rebate_value() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1);
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 120.0;
        let vol = 0.18;
        let rate = 0.04;
        let div_yield = 0.01;
        let rebate = 2.5;

        let market = market(as_of, spot, vol, rate, div_yield);
        let base = BarrierOption {
            barrier_type: BarrierType::UpAndOut,
            option_type: OptionType::Call,
            barrier: Money::new(barrier, Currency::USD).expect("valid money fixture"),
            ..down_and_out_call(expiry, strike, barrier)
        };
        let with_rebate = BarrierOption {
            rebate: Some(Money::new(rebate, Currency::USD).expect("valid money fixture")),
            ..base.clone()
        };
        let with_rebate_at_expiry = BarrierOption {
            rebate_timing: finstack_quant_models::closed_form::barrier::RebateTiming::AtExpiry,
            ..with_rebate.clone()
        };

        let base_pv = base.value(&market, as_of).expect("base pv").amount();
        let rebate_pv = with_rebate
            .value(&market, as_of)
            .expect("rebate pv")
            .amount();
        let rebate_pv_at_expiry = with_rebate_at_expiry
            .value(&market, as_of)
            .expect("rebate pv at expiry")
            .amount();

        let t = with_rebate
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let p = BarrierParams::new(spot, barrier, barrier, t, rate, div_yield, vol);

        // Default timing is at-hit (market standard).
        let expected_at_hit = finstack_quant_models::closed_form::barrier::barrier_rebate(
            &p,
            rebate,
            AnalyticalBarrierType::UpAndOut,
            finstack_quant_models::closed_form::barrier::RebateTiming::AtHit,
        );
        assert!(((rebate_pv - base_pv) - expected_at_hit).abs() < 1e-12);

        // Explicit AtExpiry reproduces the legacy pay-at-expiry value.
        let expected_at_expiry = barrier_rebate(
            &p,
            rebate,
            AnalyticalBarrierType::UpAndOut,
            RebateTiming::AtExpiry,
        );
        assert!(((rebate_pv_at_expiry - base_pv) - expected_at_expiry).abs() < 1e-12);

        // At-hit must dominate at-expiry under positive rates.
        assert!(rebate_pv >= rebate_pv_at_expiry);
    }

    #[test]
    fn expired_barrier_paths_cover_knock_in_and_knock_out_matrix() {
        let curves = MarketContext::new().insert_price("SPX", MarketScalar::Unitless(120.0));
        let base = down_and_out_call(date(2024, 7, 1), 100.0, 80.0);

        let knocked_out = BarrierOption {
            rebate: Some(Money::from((3_i64, Currency::USD))),
            rebate_timing: RebateTiming::AtExpiry,
            observed_barrier_breached: Some(true),
            ..base.clone()
        };
        let alive_knock_out = BarrierOption {
            observed_barrier_breached: Some(false),
            ..base.clone()
        };
        let knocked_in = BarrierOption {
            barrier_type: BarrierType::UpAndIn,
            observed_barrier_breached: Some(true),
            ..base.clone()
        };
        let no_hit_knock_in = BarrierOption {
            barrier_type: BarrierType::UpAndIn,
            rebate: Some(Money::new(2.5, Currency::USD).expect("valid money fixture")),
            observed_barrier_breached: Some(false),
            ..base
        };

        assert_eq!(
            price_expired_barrier(&knocked_out, &curves, date(2024, 7, 1))
                .expect("ko")
                .amount(),
            3.0
        );
        assert_eq!(
            price_expired_barrier(&alive_knock_out, &curves, date(2024, 7, 1))
                .expect("alive ko")
                .amount(),
            20.0
        );
        assert_eq!(
            price_expired_barrier(&knocked_in, &curves, date(2024, 7, 1))
                .expect("ki")
                .amount(),
            20.0
        );
        assert_eq!(
            price_expired_barrier(&no_hit_knock_in, &curves, date(2024, 7, 1))
                .expect("no hit ki")
                .amount(),
            2.5
        );
    }

    #[test]
    fn analytical_pricer_rejects_discrete_monitoring() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2024, 7, 1);
        let mut option = down_and_out_call(expiry, 100.0, 80.0);
        option.monitoring = crate::instruments::Monitoring::Discrete {
            observation_dates: vec![expiry],
        };
        let market = market(as_of, 100.0, 0.2, 0.05, 0.0);
        let error = BarrierOptionAnalyticalPricer::new()
            .price_dyn(&option, &market, as_of)
            .expect_err("discrete contract cannot use continuous formula");
        assert!(error.to_string().contains("continuous monitoring"));
    }

    /// Curves with different native day-count conventions but the same exact
    /// expiry DF must produce the same MC price. The stochastic process evolves
    /// on the instrument/volatility clock, so its rate must satisfy
    /// `exp(-r_model * t_vol) = DF`; using `-ln(DF) / t_disc` would create a
    /// spurious price difference.

    #[test]
    fn model_clock_drift_is_invariant_to_curve_day_count_for_same_df() {
        use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1 calendar year
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 75.0;
        let vol = 0.25;
        let rate = 0.05;
        let div_yield = 0.0;

        let t_365 = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("Act/365F year fraction");
        let t_360 = DayCount::Act360
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("Act/360 year fraction");
        let df_at_expiry = (-rate * t_365).exp();

        // Put the common expiry DF at the expiry's native year-fraction knot
        // on each curve. The curves therefore differ only in their coordinate
        // system, not in the economic discount factor supplied to the pricer.
        let market_365 = {
            let disc = DiscountCurve::builder("USD_DISC")
                .base_date(as_of)
                .day_count(DayCount::Act365F)
                .knots([(0.0, 1.0), (t_365, df_at_expiry)])
                .build()
                .expect("disc curve 365");
            let surface = VolSurface::builder("SPX_VOL")
                .expiries(&[0.25, 0.5, 1.0, 2.0])
                .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .build()
                .expect("surface");
            MarketContext::new()
                .insert(disc)
                .insert_surface(surface)
                .insert_price(
                    "SPX",
                    MarketScalar::Price(
                        Money::new(spot, Currency::USD).expect("valid money fixture"),
                    ),
                )
                .insert_price("SPX_DIV", MarketScalar::Unitless(div_yield))
        };
        let market_360 = {
            let disc = DiscountCurve::builder("USD_DISC")
                .base_date(as_of)
                .day_count(DayCount::Act360)
                .knots([(0.0, 1.0), (t_360, df_at_expiry)])
                .build()
                .expect("disc curve 360");
            let surface = VolSurface::builder("SPX_VOL")
                .expiries(&[0.25, 0.5, 1.0, 2.0])
                .strikes(&[80.0, 90.0, 100.0, 110.0, 120.0])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .row(&[vol; 5])
                .build()
                .expect("surface");
            MarketContext::new()
                .insert(disc)
                .insert_surface(surface)
                .insert_price(
                    "SPX",
                    MarketScalar::Price(
                        Money::new(spot, Currency::USD).expect("valid money fixture"),
                    ),
                )
                .insert_price("SPX_DIV", MarketScalar::Unitless(div_yield))
        };

        // Use the MC pricer explicitly so the drift branch is exercised.
        let mc_pricer = BarrierOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 4_000,
                seed: 42,
                steps_per_year: 50.0,
                min_steps: 50,
                ..Default::default()
            },
        };
        let option = down_and_out_call(expiry, strike, barrier);

        let pv_365 = mc_pricer
            .price_internal(&option, &market_365, as_of)
            .expect("price 365")
            .amount();
        let pv_360 = mc_pricer
            .price_internal(&option, &market_360, as_of)
            .expect("price 360")
            .amount();

        // Both prices must be finite and positive (ITM call, finite DF).
        assert!(pv_365.is_finite() && pv_365 > 0.0);
        assert!(pv_360.is_finite() && pv_360 > 0.0);

        // Identical DF, model clock, vol, spot, and random seed imply identical
        // simulated paths and discounted payoffs.
        let gap = (pv_365 - pv_360).abs();
        assert!(
            gap < 1e-12,
            "curve-coordinate choice changed the price despite identical expiry DFs: \
             pv_365={pv_365}, pv_360={pv_360}, gap={gap}"
        );
    }

    #[test]
    fn analytical_pricer_matches_put_reference_branch() {
        let as_of = date(2024, 1, 1);
        let expiry = date(2024, 9, 1);
        let spot = 100.0;
        let strike = 95.0;
        let barrier = 120.0;
        let vol = 0.22;
        let rate = 0.04;
        let div_yield = 0.01;

        let option = BarrierOption {
            barrier_type: BarrierType::UpAndOut,
            option_type: OptionType::Put,
            barrier: Money::new(barrier, Currency::USD).expect("valid money fixture"),
            ..down_and_out_call(expiry, strike, barrier)
        };
        let market = market(as_of, spot, vol, rate, div_yield);
        let pv = option.value(&market, as_of).expect("put pv").amount();

        let t = option
            .day_count
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");
        let df = (-rate * t).exp();
        let p = BarrierParams::with_df(spot, strike, barrier, t, df, div_yield, vol)
            .expect("positive df constructs");
        let expected = barrier_put_continuous(&p, AnalyticalBarrierType::UpAndOut);

        assert!((pv - expected).abs() < 1e-12);
    }

    /// Verify that the MC pricer captures the terminal spot at the correct maturity step.
    ///
    /// A degenerate up-and-out call with barrier >> spot is effectively a vanilla call
    /// (zero knockout probability). Its MC price must therefore equal the Black-Scholes
    /// call price within MC standard error.
    ///
    /// To make the off-by-one detectable, we use `steps_per_year = 2.0` / `min_steps = 2`
    /// so the time grid has exactly 2 steps (dt = 0.5 years each).  With
    /// `maturity_step = num_steps - 1 = 1`, the payoff reads the spot at t = 0.5
    /// instead of t = 1.0 — a 50% underestimate of the horizon.  The resulting MC
    /// price would equal the BS call at T = 0.5 (≈ 6.89) rather than at T = 1.0
    /// (≈ 10.47), a gap of ~3.6 that is far outside 5 MC standard errors (≈ 0.22).
    ///
    /// After the fix (`maturity_step = num_steps = 2`), the terminal spot is correctly
    /// read at step 2 (t = 1.0) and the MC price converges to the 1-year BS call.
    /// Black-Scholes call price (norm_cdf via Horner rational approximation; max err < 7.5e-8).
    fn bs_call_price(spot: f64, strike: f64, t: f64, r: f64, q: f64, sigma: f64) -> f64 {
        fn n(x: f64) -> f64 {
            if x < -8.0 {
                return 0.0;
            }
            if x > 8.0 {
                return 1.0;
            }
            let tt = 1.0 / (1.0 + 0.2316419 * x.abs());
            let poly = tt
                * (0.319_381_53
                    + tt * (-0.356_563_782
                        + tt * (1.781_477_937 + tt * (-1.821_255_978 + tt * 1.330_274_429))));
            let phi = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
            let c = 1.0 - phi * poly;
            if x >= 0.0 {
                c
            } else {
                1.0 - c
            }
        }
        let d1 = ((spot / strike).ln() + (r - q + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        spot * (-q * t).exp() * n(d1) - strike * (-r * t).exp() * n(d2)
    }

    /// W-43 / W-44 cross-check: the MC barrier pricer with the Brownian
    /// continuous bridge active estimates the
    /// *continuously*-monitored barrier price, because the bridge fills in
    /// between-step crossings. It must therefore agree with the analytical
    /// continuous-monitoring analytical pricer within Monte Carlo error.
    ///
    /// Before W-43 the MC payoff layered the Broadie–Glasserman–Kou barrier
    /// shift on top of the bridge, biasing the price by order `βσ√Δt`; the
    /// parity below would then fail. Before W-44 the shift also pointed the
    /// wrong way, compounding the error.
    #[test]
    fn mc_bridge_barrier_matches_analytical_continuous() {
        use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1 year
        let spot = 100.0;
        let strike = 100.0;
        let barrier = 80.0;
        let vol = 0.20;
        let rate = 0.05;
        let div_yield = 0.0;

        let market = market(as_of, spot, vol, rate, div_yield);

        // Analytical continuous-monitoring reference.
        let analytical = down_and_out_call(expiry, strike, barrier);
        let analytical_pv = analytical
            .value(&market, as_of)
            .expect("analytical pv")
            .amount();

        // MC with the bridge active. A fine time grid keeps the bridge
        // approximation tight; a large path count keeps MC error small.
        let mc_option = BarrierOption {
            monitoring: crate::instruments::Monitoring::Continuous,
            ..down_and_out_call(expiry, strike, barrier)
        };
        let mc_pricer = BarrierOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 200_000,
                seed: 20240101,
                steps_per_year: 100.0,
                min_steps: 100,
                ..Default::default()
            },
        };
        let mc_pv = mc_pricer
            .price_internal(&mc_option, &market, as_of)
            .expect("mc pv")
            .amount();

        // The bridge MC must converge to the continuous-monitoring price.
        // A double-counted BGK shift of order βσ√Δt with σ=0.2, Δt=0.01
        // would bias the barrier by ~1.2% — far outside this tolerance.
        let tol = 0.15;
        assert!(
            (mc_pv - analytical_pv).abs() < tol,
            "MC barrier with bridge must match analytical continuous price \
             within {tol}: mc={mc_pv:.6}, analytical={analytical_pv:.6}, \
             diff={:.6}",
            (mc_pv - analytical_pv).abs()
        );
    }

    #[test]
    fn barrier_uao_degenerate_matches_bs() {
        use finstack_quant_models::monte_carlo::pricer::path_dependent::PathDependentPricerConfig;

        let as_of = date(2024, 1, 1);
        let expiry = date(2025, 1, 1); // 1-year
        let spot = 100.0_f64;
        let strike = 100.0_f64;
        let barrier = 10_000.0_f64; // Far above spot: knockout probability ≈ 0
        let vol = 0.20_f64;
        let rate = 0.05_f64;
        let q = 0.0_f64;

        let t = DayCount::Act365F
            .year_fraction(as_of, expiry, DayCountContext::default())
            .expect("year fraction");

        // Analytic Black-Scholes call at T = 1 year
        let bs_price_t1 = bs_call_price(spot, strike, t, rate, q, vol);

        // 2-step grid: num_steps = max(round(t * 2.0), 2) = 2.
        // Bug: maturity_step = 1 → spot read at t = 0.5 → price ≈ bs_call(t=0.5) ≈ 6.89.
        // Fix: maturity_step = 2 → spot read at t = 1.0 → price ≈ bs_call(t=1.0) ≈ 10.47.
        let mc_pricer = BarrierOptionMcPricer {
            config: PathDependentPricerConfig {
                num_paths: 200_000,
                seed: 20240101,
                steps_per_year: 2.0, // forces num_steps = 2
                min_steps: 2,
                ..Default::default()
            },
        };

        let option = BarrierOption {
            id: InstrumentId::new("BARRIER-UAO-DEGEN-UNIT"),
            expiry_fixing: None,
            underlying_ticker: "SPX".to_string(),
            strike,
            barrier: Money::new(barrier, Currency::USD).expect("valid money fixture"),
            rebate: None,
            rebate_timing: Default::default(),
            option_type: OptionType::Call,
            barrier_type: BarrierType::UpAndOut,
            expiry,
            observed_barrier_breached: None,
            notional: Money::from((1_i64, Currency::USD)),
            day_count: DayCount::Act365F,
            monitoring: crate::instruments::Monitoring::Continuous,
            discount_curve_id: "USD_DISC".into(),
            spot_id: "SPX".into(),
            vol_surface_id: "SPX_VOL".into(),
            div_yield_id: Some("SPX_DIV".into()),
            instrument_pricing_overrides: Default::default(),
            metric_pricing_overrides: Default::default(),
            scenario_pricing_overrides: Default::default(),
            attributes: Attributes::new(),
        };

        let mkt = market(as_of, spot, vol, rate, q);
        let mc_pv = mc_pricer
            .price_internal(&option, &mkt, as_of)
            .expect("mc price")
            .amount();

        // MC std error ≈ BS/sqrt(N) ≈ 10.47/sqrt(200_000) ≈ 0.023.
        // Off-by-one bias with 2 steps ≈ bs_call(T=1) - bs_call(T=0.5) ≈ 3.58.
        // Tight tolerance: 0.25 (≈ 11 std errors).  Bug fails spectacularly; fix passes.
        let tolerance = 0.25_f64;
        println!("BS call (T=1): {bs_price_t1:.6}");
        println!("MC call:       {mc_pv:.6}");
        println!("Difference:    {:.6}", (mc_pv - bs_price_t1).abs());
        println!("Tolerance:     {tolerance:.6}");
        assert!(
            (mc_pv - bs_price_t1).abs() < tolerance,
            "MC up-and-out call with far barrier must match BS call at T=1 within {tolerance}: \
             mc={mc_pv:.6}, bs={bs_price_t1:.6}, diff={:.6}. \
             Likely cause: maturity_step is set one step too early.",
            (mc_pv - bs_price_t1).abs()
        );
    }
}
