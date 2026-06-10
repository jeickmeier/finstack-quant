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
use finstack_core::dates::{Date, DayCountContext};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;

// MC-specific imports
use crate::instruments::fx::fx_barrier_option::monte_carlo::FxBarrierPayoff;
use finstack_monte_carlo::payoff::barrier::BarrierType as McBarrierType;
use finstack_monte_carlo::payoff::barrier::OptionKind as McOptionKind;
use finstack_monte_carlo::pricer::path_dependent::{
    PathDependentPricer, PathDependentPricerConfig,
};
use finstack_monte_carlo::process::gbm::{GbmParams, GbmProcess};

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
    ) -> finstack_core::Result<finstack_core::money::Money> {
        validate_fx_barrier_currencies(inst)?;

        let (fx_spot, t) = collect_fx_barrier_expiry_state(inst, curves, as_of)?;
        if t <= 0.0 {
            let per_unit = expired_barrier_value_per_unit(inst, fx_spot)?;
            return Ok(finstack_core::money::Money::new(
                per_unit * inst.notional.amount(),
                inst.quote_currency,
            ));
        }

        let (_, r_dom, r_for, sigma, discount_factor) =
            collect_fx_barrier_inputs(inst, curves, as_of)?;

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
        warn_mc_at_hit_rebate_approximation(inst);
        let mc_barrier_type: McBarrierType = inst.barrier_type.into();
        let mc_option_kind = match inst.option_type {
            crate::instruments::OptionType::Call => McOptionKind::Call,
            crate::instruments::OptionType::Put => McOptionKind::Put,
        };
        let payoff = FxBarrierPayoff::new(
            inst.strike,
            inst.barrier,
            mc_barrier_type,
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

        // Derive deterministic seed from instrument ID and scenario

        use finstack_monte_carlo::seed;

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
) -> finstack_core::Result<Money> {
    let pricer = FxBarrierOptionMcPricer::new();
    pricer.price_internal(inst, curves, as_of)
}

// ========================= ANALYTICAL PRICER =========================

use crate::models::closed_form::barrier::{
    barrier_call_continuous, barrier_put_continuous, barrier_rebate, BarrierParams,
    BarrierType as AnalyticalBarrierType,
};

/// The MC path pays rebates at expiry; warn when an at-hit KO rebate is
/// requested so the timing approximation is visible in production logs.
fn warn_mc_at_hit_rebate_approximation(inst: &FxBarrierOption) {
    use crate::models::closed_form::barrier::RebateTiming;
    if inst.rebate.is_some()
        && inst.rebate_timing == RebateTiming::AtHit
        && !barrier_is_knock_in(inst.barrier_type)
    {
        tracing::warn!(
            instrument_id = %inst.id,
            "FX barrier MC pricer approximates the at-hit knock-out rebate as paid at expiry; \
             use the analytical pricer for exact at-hit rebate discounting"
        );
    }
}

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

fn expired_barrier_value_per_unit(inst: &FxBarrierOption, spot: f64) -> finstack_core::Result<f64> {
    let strike = inst.strike;
    let is_knock_in = barrier_is_knock_in(inst.barrier_type);
    let barrier_hit = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_core::Error::Validation(
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

/// Validate currency semantics and numeric bounds for FX barrier option.
///
/// # Currency Conventions
///
/// For an FX barrier option on `foreign_currency/domestic_currency` (e.g., EUR/USD):
/// - Strike and barrier are dimensionless exchange rates (f64)
/// - Notional is in foreign currency (base currency) - the amount of foreign currency
///   being bought/sold
fn validate_fx_barrier_currencies(inst: &FxBarrierOption) -> finstack_core::Result<()> {
    inst.validate()?;

    let strike = inst.strike;
    if !strike.is_finite() || strike <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "FxBarrierOption strike must be finite and > 0, got {}",
            strike
        )));
    }
    let barrier = inst.barrier;
    if !barrier.is_finite() || barrier <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
            "FxBarrierOption barrier must be finite and > 0, got {}",
            barrier
        )));
    }
    let notional = inst.notional.amount();
    if !notional.is_finite() || notional <= 0.0 {
        return Err(finstack_core::Error::Validation(format!(
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
) -> finstack_core::Result<f64> {
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
) -> finstack_core::Result<(f64, f64)> {
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
) -> finstack_core::Result<(f64, f64, f64, f64, f64)> {
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
        return Err(finstack_core::Error::Validation(format!(
            "FxBarrierOption volatility must be finite and non-negative, got {}",
            sigma
        )));
    }

    Ok((
        inputs.spot,
        inputs.r_domestic,
        inputs.r_foreign,
        sigma,
        inputs.t,
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

/// Map from the instrument's BarrierType to the analytical BarrierType.
fn map_barrier_type(
    bt: crate::instruments::exotics::barrier_option::types::BarrierType,
) -> AnalyticalBarrierType {
    use crate::instruments::exotics::barrier_option::types::BarrierType;
    match bt {
        BarrierType::UpAndIn => AnalyticalBarrierType::UpIn,
        BarrierType::UpAndOut => AnalyticalBarrierType::UpOut,
        BarrierType::DownAndIn => AnalyticalBarrierType::DownIn,
        BarrierType::DownAndOut => AnalyticalBarrierType::DownOut,
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

        let (_, r_dom, r_for, sigma, _) = collect_fx_barrier_inputs(fx_barrier, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let analytical_barrier_type = map_barrier_type(fx_barrier.barrier_type);

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

// ========================= VANNA-VOLGA PRICER =========================
//
// Gated behind the `fx-vanna-volga` feature: the smile-correction pricer is
// not registered in the production registry until per-trade smile quotes are
// part of the market-data contract.

#[cfg(feature = "fx-vanna-volga")]
use crate::instruments::fx::fx_barrier_option::vanna_volga::{
    vanna_volga_barrier_adjustment, VannaVolgaQuotes,
};

/// Internal FX barrier option Vanna-Volga pricer.
///
/// Applies the Vanna-Volga method (Castagna & Mercurio 2007) to adjust the
/// analytical BS barrier price for smile effects, using three market pillar
/// volatilities (25Δ put, ATM, 25Δ call).
///
/// This pricer is intentionally not registered in the standard registry until
/// the instrument or market-data contract carries per-trade smile quotes.
/// Standard dispatch should use [`ModelKey::FxBarrierBSContinuous`] or
/// [`ModelKey::MonteCarloGBM`].
#[cfg(feature = "fx-vanna-volga")]
pub(crate) struct FxBarrierOptionVannaVolgaPricer {
    /// Market quotes for the three-point smile. When `None`, a symmetric
    /// fallback smile is derived from the instrument's ATM vol at pricing
    /// time (see [`FxBarrierOptionVannaVolgaPricer::resolve_quotes`]).
    pub(crate) quotes: Option<VannaVolgaQuotes>,
}

#[cfg(feature = "fx-vanna-volga")]
impl FxBarrierOptionVannaVolgaPricer {
    /// Create a Vanna-Volga pricer without bound smile quotes.
    ///
    /// Calls fail until explicit market quotes are supplied with
    /// [`Self::with_quotes`].
    pub(crate) fn new() -> Self {
        Self { quotes: None }
    }

    /// Create a Vanna-Volga pricer bound to explicit market quotes.
    #[allow(dead_code)]
    pub(crate) fn with_quotes(quotes: VannaVolgaQuotes) -> Self {
        Self {
            quotes: Some(quotes),
        }
    }

    /// Resolve the three-pillar smile quotes for a given instrument.
    ///
    /// Precedence:
    ///
    /// 1. Pricer-bound `self.quotes` (set via `with_quotes`).
    /// 2. Otherwise: returns `Err` — there is no statistically meaningful
    ///    "default" Vanna-Volga smile, and silently collapsing to the BS
    ///    path would produce a price that ignores the smile model the
    ///    caller explicitly requested via `ModelKey::FxBarrierVannaVolga`.
    ///    Production callers must construct the pricer with real 25Δ
    ///    market quotes via `with_quotes(...)`.
    fn resolve_quotes(
        &self,
        fx_barrier: &FxBarrierOption,
    ) -> std::result::Result<VannaVolgaQuotes, PricingError> {
        if let Some(q) = self.quotes {
            return Ok(q);
        }
        Err(PricingError::model_failure_with_context(
            format!(
                "Vanna-Volga pricing requires three-pillar smile quotes (25Δ put, ATM, 25Δ call) \
                 but none are bound on the pricer. Construct the pricer with `with_quotes(...)`, \
                 or use ModelKey::FxBarrierBSContinuous if no smile correction is desired. \
                 Instrument id: {}",
                fx_barrier.id()
            ),
            PricingErrorContext::default(),
        ))
    }

    /// Price an FX barrier option with Vanna-Volga smile adjustment.
    #[allow(dead_code)]
    pub(crate) fn price_with_vv(
        &self,
        fx_barrier: &FxBarrierOption,
        market: &MarketContext,
        as_of: Date,
    ) -> std::result::Result<Money, PricingError> {
        if fx_barrier.use_gobet_miri {
            return Err(PricingError::model_failure_with_context(
                "Discrete barrier monitoring (use_gobet_miri = true) requires the Monte Carlo \
                 pricer; the Vanna-Volga analytical pricer assumes continuous monitoring and \
                 would silently mis-price under discrete observation."
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
            return Ok(Money::new(
                per_unit * fx_barrier.notional.amount(),
                fx_barrier.quote_currency,
            ));
        }

        let (_, r_dom, r_for, sigma, _) = collect_fx_barrier_inputs(fx_barrier, market, as_of)
            .map_err(|e| {
                PricingError::model_failure_with_context(
                    e.to_string(),
                    PricingErrorContext::default(),
                )
            })?;

        let analytical_barrier_type = map_barrier_type(fx_barrier.barrier_type);
        let is_call = matches!(fx_barrier.option_type, crate::instruments::OptionType::Call);

        // Compute BS barrier price at ATM vol
        let bs_price = bs_barrier_price_per_unit(
            fx_barrier,
            fx_spot,
            r_dom,
            r_for,
            sigma,
            t,
            analytical_barrier_type,
        );

        // Apply Vanna-Volga correction with resolved smile quotes — fails loudly
        // when no quotes are bound, rather than silently collapsing to BS.
        let quotes = self.resolve_quotes(fx_barrier)?;
        let vv_params = BarrierParams::new(
            fx_spot,
            fx_barrier.strike,
            fx_barrier.barrier,
            t,
            r_dom,
            r_for,
            sigma,
        );
        let vv_price = vanna_volga_barrier_adjustment(
            bs_price,
            &vv_params,
            &quotes,
            is_call,
            analytical_barrier_type,
        );

        Ok(Money::new(
            vv_price * fx_barrier.notional.amount(),
            fx_barrier.quote_currency,
        ))
    }
}

#[cfg(feature = "fx-vanna-volga")]
impl Default for FxBarrierOptionVannaVolgaPricer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "fx-vanna-volga")]
impl Pricer for FxBarrierOptionVannaVolgaPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::FxBarrierOption,
            ModelKey::FxBarrierVannaVolga,
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

        let pv = self.price_with_vv(fx_barrier, market, as_of)?;
        Ok(ValuationResult::stamped(fx_barrier.id(), as_of, pv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::exotics::barrier_option::types::BarrierType;
    use crate::instruments::Instrument;
    use crate::instruments::OptionType;
    use finstack_core::currency::Currency;
    use finstack_core::dates::Date;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::market_data::scalars::MarketScalar;
    use finstack_core::market_data::surfaces::VolSurface;
    use finstack_core::market_data::term_structures::DiscountCurve;
    use finstack_core::money::fx::{FxMatrix, SimpleFxProvider};
    use finstack_core::money::Money;
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
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_core::dates::DayCount::Act365F)
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

    /// With the symmetric-smile fallback (default-constructed pricer,
    /// no explicit market quotes), the Vanna-Volga adjustment must
    /// collapse to zero so the VV price matches the BS analytical
    /// price. This keeps the pricer registration safe for instruments
    /// that haven't yet had smile data wired in.
    #[cfg(feature = "fx-vanna-volga")]
    #[test]
    fn vanna_volga_errors_without_explicit_quotes() {
        let as_of = Date::from_calendar_date(2024, Month::January, 1).expect("valid date");
        let expiry = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");

        let option = FxBarrierOption::builder()
            .id("FXBAR-VV-SMOKE".into())
            .strike(1.10)
            .barrier(1.20)
            .rebate(0.0)
            .option_type(crate::instruments::OptionType::Call)
            .barrier_type(crate::instruments::exotics::barrier_option::types::BarrierType::UpAndOut)
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_core::dates::DayCount::Act365F)
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
                    .strikes(&[1.0, 1.1, 1.2])
                    .row(&[0.12, 0.12, 0.12])
                    .row(&[0.12, 0.12, 0.12])
                    .row(&[0.12, 0.12, 0.12])
                    .build()
                    .expect("vol surface"),
            )
            .insert_price("EURUSD-SPOT", MarketScalar::Unitless(1.10));

        let bs = FxBarrierOptionAnalyticalPricer::new();
        let vv = FxBarrierOptionVannaVolgaPricer::new();

        // BS still prices fine — no smile required.
        let _bs_pv = bs
            .price_dyn(&option, &market, as_of)
            .expect("bs price")
            .value;

        // VV without bound smile quotes must fail loudly rather than silently
        // collapsing to BS, which would ignore the smile model the caller
        // explicitly requested via ModelKey::FxBarrierVannaVolga.
        let err = vv
            .price_dyn(&option, &market, as_of)
            .expect_err("VV must error without bound smile quotes");
        let msg = format!("{err}");
        assert!(
            msg.contains("Vanna-Volga") && msg.contains("smile quotes"),
            "expected smile-quote error, got: {msg}"
        );
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
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_core::dates::DayCount::Act365F)
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
            .expiry(expiry)
            .notional(Money::new(1_000_000.0, Currency::EUR))
            .base_currency(Currency::EUR)
            .quote_currency(Currency::USD)
            .day_count(finstack_core::dates::DayCount::Act365F)
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
}
