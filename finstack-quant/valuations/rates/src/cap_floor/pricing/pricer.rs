//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::cap_floor::pricing::projection::{unpaid_caplets, UnpaidCaplet};
use crate::cap_floor::pricing::CapletFloorletInputs;
use crate::cap_floor::{CapFloor, CapFloorVolType};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError,
};
use finstack_quant_valuations_core::results::ValuationResult;

/// Minimum time-to-fixing for vol surface lookup (in years).
///
/// When a caplet is at or past its fixing date (`t_fix <= 0`), the vol surface lookup
/// still requires a positive time input. This constant provides a small floor (~31.5 seconds)
/// to avoid numerical issues while still returning a near-expiry volatility.
///
/// The choice of 1e-6 years is small enough to not materially affect the volatility lookup
/// but large enough to avoid potential division-by-zero or log(0) issues in vol surface
/// interpolation. For seasoned caplets, the Black formula will use intrinsic value anyway,
/// so the exact vol returned is not critical.
const MIN_VOL_LOOKUP_TIME: f64 = 1e-6;

/// Resolve the quote convention once for pricing, inversion and analytic risk.
pub(crate) fn resolve_caplet_volatility(
    cap_floor: &CapFloor,
    curves: &MarketContext,
    expiry: f64,
    strike: f64,
) -> finstack_quant_core::Result<
    finstack_quant_valuations_core::instruments::common_impl::vol_resolution::ResolvedVolatility,
> {
    use finstack_quant_models::volatility::VolatilityConvention;
    use finstack_quant_valuations_core::instruments::common_impl::vol_resolution::{
        resolve_volatility, VolatilityRequest,
    };
    let convention = match cap_floor.vol_type {
        CapFloorVolType::Auto => None,
        CapFloorVolType::Normal => Some(VolatilityConvention::Normal),
        CapFloorVolType::Lognormal => Some(VolatilityConvention::Lognormal),
        CapFloorVolType::ShiftedLognormal => Some(VolatilityConvention::ShiftedLognormal {
            shift: cap_floor.resolved_vol_shift(),
        }),
    };
    resolve_volatility(
        &cap_floor.instrument_pricing_overrides.market_quotes,
        curves,
        cap_floor.vol_surface_id.as_str(),
        VolatilityRequest {
            expiry,
            tenor: 0.0,
            strike,
            convention,
            clamp: true,
        },
    )
}

/// Price in the resolved quote convention without conversion or rate fallback.
pub(crate) fn price_caplet_quote(
    inputs: CapletFloorletInputs,
    quote: finstack_quant_valuations_core::instruments::common_impl::vol_resolution::ResolvedVolatility,
) -> finstack_quant_core::Result<Money> {
    let ccy = inputs.currency;
    let pv = price_caplet_quote_raw(inputs, quote)?;
    Money::new(pv, ccy)
}

pub(crate) fn price_caplet_quote_raw(
    inputs: CapletFloorletInputs,
    quote: finstack_quant_valuations_core::instruments::common_impl::vol_resolution::ResolvedVolatility,
) -> finstack_quant_core::Result<f64> {
    use crate::cap_floor::pricing::{black, normal};
    use finstack_quant_models::volatility::VolatilityConvention;
    let (forward, strike) = quote.model_rates(inputs.forward, inputs.strike)?;
    let inputs = CapletFloorletInputs {
        forward,
        strike,
        sigma: quote.sigma,
        ..inputs
    };
    match quote.convention {
        VolatilityConvention::Normal => normal::price_caplet_floorlet_raw(inputs),
        _ => black::price_caplet_floorlet_raw(inputs),
    }
}

#[cfg(test)]
pub(crate) fn price_cap_floor(
    cap_floor: &CapFloor,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    let pv = price_cap_floor_raw(cap_floor, curves, as_of)?;
    Money::new(pv, cap_floor.notional.currency())
}

pub(crate) fn price_cap_floor_raw(
    cap_floor: &CapFloor,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    use finstack_quant_models::volatility::VolatilityConvention;
    use finstack_quant_valuations_core::instruments::common_impl::vol_resolution::ResolvedVolatility;

    cap_floor.validate_for_pricing()?;

    let mut total_pv = 0.0;
    let is_cap = cap_floor.rate_option_type.is_cap();
    for caplet in unpaid_caplets(cap_floor, curves, as_of)? {
        let UnpaidCaplet {
            inputs: resolved_inputs,
            forward,
            strike,
        } = caplet?;
        let projection = &resolved_inputs.coupon;

        let fixing_date = projection.fixing_date;
        // No option time remains on or after the contractual fixing date.
        // Earlier fixings are observed; under the explicit start-of-day policy,
        // a same-day unpublished fixing is projected but priced intrinsically.
        let has_no_option_time = fixing_date <= as_of;
        let effective_t_fix = if has_no_option_time {
            0.0
        } else {
            resolved_inputs.time_to_fixing.max(MIN_VOL_LOOKUP_TIME)
        };

        let df = resolved_inputs.discount_factor;
        let quote = if effective_t_fix > 0.0 {
            resolve_caplet_volatility(cap_floor, curves, effective_t_fix, strike)?
        } else {
            ResolvedVolatility {
                sigma: 0.0,
                convention: VolatilityConvention::Normal,
            }
        };
        let tau = projection.accrual_year_fraction;

        let inputs = CapletFloorletInputs {
            is_cap,
            notional: cap_floor.notional.amount(),
            strike,
            forward,
            discount_factor: df,
            sigma: quote.sigma,
            time_to_fixing: effective_t_fix,
            accrual_year_fraction: tau,
            currency: cap_floor.notional.currency(),
        };
        let leg_pv = price_caplet_quote_raw(inputs, quote)?;
        total_pv += leg_pv;
    }

    Ok(total_pv)
}

/// Black76 cap/floor pricer.
pub(crate) struct SimpleCapFloorBlackPricer;

impl Pricer for SimpleCapFloorBlackPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CapFloor, ModelKey::Black76)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        // Type-safe downcasting
        let cap_floor = finstack_quant_valuations_core::pricer::expect_inst::<CapFloor>(
            instrument,
            InstrumentType::CapFloor,
        )?;

        // Unshocked holder NPV, net of any unsettled premium; the registry
        // applies the pricing lifecycle (scenario overrides) on top.
        let pv = cap_floor.base_value(market, as_of)?;

        Ok(ValuationResult::stamped(cap_floor.id(), as_of, pv))
    }

    fn price_raw_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<f64, PricingError> {
        let cap_floor = finstack_quant_valuations_core::pricer::expect_inst::<CapFloor>(
            instrument,
            InstrumentType::CapFloor,
        )?;
        Ok(cap_floor.base_value_raw(market, as_of)?)
    }
}
