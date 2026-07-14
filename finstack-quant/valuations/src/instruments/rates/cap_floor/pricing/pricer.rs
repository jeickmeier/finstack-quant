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

/// Resolve the effective vol type.
///
/// `Auto` is treated as a **lognormal** surface and resolves to `Lognormal`. The
/// `Lognormal` pricing arm prices each caplet with Black-76 where the model is
/// well-defined (`forward > 0` and `strike > 0`) and otherwise converts the
/// lognormal vol to an equivalent normal vol and uses Bachelier. This keeps a
/// single, consistent interpretation of the supplied surface across every
/// caplet — including a cap whose schedule crosses zero — rather than feeding the
/// same surface number to two incompatible models. Explicit model selections
/// remain explicit.
fn resolve_vol_type(vol_type: CapFloorVolType) -> CapFloorVolType {
    match vol_type {
        CapFloorVolType::Auto => CapFloorVolType::Lognormal,
        other => other,
    }
}

/// Price a lognormal quote with the cap/floor pricer's negative-rate fallback.
///
/// The quote remains lognormal. Where Black-76 is outside its positive
/// forward/strike domain, the quote is converted to an equivalent normal
/// volatility before Bachelier pricing. `Auto` pricing and implied-vol inversion
/// share this function so inversion returns the original quote convention.
pub(crate) fn price_lognormal_quote_with_fallback(
    inputs: CapletFloorletInputs,
) -> finstack_quant_core::Result<Money> {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};

    if inputs.forward > 0.0 && inputs.strike > 0.0 {
        black::price_caplet_floorlet(inputs)
    } else {
        let normal_vol = crate::instruments::rates::swaption::types::lognormal_to_normal_vol(
            inputs.volatility,
            inputs.forward,
            inputs.strike,
            inputs.time_to_fixing,
            None,
        );
        normal::price_caplet_floorlet(CapletFloorletInputs {
            volatility: normal_vol,
            ..inputs
        })
    }
}

pub(crate) fn price_cap_floor(
    cap_floor: &CapFloor,
    curves: &MarketContext,
    as_of: Date,
) -> finstack_quant_core::Result<Money> {
    use crate::instruments::rates::cap_floor::pricing::{black, normal};

    cap_floor.validate_for_pricing()?;

    let strike = cap_floor.strike_f64()?;

    let mut total_pv = Money::new(0.0, cap_floor.notional.currency());
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
        let sigma = if effective_t_fix > 0.0 {
            crate::instruments::common_impl::vol_resolution::resolve_sigma_at(
                &cap_floor.pricing_overrides.market_quotes,
                curves,
                cap_floor.vol_surface_id.as_str(),
                effective_t_fix,
                strike,
            )?
        } else {
            0.0
        };
        let tau = projection.accrual_year_fraction;

        let inputs = || CapletFloorletInputs {
            is_cap,
            notional: cap_floor.notional.amount(),
            strike,
            forward,
            discount_factor: df,
            volatility: sigma,
            time_to_fixing: effective_t_fix,
            accrual_year_fraction: tau,
            currency: cap_floor.notional.currency(),
        };
        let vol_shift = cap_floor.resolved_vol_shift();
        let resolved = resolve_vol_type(cap_floor.vol_type);
        let leg_pv = match resolved {
            CapFloorVolType::Lognormal => price_lognormal_quote_with_fallback(inputs())?,
            CapFloorVolType::ShiftedLognormal => {
                // Shifted-lognormal Black-76 requires the SHIFTED forward and
                // strike to be strictly positive — that is the whole point of
                // the shift in a negative-rate regime. If `vol_shift` is too
                // small to lift this caplet's forward (the most-negative
                // forward across the schedule fails first), `(F + shift)`
                // would be non-positive and Black-76 would produce a
                // log-of-non-positive NaN. Validate explicitly with an
                // actionable error rather than emitting garbage.
                let shifted_forward = forward + vol_shift;
                let shifted_strike = strike + vol_shift;
                if shifted_forward <= 0.0 || shifted_strike <= 0.0 {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "cap/floor ShiftedLognormal: vol_shift {vol_shift:.6} does not lift \
                         the caplet forward/strike positive (shifted forward {shifted_forward:.6}, \
                         shifted strike {shifted_strike:.6}, fixing {fixing_date}). \
                         Increase vol_shift so F + shift > 0 for the most-negative caplet, \
                         or price with the Normal model."
                    )));
                }
                black::price_caplet_floorlet(CapletFloorletInputs {
                    strike: shifted_strike,
                    forward: shifted_forward,
                    ..inputs()
                })?
            }
            CapFloorVolType::Normal => normal::price_caplet_floorlet(inputs())?,
            CapFloorVolType::Auto => {
                return Err(finstack_quant_core::Error::Validation(
                    "internal error: cap/floor vol_type resolved to Auto".to_string(),
                ));
            }
        };
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
        let cap_floor = instrument
            .as_any()
            .downcast_ref::<CapFloor>()
            .ok_or_else(|| {
                PricingError::type_mismatch(InstrumentType::CapFloor, instrument.key())
            })?;

        let pv = price_cap_floor(cap_floor, market, as_of).map_err(|e| {
            PricingError::model_failure_with_context(e.to_string(), PricingErrorContext::default())
        })?;

        // Return stamped result
        Ok(ValuationResult::stamped(cap_floor.id(), as_of, pv))
    }
}
