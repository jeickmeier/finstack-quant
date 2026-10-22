//! Commodity spread option metrics module.
//!
//! Provides risk sensitivities for commodity spread options:
//! - **Delta (leg 1)**: Sensitivity to leg 1 forward price (bump-and-reprice on PriceCurve)
//! - **Vega**: Volatility sensitivity (bump-and-reprice on both leg vol surfaces)
//! - **DV01**: Interest rate sensitivity (discount curve bump)
//! - **Theta**: Time decay

use crate::commodity::bump_price_curve_pct;
use crate::commodity::commodity_spread_option::CommoditySpreadOption;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{
    MetricCalculator, MetricContext, MetricRegistry, VOL_POINTS_PER_ABSOLUTE_VOL,
};
use finstack_quant_valuations_core::pricer::InstrumentType;

/// Delta calculator for one spread leg's forward price.
struct SpreadDeltaCalculator {
    leg: u8,
}

impl MetricCalculator for SpreadDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &CommoditySpreadOption = context.instrument_as()?;

        let bump_pct = sens_config::resolve(context)?.spot_bump_decimal;

        let curve_id = match self.leg {
            1 => &inst.leg1_forward_curve_id,
            2 => &inst.leg2_forward_curve_id,
            _ => {
                return Err(finstack_quant_core::Error::Validation(
                    "invalid spread leg".into(),
                ))
            }
        };
        let forward = inst.leg_forward(&context.curves, curve_id)?;
        let bump_size = forward * bump_pct;
        if bump_size <= 0.0 {
            return Ok(0.0);
        }

        let market_up = bump_price_curve_pct(&context.curves, curve_id.as_str(), bump_pct)?;
        let pv_up = inst.value(&market_up, context.as_of)?.amount();

        let market_down = bump_price_curve_pct(&context.curves, curve_id.as_str(), -bump_pct)?;
        let pv_down = inst.value(&market_down, context.as_of)?.amount();

        Ok((pv_up - pv_down) / (2.0 * bump_size))
    }
}

/// Vega calculator for one spread leg's volatility surface, or both.
///
/// Bumps the leg surface (`Some(leg)`) or each distinct leg surface once
/// (`None`, the headline `Vega`) by the resolved vol bump (default 1 vol
/// point) and reports the result per vol point. The Kirk pricer reads the
/// surfaces directly, so the surfaces are bumped even when an implied-vol
/// override is set (the core `GenericFdVega` would bump that unused quote).
struct SpreadVegaCalculator {
    leg: Option<u8>,
}

impl MetricCalculator for SpreadVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &CommoditySpreadOption = context.instrument_as()?;

        let vol_bump = sens_config::resolve(context)?.vol_bump_decimal;

        let bump = finstack_quant_valuations_core::metrics::bump_surface_vol_absolute;
        let bump_market = |amount: f64| match self.leg {
            Some(1) => bump(&context.curves, inst.leg1_vol_surface_id.as_str(), amount),
            Some(2) => bump(&context.curves, inst.leg2_vol_surface_id.as_str(), amount),
            None => {
                let first = bump(&context.curves, inst.leg1_vol_surface_id.as_str(), amount)?;
                if inst.leg1_vol_surface_id == inst.leg2_vol_surface_id {
                    Ok(first)
                } else {
                    bump(&first, inst.leg2_vol_surface_id.as_str(), amount)
                }
            }
            Some(_) => Err(finstack_quant_core::Error::Validation(
                "invalid spread-option vega leg".into(),
            )),
        };
        let pv_up = inst.value(&bump_market(vol_bump)?, context.as_of)?.amount();
        let pv_dn = inst
            .value(&bump_market(-vol_bump)?, context.as_of)?
            .amount();

        // Report cash P&L per one-vol-point move: normalise by the bump width
        // expressed in vol points (2.0 at the 1 vol-point default).
        Ok((pv_up - pv_dn) / (2.0 * vol_bump * VOL_POINTS_PER_ABSOLUTE_VOL))
    }
}

/// Register commodity spread option metrics with the registry.
pub(crate) fn register_commodity_spread_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySpreadOption,
        metrics: [
            (Delta, SpreadDeltaCalculator { leg: 1 }),
            (Vega, SpreadVegaCalculator { leg: None }),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
        ],
        custom: [
            ("delta::leg1", SpreadDeltaCalculator { leg: 1 }),
            ("delta::leg2", SpreadDeltaCalculator { leg: 2 }),
            ("vega::leg1", SpreadVegaCalculator { leg: Some(1) }),
            ("vega::leg2", SpreadVegaCalculator { leg: Some(2) }),
        ]
    }
    Ok(())
}
