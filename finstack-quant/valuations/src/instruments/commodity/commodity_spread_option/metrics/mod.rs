//! Commodity spread option metrics module.
//!
//! Provides risk sensitivities for commodity spread options:
//! - **Delta (leg 1)**: Sensitivity to leg 1 forward price (bump-and-reprice on PriceCurve)
//! - **Vega**: Volatility sensitivity (bump-and-reprice on vol surfaces)
//! - **DV01**: Interest rate sensitivity (discount curve bump)
//! - **Theta**: Time decay

use crate::instruments::commodity::commodity_spread_option::CommoditySpreadOption;
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{MetricCalculator, MetricContext, MetricId, MetricRegistry};
use crate::pricer::InstrumentType;
use finstack_quant_core::market_data::bumps::{
    BumpMode, BumpSpec, BumpType, BumpUnits, MarketBump,
};
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use std::sync::Arc;

/// Delta calculator for leg 1 forward price.
///
/// Computes dPV/dF1 by bumping the leg 1 price curve up and down.
struct SpreadDeltaLeg1Calculator;

impl MetricCalculator for SpreadDeltaLeg1Calculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &CommoditySpreadOption = context.instrument_as()?;

        let bump_pct = crate::metrics::bump_sizes::SPOT; // 1% = 0.01

        let curve_id = CurveId::new(inst.leg1_forward_curve_id.as_str());
        let f1 = inst.leg1_forward(&context.curves)?;
        let bump_size = f1 * bump_pct;
        if bump_size <= 0.0 {
            return Ok(0.0);
        }

        let market_up = context.curves.bump([MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: bump_pct * 100.0,
            },
        }])?;
        let pv_up = inst.value(&market_up, context.as_of)?.amount();

        let market_down = context.curves.bump([MarketBump::Curve {
            id: curve_id,
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: -bump_pct * 100.0,
            },
        }])?;
        let pv_down = inst.value(&market_down, context.as_of)?.amount();

        Ok((pv_up - pv_down) / (2.0 * bump_size))
    }
}

/// Vega calculator: combined sensitivity to both vol surfaces.
///
/// Bumps both leg 1 and leg 2 vol surfaces simultaneously by 1 vol point.
struct SpreadVegaCalculator;

impl MetricCalculator for SpreadVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &CommoditySpreadOption = context.instrument_as()?;

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY; // 1 vol point = 0.01

        // Bump both vol surfaces up
        let up1 = crate::metrics::bump_surface_vol_absolute(
            &context.curves,
            inst.leg1_vol_surface_id.as_str(),
            vol_bump,
        )?;
        let up_both = crate::metrics::bump_surface_vol_absolute(
            &up1,
            inst.leg2_vol_surface_id.as_str(),
            vol_bump,
        )?;
        let pv_up = inst.value(&up_both, context.as_of)?.amount();

        // Bump both vol surfaces down
        let dn1 = crate::metrics::bump_surface_vol_absolute(
            &context.curves,
            inst.leg1_vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let dn_both = crate::metrics::bump_surface_vol_absolute(
            &dn1,
            inst.leg2_vol_surface_id.as_str(),
            -vol_bump,
        )?;
        let pv_dn = inst.value(&dn_both, context.as_of)?.amount();

        Ok((pv_up - pv_dn) / (2.0 * vol_bump))
    }
}

/// Register commodity spread option metrics with the registry.
pub(crate) fn register_commodity_spread_option_metrics(registry: &mut MetricRegistry) {
    registry.register_metric(
        MetricId::Delta,
        Arc::new(SpreadDeltaLeg1Calculator),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::Vega,
        Arc::new(SpreadVegaCalculator),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::Dv01,
        Arc::new(
            crate::metrics::UnifiedDv01Calculator::<CommoditySpreadOption>::new(
                crate::metrics::Dv01CalculatorConfig::parallel_combined(),
            ),
        ),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::BucketedDv01,
        Arc::new(
            crate::metrics::UnifiedDv01Calculator::<CommoditySpreadOption>::new(
                crate::metrics::Dv01CalculatorConfig::triangular_key_rate(),
            ),
        ),
        &[InstrumentType::CommoditySpreadOption],
    );
}
