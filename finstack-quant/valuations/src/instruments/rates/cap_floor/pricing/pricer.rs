//! Pricing and metric helpers for interest-rate instruments.
//!
use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::rates::cap_floor::pricing::payoff::CapletFloorletInputs;
use crate::instruments::rates::cap_floor::pricing::projection::resolve_optioned_caplet_inputs;
use crate::instruments::rates::cap_floor::{CapFloor, CapFloorVolType, RateOptionType};
use crate::pricer::{
    InstrumentType, ModelKey, Pricer, PricerKey, PricingError, PricingErrorContext,
};
use crate::results::ValuationResult;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;

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
) -> finstack_quant_core::Result<crate::instruments::common_impl::vol_resolution::ResolvedVolatility>
{
    use crate::instruments::common_impl::vol_resolution::{resolve_volatility, VolatilityRequest};
    use finstack_quant_models::volatility::VolatilityConvention;
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
    quote: crate::instruments::common_impl::vol_resolution::ResolvedVolatility,
) -> finstack_quant_core::Result<Money> {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};
    use finstack_quant_models::volatility::VolatilityConvention;
    let (forward, strike) = quote.model_rates(inputs.forward, inputs.strike)?;
    let inputs = CapletFloorletInputs {
        forward,
        strike,
        volatility: quote.sigma,
        ..inputs
    };
    match quote.convention {
        VolatilityConvention::Normal => normal::price_caplet_floorlet(inputs),
        _ => black::price_caplet_floorlet(inputs),
    }
}

pub(crate) fn price_cap_floor(
    cap_floor: &CapFloor,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    use crate::instruments::common_impl::vol_resolution::ResolvedVolatility;
    use finstack_quant_models::volatility::VolatilityConvention;

    cap_floor.validate_for_pricing()?;

    let strike = cap_floor.strike_f64()?;

    let mut total_pv = Money::from((0_i64, cap_floor.notional.currency()));
    let periods = cap_floor.pricing_periods()?;
    if periods.is_empty() {
        return Ok(total_pv);
    }

    let is_cap = matches!(
        cap_floor.rate_option_type,
        RateOptionType::Caplet | RateOptionType::Cap
    );
    for period in periods {
        if period.payment_date <= as_of {
            continue;
        }
        let resolved_inputs = resolve_optioned_caplet_inputs(cap_floor, &period, curves, as_of)?;
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

        let forward = projection.forward;
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
            volatility: quote.sigma,
            time_to_fixing: effective_t_fix,
            accrual_year_fraction: tau,
            currency: cap_floor.notional.currency(),
        };
        let leg_pv = price_caplet_quote(inputs, quote)?;
        total_pv = total_pv.checked_add(leg_pv)?;
    }

    Ok(total_pv)
}

/// New simplified Cap/Floor pricer supporting multiple models.
pub(crate) struct SimpleCapFloorBlackPricer {
    model: ModelKey,
}

impl SimpleCapFloorBlackPricer {
    /// Create a new cap/floor Black pricer with default model
    pub(crate) fn new() -> Self {
        Self {
            model: ModelKey::Black76,
        }
    }
}

impl Default for SimpleCapFloorBlackPricer {
    fn default() -> Self {
        Self::new()
    }
}

impl Pricer for SimpleCapFloorBlackPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::CapFloor, self.model)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        // Type-safe downcasting
        let cap_floor =
            crate::pricer::expect_inst::<CapFloor>(instrument, InstrumentType::CapFloor)?;

        let pv = price_cap_floor(cap_floor, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        Ok(ValuationResult::stamped(cap_floor.id(), as_of, pv))
    }
}
