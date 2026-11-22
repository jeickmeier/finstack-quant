//! Shared bump sizing, validation, CRN seeding, and scratch evaluation helpers
//! for generic finite-difference Greek calculators.

use crate::instruments::Instrument;
use crate::instruments::MarketDependencies;
use crate::metrics::MetricContext;
use crate::metrics::finite_difference::{
    active_minimum_vol, apply_parallel_surface_bumps_in_place, bumped_volatility_override_dyn,
    revert_scratch_bumps, scalar_numeric_value, volatility_override,
};
use finstack_quant_core::Result;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::types::CurveId;

/// Minimum absolute bump size for spot-based finite differences.
///
/// This floor prevents division by zero or numerically unstable greeks when
/// the underlying spot price is very small. The denominator in finite differences
/// scales with this bump size, so an excessively small bump leads to explosive greeks.
///
/// # Value Rationale
///
/// The value 1e-8 is chosen as a balance:
/// - **Equity prices** (typically $1-$10,000): 1% bump on a $0.01 stock = $0.0001,
///   well above 1e-8, so the floor rarely activates for normal equities.
/// - **FX rates** (typically 0.5-200): 1% bump on a 0.01 rate = 1e-4, still safe.
/// - **Fractional shares or near-zero prices**: The floor activates to prevent
///   instability when spot × bump_pct would be dangerously small.
///
/// # Limitations
///
/// This is a conservative fixed floor. For instruments with atypical price scales
/// (e.g., cryptocurrencies with 8+ decimal places, or very large notionals), users
/// may need to adjust bump percentages in `FinstackConfig` rather than relying on
/// this floor.
pub const MIN_ABSOLUTE_BUMP: f64 = 1e-8;

/// Common random number seed scenario for MC greek calculations.
///
/// Using the same seed for all bump scenarios (up/down/base) ensures that
/// Monte Carlo noise cancels in finite differences, providing stable greeks.
/// This is the standard "common random numbers" (CRN) variance reduction technique.
pub(super) const CRN_SEED_SCENARIO: &str = "greeks_crn";

/// Validate that spot price is positive and finite.
///
/// Returns an error if the spot is non-positive, NaN, or infinite.
pub(super) fn validate_spot(spot: f64, greek_name: &str) -> Result<()> {
    if !spot.is_finite() || spot <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Non-positive or invalid spot price ({}) for {} calculation. \
             Spot must be positive and finite.",
            spot, greek_name
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(super) struct SpotBump {
    pub(super) relative: f64,
    pub(super) absolute: f64,
}

/// Compute a spot bump whose applied relative shock matches the absolute denominator.
///
/// Returns the configured relative bump unless its absolute spot move is smaller
/// than [`MIN_ABSOLUTE_BUMP`]. In that case the relative bump is increased so
/// the market scenario and finite-difference denominator use the same move.
#[inline]
pub(super) fn effective_spot_bump(spot: f64, bump_pct: f64) -> SpotBump {
    let signed_relative = bump_pct.signum() * (spot * bump_pct).abs().max(MIN_ABSOLUTE_BUMP) / spot;
    SpotBump {
        relative: signed_relative,
        absolute: (spot * signed_relative).abs(),
    }
}

/// Resolve the active grids when the instrument has no scalar override.
pub(super) fn bumped_vol_surface_ids(
    instrument: &dyn Instrument,
    dependencies: &MarketDependencies,
    market: &MarketContext,
    metric_name: &str,
) -> Result<Vec<CurveId>> {
    if volatility_override(instrument).is_some() {
        return Ok(Vec::new());
    }
    if dependencies.volatility_dependencies.is_empty() {
        return Err(finstack_quant_core::Error::Validation(format!(
            "Instrument missing volatility dependencies for {metric_name} calculation"
        )));
    }

    dependencies.grid_vol_surface_ids(market)
}

// First market scalar of the instrument: the spot the spot-bumped greeks move.
pub(super) fn spot_id<'a>(
    dependencies: &'a MarketDependencies,
    greek_name: &str,
) -> Result<&'a String> {
    dependencies.market_scalar_ids.first().ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!(
            "Instrument missing spot_id for {greek_name} calculation"
        ))
    })
}

// Validated level of `spot_id` in `market`.
pub(super) fn current_spot(market: &MarketContext, spot_id: &str, greek_name: &str) -> Result<f64> {
    let spot = scalar_numeric_value(market.get_price(spot_id)?);
    validate_spot(spot, greek_name)?;
    Ok(spot)
}

// Whether the option has expired at `as_of`; its FD higher-order greeks are then zero.
pub(super) fn is_expired(instrument: &dyn Instrument, as_of: Date) -> Result<bool> {
    let expiry = instrument.expiry().ok_or_else(|| {
        finstack_quant_core::Error::Validation(
            "Instrument missing expiry for finite-difference option Greeks".into(),
        )
    })?;
    Ok(as_of >= expiry)
}

// Keep central higher-order differences inside the positive-volatility domain.
pub(super) fn symmetric_volatility_step(
    instrument: &dyn Instrument,
    market: &MarketContext,
    volatility_surface_ids: &[CurveId],
    requested: f64,
) -> Result<f64> {
    let minimum = active_minimum_vol(instrument, market, volatility_surface_ids)?;
    match minimum {
        Some(volatility) if volatility.is_finite() && volatility > 0.0 => {
            Ok(requested.min(volatility * 0.5))
        }
        _ => Err(finstack_quant_core::Error::Validation(
            "Central volatility Greeks require positive active volatility".into(),
        )),
    }
}

/// Guard against NaN / ±Inf leaking out of finite-difference calculations.
pub(super) fn ensure_finite(value: f64, metric_name: &str) -> finstack_quant_core::Result<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(finstack_quant_core::Error::Validation(format!(
            "{metric_name} produced non-finite result: {value}"
        )))
    }
}

pub(super) fn clone_with_crn_seed(instrument: &dyn Instrument) -> Result<Box<dyn Instrument>> {
    let mut seeded = instrument.clone_box();
    let overrides = seeded
        .get_instrument_pricing_overrides_mut()
        .ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "Instrument {} does not expose instrument_pricing_overrides.model_config.mc_seed_scenario required for finite-difference common-random-number seeding",
                instrument.id()
            ))
        })?;
    overrides.model_config.mc_seed_scenario = Some(CRN_SEED_SCENARIO.to_string());
    Ok(seeded)
}

/// Clone of `instrument` seeded with [`CRN_SEED_SCENARIO`] so base and bumped
/// Monte Carlo reprices share random draws; any caller-set seed label is
/// replaced. Instruments without pricing overrides cannot be Monte Carlo
/// priced and are returned unchanged.
///
/// # Arguments
///
/// * `instrument` - Instrument cloned into a boxed trait object and seeded when
///   pricing overrides are exposed.
#[doc(hidden)]
pub fn clone_with_crn_seed_if_supported(instrument: &dyn Instrument) -> Box<dyn Instrument> {
    let mut seeded = instrument.clone_box();
    if let Some(overrides) = seeded.get_instrument_pricing_overrides_mut() {
        overrides.model_config.mc_seed_scenario = Some(CRN_SEED_SCENARIO.to_string());
    }
    seeded
}

pub(super) fn eval_raw_with_scratch_bumps(
    context: &MetricContext,
    scratch: &mut MarketContext,
    instrument: &dyn Instrument,
    as_of: Date,
    spot_bump: Option<(&str, f64)>,
    vol_bump: Option<(&[CurveId], f64)>,
) -> Result<f64> {
    let bumped_instrument = match vol_bump {
        Some((_, bump_abs)) => bumped_volatility_override_dyn(instrument, bump_abs, false)?,
        None => None,
    };
    let price_token = if let Some((spot_id, bump_pct)) = spot_bump {
        Some(scratch.apply_price_bump_fraction_in_place(spot_id, bump_pct)?)
    } else {
        None
    };

    let surface_tokens = match vol_bump.filter(|_| bumped_instrument.is_none()) {
        Some((vol_surface_ids, bump_abs)) => {
            match apply_parallel_surface_bumps_in_place(scratch, vol_surface_ids, bump_abs) {
                Ok(tokens) => Some(tokens),
                Err(err) => {
                    if let Some(token) = price_token {
                        scratch.revert_scratch_bump(token)?;
                    }
                    return Err(err);
                }
            }
        }
        None => None,
    };

    let value = context.reprice_instrument_raw(
        bumped_instrument.as_deref().unwrap_or(instrument),
        scratch,
        as_of,
    );
    if let Some(tokens) = surface_tokens {
        revert_scratch_bumps(scratch, tokens)?;
    }
    if let Some(token) = price_token {
        scratch.revert_scratch_bump(token)?;
    }
    value
}
