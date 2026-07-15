//! FX barrier option pricers (Monte Carlo and analytical).

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fx::fx_barrier_option::types::FxBarrierOption;
use crate::instruments::fx::shared::{
    collect_fx_option_inputs, resolve_fx_spot as resolve_shared_fx_spot, FxOptionInputRequest,
    FxSpotSource,
};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

// MC-specific imports
use crate::instruments::fx::fx_barrier_option::monte_carlo::FxBarrierPayoff;
use finstack_quant_monte_carlo::payoff::barrier::OptionKind as McOptionKind;
use finstack_quant_monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_quant_monte_carlo::process::gbm::{GbmParams, GbmProcess};

/// FX barrier option Monte Carlo pricer.
pub struct FxBarrierOptionMcPricer {
    config: PathDependentPricerConfig,
}

impl FxBarrierOptionMcPricer {
    /// Create a new FX barrier option MC pricer with default config.
    pub fn new() -> Self {
        Self {
            config: PathDependentPricerConfig::default(),
        }
    }

    fn merged_path_config(&self, inst: &FxBarrierOption) -> PathDependentPricerConfig {
        let mut c = self.config.clone();
        if let Some(n) = inst.pricing_overrides.model_config.mc_paths {
            if n > 0 {
                c.num_paths = n;
            }
        }
        c
    }

    /// Price an FX barrier option using Monte Carlo.
    fn price_internal(
        &self,
        inst: &FxBarrierOption,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<finstack_quant_core::money::Money> {
        inst.validate()?;
        if as_of > inst.expiry {
            return Ok(finstack_quant_core::money::Money::new(
                0.0,
                inst.quote_currency,
            ));
        }
        validate_fx_barrier_currencies(inst)?;

        let (fx_spot, t) = collect_fx_barrier_expiry_state(inst, curves, as_of)?;
        if t <= 0.0 {
            let per_unit = expired_barrier_value_per_unit(inst, fx_spot)?;
            return Ok(finstack_quant_core::money::Money::new(
                per_unit * inst.notional.amount(),
                inst.quote_currency,
            ));
        }

        let (_, r_dom, r_for, sigma, discount_factor) =
            collect_fx_barrier_inputs(inst, curves, as_of)?;

        if inst.observed_barrier_breached == Some(true) {
            let per_unit = seasoned_breached_value_per_unit(
                inst,
                fx_spot,
                r_dom,
                r_for,
                sigma,
                t,
                discount_factor,
            );
            return Ok(Money::new(
                per_unit * inst.notional.amount(),
                inst.quote_currency,
            ));
        }

        // For FX, drift is r_dom - r_for.
        // In GBM process param 'q' is subtracted from r to get drift (r-q).
        // So q should be r_for.
        let q = r_for;
        let gbm_params = GbmParams::new(r_dom, q, sigma)?;
        let process = GbmProcess::new(gbm_params);

        let steps_per_year = self.config.steps_per_year;
        let num_steps = ((t * steps_per_year).round() as usize).max(self.config.min_steps);
        let dt = t / num_steps as f64;
        // `maturity_step` must equal `num_steps`: the engine fires `on_event` with
        // `state.step = num_steps` on the last iteration, so the terminal-spot capture
        // guard `state.step == maturity_step` must fire at that step.
        let maturity_step = num_steps;

        // Standard FX barrier: the GBM drift `r_dom - r_for` (set above via
        // `GbmParams`) fully describes the dynamics. Quanto barriers are not
        // supported by this 1D MC payoff — see `FxBarrierPayoff` docs.
        let mc_option_kind = match inst.option_type {
            crate::instruments::OptionType::Call => McOptionKind::Call,
            crate::instruments::OptionType::Put => McOptionKind::Put,
        };
        let mut payoff = FxBarrierPayoff::new(
            inst.strike,
            inst.barrier,
            inst.barrier_type,
            mc_option_kind,
            inst.notional.amount(),
            maturity_step,
            sigma,
            dt,
            inst.use_gobet_miri,
            inst.base_currency,
            inst.quote_currency,
            inst.rebate,
        )?;
        // Exact at-hit rebate timing: compound the rebate forward from the
        // hit time at the domestic rate so DF(T) nets to DF(τ).
        {
            use crate::models::closed_form::barrier::RebateTiming;
            if inst.rebate.is_some() && inst.rebate_timing == RebateTiming::AtHit {
                payoff = payoff.with_rebate_at_hit(r_dom);
            }
        }

        // Derive deterministic seed from instrument ID and scenario

        use finstack_quant_monte_carlo::seed;

        let seed = if let Some(ref scenario) = inst.pricing_overrides.metrics.mc_seed_scenario {
            seed::derive_seed(&inst.id, scenario)
        } else {
            seed::derive_seed(&inst.id, "base")
        };

        let mut config = self.merged_path_config(inst);
        config.seed = seed;
        let pricer = PathDependentPricer::new(config);
        let result = pricer.price(
            &process,
            fx_spot,
            t,
            num_steps,
            &payoff,
            inst.quote_currency,
            discount_factor,
        )?;

        Ok(result.mean)
    }
}

impl Default for FxBarrierOptionMcPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for FxBarrierOptionMcPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::FxBarrierOption, ModelKey::MonteCarloGBM)
    }

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let fx_barrier = instrument
            .as_any()
            .downcast_ref::<FxBarrierOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::FxBarrierOption, instrument.key())
            })?;

        validate_monitoring_state(fx_barrier, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        let pv = self
            .price_internal(fx_barrier, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        Ok(ValuationResult::stamped(fx_barrier.id(), as_of, pv))
    }
}

/// Present value using Monte Carlo.
pub(crate) fn compute_pv(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    inst.validate()?;
    validate_monitoring_state(inst, as_of)?;
    if as_of > inst.expiry {
        return Ok(Money::new(0.0, inst.quote_currency));
    }
    let pricer = FxBarrierOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

fn validate_monitoring_state(
    inst: &FxBarrierOption,
    as_of: Date,
) -> finstack_quant_core::Result<()> {
    let start = inst.monitoring_start_date.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "FxBarrierOption requires monitoring_start_date".to_string(),
        )
    })?;
    if as_of > start && as_of <= inst.expiry && inst.observed_barrier_breached.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "Seasoned FX barrier option requires observed_barrier_breached after monitoring starts"
                .to_string(),
        ));
    }
    Ok(())
}

// ========================= ANALYTICAL PRICER =========================

use crate::models::closed_form::barrier::{
    barrier_call_continuous, barrier_put_continuous, barrier_rebate, BarrierParams,
    BarrierType as AnalyticalBarrierType,
};

#[inline]
fn barrier_is_knock_in(
    bt: crate::instruments::exotics::barrier_option::types::BarrierType,
) -> bool {
    matches!(
        bt,
        crate::instruments::exotics::barrier_option::types::BarrierType::UpAndIn
            | crate::instruments::exotics::barrier_option::types::BarrierType::DownAndIn
    )
}

fn expired_barrier_value_per_unit(
    inst: &FxBarrierOption,
    spot: f64,
) -> finstack_quant_core::Result<f64> {
    let strike = inst.strike;
    let is_knock_in = barrier_is_knock_in(inst.barrier_type);
    let barrier_hit = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Expired FX barrier option requires `observed_barrier_breached` to determine realized payoff"
                .to_string(),
        )
    })?;
    let activated = if is_knock_in {
        barrier_hit
    } else {
        !barrier_hit
    };

    let intrinsic = if activated {
        match inst.option_type {
            crate::instruments::OptionType::Call => (spot - strike).max(0.0),
            crate::instruments::OptionType::Put => (strike - spot).max(0.0),
        }
    } else {
        0.0
    };

    let rebate_due = if is_knock_in {
        !barrier_hit
    } else {
        barrier_hit
    };
    let rebate = if rebate_due {
        inst.rebate.unwrap_or(0.0)
    } else {
        0.0
    };

    Ok(intrinsic + rebate)
}

#[allow(clippy::too_many_arguments)]
fn seasoned_breached_value_per_unit(
    inst: &FxBarrierOption,
    spot: f64,
    r_dom: f64,
    r_for: f64,
    sigma: f64,
    t: f64,
    discount_factor: f64,
) -> f64 {
    if barrier_is_knock_in(inst.barrier_type) {
        crate::models::closed_form::vanilla::bs_price(
            spot,
            inst.strike,
            r_dom,
            r_for,
            sigma,
            t,
            inst.option_type,
        )
    } else {
        match inst.rebate_timing {
            crate::models::closed_form::barrier::RebateTiming::AtHit => 0.0,
            crate::models::closed_form::barrier::RebateTiming::AtExpiry => {
                inst.rebate.unwrap_or(0.0) * discount_factor
            }
        }
    }
}

/// Validate currency semantics and numeric bounds for FX barrier option.
///
/// # Currency Conventions
///
/// For an FX barrier option on `foreign_currency/domestic_currency` (e.g., EUR/USD):
/// - Strike and barrier are dimensionless exchange rates (f64)
/// - Notional is in foreign currency (base currency) - the amount of foreign currency
///   being bought/sold
fn validate_fx_barrier_currencies(inst: &FxBarrierOption) -> finstack_quant_core::Result<()> {
    inst.validate()?;

    let strike = inst.strike;
    if !strike.is_finite() || strike <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxBarrierOption strike must be finite and > 0, got {}",
            strike
        )));
    }
    let barrier = inst.barrier;
    if !barrier.is_finite() || barrier <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxBarrierOption barrier must be finite and > 0, got {}",
            barrier
        )));
    }
    let notional = inst.notional.amount();
    if !notional.is_finite() || notional <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxBarrierOption notional must be finite and > 0, got {}",
            notional
        )));
    }

    Ok(())
}

fn resolve_fx_spot(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    resolve_shared_fx_spot(FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        pricing_overrides: &inst.pricing_overrides,
        spot_source: FxSpotSource::ScalarId(inst.fx_spot_id.as_ref()),
        rate_context: "FxBarrierOption",
    })
}

fn collect_fx_barrier_expiry_state(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(f64, f64)> {
    validate_fx_barrier_currencies(inst)?;
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;
    let fx_spot = resolve_fx_spot(inst, curves, as_of)?;
    Ok((fx_spot, t))
}

/// Helper to collect inputs for FX barrier option pricing.
fn collect_fx_barrier_inputs(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<(f64, f64, f64, f64, f64)> {
    // Validate currency semantics first
    validate_fx_barrier_currencies(inst)?;

    let inputs = collect_fx_option_inputs(FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        pricing_overrides: &inst.pricing_overrides,
        spot_source: FxSpotSource::ScalarId(inst.fx_spot_id.as_ref()),
        rate_context: "FxBarrierOption",
    })?;
    let sigma = inputs.sigma;
    if !sigma.is_finite() || sigma < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxBarrierOption volatility must be finite and non-negative, got {}",
            sigma
        )));
    }

    let domestic_df = curves
        .get_discount(inst.domestic_discount_curve_id.as_str())?
        .df_between_dates(as_of, inst.expiry)?;

    Ok((
        inputs.spot,
        inputs.r_domestic,
        inputs.r_foreign,
        sigma,
        domestic_df,
    ))
}

/// FX Barrier option analytical pricer (continuous monitoring).
pub(crate) struct FxBarrierOptionAnalyticalPricer;

impl FxBarrierOptionAnalyticalPricer {
    /// Create a new analytical FX barrier option pricer
    pub(crate) fn new() -> Self {
        Self
    }
}

impl Default for FxBarrierOptionAnalyticalPricer {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the BS barrier price + optional rebate (without notional scaling).
fn bs_barrier_price_per_unit(
    fx_barrier: &FxBarrierOption,
    fx_spot: f64,
    r_dom: f64,
    r_for: f64,
    sigma: f64,
    t: f64,
    analytical_barrier_type: AnalyticalBarrierType,
) -> f64 {
    let params = BarrierParams::new(
        fx_spot,
        fx_barrier.strike,
        fx_barrier.barrier,
        t,
        r_dom,
        r_for,
        sigma,
    );
    let price = match fx_barrier.option_type {
        crate::instruments::OptionType::Call => {
            barrier_call_continuous(&params, analytical_barrier_type)
        }
        crate::instruments::OptionType::Put => {
            barrier_put_continuous(&params, analytical_barrier_type)
        }
    };

    let rebate_val = if let Some(rebate) = fx_barrier.rebate {
        barrier_rebate(
            &params,
            rebate,
            analytical_barrier_type,
            fx_barrier.rebate_timing,
        )
    } else {
        0.0
    };

    price + rebate_val
}

impl Pricer for FxBarrierOptionAnalyticalPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::FxBarrierOption,
            ModelKey::FxBarrierBSContinuous,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let fx_barrier = instrument
            .as_any()
            .downcast_ref::<FxBarrierOption>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::FxBarrierOption, instrument.key())
            })?;

        fx_barrier.validate().map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;
        validate_monitoring_state(fx_barrier, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        validate_monitoring_state(fx_barrier, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        if as_of > fx_barrier.expiry {
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::new(0.0, fx_barrier.quote_currency),
            ));
        }

        if fx_barrier.use_gobet_miri {
            return Err(PricingError::model_failure_with_context(
                "Discrete barrier monitoring (use_gobet_miri = true) requires the Monte Carlo \
                 pricer; the analytical Black-Scholes barrier pricer assumes continuous \
                 monitoring and would silently mis-price under discrete observation. \
                 Switch to ModelKey::FxBarrierMonteCarlo, or set use_gobet_miri = false to \
                 confirm continuous-monitoring pricing is intended."
                    .to_string(),
                PricingErrorContext::default(),
            ));
        }

        let (fx_spot, t) =
            collect_fx_barrier_expiry_state(fx_barrier, market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if t <= 0.0 {
            let per_unit = expired_barrier_value_per_unit(fx_barrier, fx_spot).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::new(
                    per_unit * fx_barrier.notional.amount(),
                    fx_barrier.quote_currency,
                ),
            ));
        }

        let (_, r_dom, r_for, sigma, discount_factor) =
            collect_fx_barrier_inputs(fx_barrier, market, as_of).map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        if fx_barrier.observed_barrier_breached == Some(true) {
            let per_unit = seasoned_breached_value_per_unit(
                fx_barrier,
                fx_spot,
                r_dom,
                r_for,
                sigma,
                t,
                discount_factor,
            );
            return Ok(ValuationResult::stamped(
                fx_barrier.id(),
                as_of,
                Money::new(
                    per_unit * fx_barrier.notional.amount(),
                    fx_barrier.quote_currency,
                ),
            ));
        }

        let analytical_barrier_type = fx_barrier.barrier_type;

        let price_per_unit = bs_barrier_price_per_unit(
            fx_barrier,
            fx_spot,
            r_dom,
            r_for,
            sigma,
            t,
            analytical_barrier_type,
        );

        let pv = Money::new(
            price_per_unit * fx_barrier.notional.amount(),
            fx_barrier.quote_currency,
        );
        Ok(ValuationResult::stamped(fx_barrier.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::barrier_option::types::BarrierType;
    use crate::instruments::Instrument;
    use crate::instruments::OptionType;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::market_data::surfaces::VolSurface;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    #[test]
    fn expired_up_and_in_call_returns_intrinsic_when_hit() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Call;
        inst.barrier_type = BarrierType::UpAndIn;
        inst.strike = 1.10;
        inst.barrier = 1.20;
        inst.rebate = None;
        inst.observed_barrier_breached = Some(true);

        let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
        assert!((per_unit - 0.15).abs() < 1e-12);
    }

    #[test]
    fn expired_down_and_out_put_returns_intrinsic_when_not_hit() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Put;
        inst.barrier_type = BarrierType::DownAndOut;
        inst.strike = 1.10;
        inst.barrier = 0.90;
        inst.rebate = None;
        inst.observed_barrier_breached = Some(false);

        // Barrier not hit at expiry => down-and-out stays active => intrinsic applies.
        let per_unit = expired_barrier_value_per_unit(&inst, 1.00).expect("expired value");
        assert!((per_unit - 0.10).abs() < 1e-12);
    }

    #[test]
    fn expired_up_and_out_with_hit_pays_rebate_only() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Call;
        inst.barrier_type = BarrierType::UpAndOut;
        inst.strike = 1.10;
        inst.barrier = 1.20;
        inst.rebate = Some(0.02);
        inst.observed_barrier_breached = Some(true);

        // Barrier hit at expiry => knocked out. With rebate, no intrinsic and rebate paid.
        let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
        assert!((per_unit - 0.02).abs() < 1e-12);
    }

    #[test]
    fn expired_up_and_in_with_no_hit_pays_rebate_only() {
        let mut inst = FxBarrierOption::example();
        inst.option_type = OptionType::Call;
        inst.barrier_type = BarrierType::UpAndIn;
        inst.strike = 1.10;
        inst.barrier = 1.20;
        inst.rebate = Some(0.02);
        inst.observed_barrier_breached = Some(false);

        let per_unit = expired_barrier_value_per_unit(&inst, 1.25).expect("expired value");
        assert!((per_unit - 0.02).abs() < 1e-12);
    }

    #[test]
    fn expired_fx_barrier_requires_observed_state() {
        let mut inst = FxBarrierOption::example();
        inst.observed_barrier_breached = None;

        let err = expired_barrier_value_per_unit(&inst, 1.25).expect_err("missing observed state");
        assert!(
            err.to_string().contains("observed_barrier_breached"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn validation_allows_barrier_equal_to_strike() {
        let mut inst = FxBarrierOption::example();
        inst.strike = 1.10;
        inst.barrier = 1.10;

        validate_fx_barrier_currencies(&inst).expect("equal strike/barrier should remain valid");
    }

    #[test]
    fn expired_analytical_value_only_requires_observed_state_and_spot() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let mut option = FxBarrierOption::example();
        option.expiry = as_of;
        option.use_gobet_miri = false;
        option.option_type = OptionType::Call;
        option.barrier_type = BarrierType::UpAndIn;
        option.rebate = Some(0.02);
        option.observed_barrier_breached = Some(false);

        let market = MarketContext::new().insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.25));

        let pv = option
            .value(&market, as_of)
            .expect("expired analytical value");
        assert!(
            (pv.amount() - 20_000.0).abs() < 1e-8,
            "expired FX barrier should settle from observed state and spot only, got {}",
            pv.amount()
        );
    }

    #[test]
    fn analytical_pricer_handles_zero_vol_knock_in_rebate_end_to_end() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let option = FxBarrierOption::builder()
            .id("FXBAR-ZERO-VOL-UPIN".into())
            .strike(1.10)
            .barrier(1.20)
            .rebate(0.02)
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndIn)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .use_gobet_miri(false)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .pricing_overrides(crate::instruments::PricingOverrides::default())
            .attributes(crate::instruments::Attributes::new())
            .build()
            .expect("fx barrier option");

        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("dom curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 1.0)])
                    .build()
                    .expect("for curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[0.25, 0.5, 1.0])
                    .strikes(&[1.0, 1.1, 1.2])
                    .row(&[0.0, 0.0, 0.0])
                    .row(&[0.0, 0.0, 0.0])
                    .row(&[0.0, 0.0, 0.0])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        let pv = option.value(&market, as_of).expect("fx barrier pv");
        assert!(
            (pv.amount() - 20_000.0).abs() < 1e-8,
            "zero-vol no-hit knock-in rebate should settle at rebate * notional, got {}",
            pv.amount()
        );
        assert_eq!(pv.currency(), Currency::USD);
    }

    #[test]
    fn validation_rejects_currency_mismatch_and_invalid_numeric_fields() {
        let mut mismatched = FxBarrierOption::example();
        mismatched.notional = Money::new(1_000_000.0, Currency::USD);
        let err = validate_fx_barrier_currencies(&mismatched).expect_err("currency mismatch");
        assert!(err.to_string().contains("Currency mismatch"));

        let mut bad_strike = FxBarrierOption::example();
        bad_strike.strike = 0.0;
        assert!(validate_fx_barrier_currencies(&bad_strike)
            .expect_err("bad strike")
            .to_string()
            .contains("strike"));

        let mut bad_barrier = FxBarrierOption::example();
        bad_barrier.barrier = f64::NAN;
        assert!(validate_fx_barrier_currencies(&bad_barrier)
            .expect_err("bad barrier")
            .to_string()
            .contains("barrier"));

        let mut bad_notional = FxBarrierOption::example();
        bad_notional.notional = Money::new(0.0, Currency::EUR);
        assert!(validate_fx_barrier_currencies(&bad_notional)
            .expect_err("bad notional")
            .to_string()
            .contains("notional"));
    }

    #[test]
    fn resolve_fx_spot_uses_fx_matrix_when_spot_id_is_absent() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut inst = FxBarrierOption::example();
        inst.fx_spot_id = None;

        let provider = Arc::new(SimpleFxProvider::new());
        provider
            .set_quote(Currency::EUR, Currency::USD, 1.15)
            .expect("valid quote");
        let market = MarketContext::new().insert_fx(FxMatrix::new(provider));

        let spot = resolve_fx_spot(&inst, &market, as_of).expect("fx matrix spot");
        assert!((spot - 1.15).abs() < 1e-12);
    }

    #[test]
    fn resolve_fx_spot_requires_valid_spot_source() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let mut no_matrix = FxBarrierOption::example();
        no_matrix.fx_spot_id = None;
        let err =
            resolve_fx_spot(&no_matrix, &MarketContext::new(), as_of).expect_err("missing matrix");
        assert!(err.to_string().contains("fx_matrix"));

        let mut price_scalar = FxBarrierOption::example();
        price_scalar.fx_spot_id = Some("EURUSD-SPOT".into());
        let price_market = MarketContext::new().insert_price(
            "EURUSD-SPOT",
            MarketScalar::Price(Money::new(1.10, Currency::USD)),
        );
        let spot = resolve_fx_spot(&price_scalar, &price_market, as_of).expect("price scalar spot");
        assert!((spot - 1.10).abs() < 1e-12);

        let bad_market =
            MarketContext::new().insert_price("EURUSD-SPOT", MarketScalar::Unitless(0.0));
        let err = resolve_fx_spot(&price_scalar, &bad_market, as_of).expect_err("bad scalar");
        assert!(err.to_string().contains("spot must be finite and > 0"));
    }

    /// Regression: MC pricer must honour option_type for puts.
    ///
    /// A deep-ITM down-and-out put (spot well below strike, barrier far below spot
    /// so the barrier is never hit) priced via the MC path (use_gobet_miri = true)
    /// must be close to the analytical price.  Under the bug the MC path hardcodes
    /// OptionKind::Call and returns max(S-K,0) ≈ 0 for this deep-ITM put, while
    /// the analytical path returns max(K-S,0) * df ≈ a large positive number.
    #[test]
    fn mc_barrier_put_honours_option_type() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        // Deep-ITM down-and-out put: spot=1.10, strike=1.30, barrier=0.80
        // spot is well below strike => put is deep ITM.
        // barrier=0.80 is far below spot=1.10 => very unlikely to knock out.
        // We use a moderate vol so the MC path stays active.
        let mc_option = FxBarrierOption::builder()
            .id("FXBAR-MC-PUT-BUG".into())
            .strike(1.30)
            .barrier(0.80)
            .rebate_opt(None)
            .option_type(OptionType::Put)
            .barrier_type(BarrierType::DownAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .use_gobet_miri(true) // force MC path
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .pricing_overrides(crate::instruments::PricingOverrides::default())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("mc put option");

        // Matching analytical option (use_gobet_miri=false for analytical pricer)
        let analytical_option = FxBarrierOption::builder()
            .id("FXBAR-ANAL-PUT-BUG".into())
            .strike(1.30)
            .barrier(0.80)
            .rebate_opt(None)
            .option_type(OptionType::Put)
            .barrier_type(BarrierType::DownAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .use_gobet_miri(false)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .pricing_overrides(crate::instruments::PricingOverrides::default())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("analytical put option");

        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.97)])
                    .build()
                    .expect("dom curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (1.0, 0.98)])
                    .build()
                    .expect("for curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[0.25, 0.5, 1.0])
                    .strikes(&[0.9, 1.1, 1.3])
                    .row(&[0.10, 0.10, 0.10])
                    .row(&[0.10, 0.10, 0.10])
                    .row(&[0.10, 0.10, 0.10])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        use crate::instruments::common_impl::traits::Instrument;

        let mc_pv = mc_option
            .value(&market, as_of)
            .expect("MC put price")
            .amount();

        let analytical_pv = analytical_option
            .value(&market, as_of)
            .expect("analytical put price")
            .amount();

        // Both must be positive (deep ITM put, barrier not hit)
        assert!(
            mc_pv > 0.0,
            "MC put price should be positive (deep ITM), got {}",
            mc_pv
        );
        assert!(
            analytical_pv > 0.0,
            "Analytical put price should be positive (deep ITM), got {}",
            analytical_pv
        );

        // MC and analytical must agree within 10% (MC tolerance for 100K paths)
        let rel_err = (mc_pv - analytical_pv).abs() / analytical_pv;
        assert!(
            rel_err < 0.10,
            "MC put price {} differs from analytical {} by {:.1}% (>10%), \
             option_type is likely being ignored in MC path",
            mc_pv,
            analytical_pv,
            rel_err * 100.0
        );
    }

    #[test]
    fn mc_inputs_pass_domestic_discount_factor_not_year_fraction() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");

        let option = FxBarrierOption::builder()
            .id("FXBAR-MC-DF".into())
            .strike(1.10)
            .barrier(1.30)
            .rebate_opt(None)
            .option_type(OptionType::Call)
            .barrier_type(BarrierType::UpAndOut)
            .monitoring_start_date(as_of)
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_quant_core::dates::DayCount::Act365F)
            .use_gobet_miri(true)
            .domestic_discount_curve_id("USD-OIS".into())
            .foreign_discount_curve_id("EUR-OIS".into())
            .fx_spot_id_opt(Some("EURUSD-SPOT".into()))
            .vol_surface_id("EURUSD-VOL".into())
            .pricing_overrides(crate::instruments::PricingOverrides::default())
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("fx barrier option");

        let domestic_df_at_two_years = (-0.03_f64 * 2.0).exp();
        let market = MarketContext::new()
            .insert(
                DiscountCurve::builder("USD-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, domestic_df_at_two_years)])
                    .build()
                    .expect("dom curve"),
            )
            .insert(
                DiscountCurve::builder("EUR-OIS")
                    .base_date(as_of)
                    .knots([(0.0, 1.0), (2.0, (-0.01_f64 * 2.0).exp())])
                    .build()
                    .expect("for curve"),
            )
            .insert_surface(
                VolSurface::builder("EURUSD-VOL")
                    .expiries(&[2.0])
                    .strikes(&[1.10])
                    .row(&[0.12])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        let expected_domestic_df = market
            .get_discount("USD-OIS")
            .expect("dom curve")
            .df_between_dates(as_of, expiry)
            .expect("domestic df");
        let (_, _, _, _, discount_factor) =
            collect_fx_barrier_inputs(&option, &market, as_of).expect("inputs");

        assert!(
            (discount_factor - expected_domestic_df).abs() < 1e-12,
            "MC discount factor must be domestic DF {expected_domestic_df}, got {discount_factor}"
        );
    }
}
