//! FX barrier option pricers (Monte Carlo and analytical).

mod analytical;
mod mc;
#[cfg(test)]
mod tests;

use crate::fx_barrier_option::types::FxBarrierOption;
use crate::shared::{FxOptionInputRequest, FxOptionInputs, collect_fx_option_inputs_at_spot};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::PayoutTiming;
use finstack_quant_valuations_core::instruments::Monitoring;
use finstack_quant_valuations_core::pricer::{ModelKey, PricingErrorContext};

pub(crate) use analytical::FxBarrierOptionAnalyticalPricer;
pub use mc::FxBarrierOptionMcPricer;
pub(crate) use mc::compute_pv;

pub(super) fn barrier_pricing_context(
    inst: &FxBarrierOption,
    model: ModelKey,
) -> PricingErrorContext {
    let mut context = PricingErrorContext::from_instrument(inst)
        .model(model)
        .curve_ids([
            inst.domestic_discount_curve_id.as_str(),
            inst.foreign_discount_curve_id.as_str(),
            inst.vol_surface_id.as_str(),
        ]);
    if let Some(spot_id) = &inst.fx_spot_id {
        context = context.curve_id(spot_id.as_str());
    }
    context
}

pub(super) fn validate_monitoring_state(
    inst: &FxBarrierOption,
    as_of: Date,
) -> finstack_quant_core::Result<()> {
    let has_past_monitoring = match &inst.monitoring {
        Monitoring::Continuous => {
            let start = inst.monitoring_start_date.ok_or_else(|| {
                finstack_quant_core::Error::Validation(
                    "FxBarrierOption requires monitoring_start_date".to_string(),
                )
            })?;
            as_of > start
        }
        Monitoring::Discrete { observation_dates } => {
            observation_dates.iter().any(|date| *date < as_of)
        }
    };
    if has_past_monitoring && as_of <= inst.expiry && inst.observed_barrier_breached.is_none() {
        return Err(finstack_quant_core::Error::Validation(
            "Seasoned FX barrier option requires observed_barrier_breached after monitoring starts"
                .to_string(),
        ));
    }
    Ok(())
}

pub(super) fn expired_barrier_value_per_unit(
    inst: &FxBarrierOption,
    spot: f64,
) -> finstack_quant_core::Result<f64> {
    let strike = inst.strike;
    let barrier_hit = inst.observed_barrier_breached.ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Expired FX barrier option requires `observed_barrier_breached` to determine realized payoff"
                .to_string(),
        )
    })?;
    let intrinsic = match inst.option_type {
        finstack_quant_models::types::OptionType::Call => (spot - strike).max(0.0),
        finstack_quant_models::types::OptionType::Put => (strike - spot).max(0.0),
    };
    // A recorded knock-out hit includes settlement of an at-hit rebate; the
    // same claim is extinguished before, on, and after expiry.
    Ok(finstack_quant_models::closed_form::expired_barrier_payoff(
        inst.barrier_type,
        barrier_hit,
        intrinsic,
        inst.rebate_terms_per_unit(),
    ))
}

fn seasoned_breached_value_per_unit(inst: &FxBarrierOption, inputs: &FxOptionInputs) -> f64 {
    if inst.barrier_type.is_knock_in() {
        finstack_quant_models::closed_form::bs_price_unchecked(
            inputs.spot,
            inst.strike,
            inputs.r_domestic,
            inputs.r_foreign,
            inputs.sigma,
            inputs.t,
            inst.option_type,
        )
    } else {
        match inst.rebate_terms_per_unit() {
            Some((rebate, PayoutTiming::AtExpiry)) => rebate * inputs.df_domestic,
            Some((_, PayoutTiming::AtHit)) | None => 0.0,
        }
    }
}

/// FX spot from the `fx_spot_id` scalar when set, else the FX matrix.
pub(crate) fn resolve_fx_spot(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    crate::shared::resolve_fx_spot(
        curves,
        as_of,
        inst.base_currency,
        inst.quote_currency,
        inst.fx_spot_id.as_ref(),
        "FxBarrierOption",
    )
}

fn input_request<'a>(
    inst: &'a FxBarrierOption,
    curves: &'a MarketContext,
    as_of: Date,
) -> FxOptionInputRequest<'a> {
    FxOptionInputRequest {
        market: curves,
        as_of,
        base_currency: inst.base_currency,
        quote_currency: inst.quote_currency,
        expiry: inst.expiry,
        delivery: inst.expiry,
        spot_basis: crate::shared::FxSpotBasis::Cash,
        day_count: inst.day_count,
        domestic_discount_curve_id: &inst.domestic_discount_curve_id,
        foreign_discount_curve_id: &inst.foreign_discount_curve_id,
        vol_surface_id: inst.vol_surface_id.as_str(),
        strike: inst.strike,
        instrument_pricing_overrides: &inst.instrument_pricing_overrides,
        spot_id: inst.fx_spot_id.as_ref(),
        rate_context: "FxBarrierOption",
    }
}

/// Live FX barrier inputs at an already resolved `spot`.
pub(super) fn collect_fx_barrier_inputs(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
    spot: f64,
) -> finstack_quant_core::Result<FxOptionInputs> {
    let inputs = collect_fx_option_inputs_at_spot(input_request(inst, curves, as_of), spot)?;
    if !inputs.sigma.is_finite() || inputs.sigma < 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "FxBarrierOption volatility must be finite and non-negative, got {}",
            inputs.sigma
        )));
    }
    Ok(inputs)
}

/// FX barrier state at a valuation date on or before expiry.
pub(super) enum BarrierState {
    /// Value per unit of base notional fixed by the observed barrier state.
    Fixed(f64),
    /// Live market inputs for a model price.
    Live(FxOptionInputs),
}

/// Resolve spot once, then settle expired and breached contracts or return
/// the live inputs both pricers price from.
pub(super) fn barrier_state(
    inst: &FxBarrierOption,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<BarrierState> {
    let t = inst
        .day_count
        .year_fraction(as_of, inst.expiry, DayCountContext::default())?;
    let spot = resolve_fx_spot(inst, curves, as_of)?;
    if t <= 0.0 {
        return Ok(BarrierState::Fixed(expired_barrier_value_per_unit(
            inst, spot,
        )?));
    }
    let inputs = collect_fx_barrier_inputs(inst, curves, as_of, spot)?;
    if inst.observed_barrier_breached == Some(true) {
        return Ok(BarrierState::Fixed(seasoned_breached_value_per_unit(
            inst, &inputs,
        )));
    }
    Ok(BarrierState::Live(inputs))
}

/// Scale a per-unit value by the base notional into quote currency.
pub(super) fn notional_value(
    inst: &FxBarrierOption,
    per_unit: f64,
) -> finstack_quant_core::Result<Money> {
    Money::from_model_value(per_unit * inst.notional.amount(), inst.quote_currency)
}
