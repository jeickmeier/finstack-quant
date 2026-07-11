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

/// Delta calculator for one spread leg's forward price.
struct SpreadDeltaCalculator {
    leg: u8,
}

impl MetricCalculator for SpreadDeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &CommoditySpreadOption = context.instrument_as()?;

        let bump_pct = crate::metrics::bump_sizes::SPOT; // 1% = 0.01

        let (curve_id, forward) = match self.leg {
            1 => (
                CurveId::new(inst.leg1_forward_curve_id.as_str()),
                inst.leg1_forward(&context.curves)?,
            ),
            2 => (
                CurveId::new(inst.leg2_forward_curve_id.as_str()),
                inst.leg2_forward(&context.curves)?,
            ),
            _ => {
                return Err(finstack_quant_core::Error::Validation(
                    "invalid spread leg".into(),
                ))
            }
        };
        let bump_size = forward * bump_pct;
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
struct SpreadVegaCalculator {
    leg: Option<u8>,
}

impl MetricCalculator for SpreadVegaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let inst: &CommoditySpreadOption = context.instrument_as()?;

        let vol_bump = crate::metrics::bump_sizes::VOLATILITY; // 1 vol point = 0.01

        let bump_market = |amount: f64| -> Result<_> {
            match self.leg {
                Some(1) => crate::metrics::bump_surface_vol_absolute(
                    &context.curves,
                    inst.leg1_vol_surface_id.as_str(),
                    amount,
                ),
                Some(2) => crate::metrics::bump_surface_vol_absolute(
                    &context.curves,
                    inst.leg2_vol_surface_id.as_str(),
                    amount,
                ),
                None => {
                    let first = crate::metrics::bump_surface_vol_absolute(
                        &context.curves,
                        inst.leg1_vol_surface_id.as_str(),
                        amount,
                    )?;
                    crate::metrics::bump_surface_vol_absolute(
                        &first,
                        inst.leg2_vol_surface_id.as_str(),
                        amount,
                    )
                }
                Some(_) => Err(finstack_quant_core::Error::Validation(
                    "invalid spread-option vega leg".into(),
                )),
            }
        };
        let pv_up = inst.value(&bump_market(vol_bump)?, context.as_of)?.amount();
        let pv_dn = inst
            .value(&bump_market(-vol_bump)?, context.as_of)?
            .amount();

        // Report cash P&L per one-vol-point move. The scenarios are already
        // shifted by +/- `vol_bump`, so do not normalize back to dPV/dsigma.
        Ok((pv_up - pv_dn) / 2.0)
    }
}

/// Register commodity spread option metrics with the registry.
pub(crate) fn register_commodity_spread_option_metrics(registry: &mut MetricRegistry) {
    registry.register_metric(
        MetricId::Delta,
        Arc::new(SpreadDeltaCalculator { leg: 1 }),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::Vega,
        Arc::new(SpreadVegaCalculator { leg: None }),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::custom("delta::leg1"),
        Arc::new(SpreadDeltaCalculator { leg: 1 }),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::custom("delta::leg2"),
        Arc::new(SpreadDeltaCalculator { leg: 2 }),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::custom("vega::leg1"),
        Arc::new(SpreadVegaCalculator { leg: Some(1) }),
        &[InstrumentType::CommoditySpreadOption],
    );
    registry.register_metric(
        MetricId::custom("vega::leg2"),
        Arc::new(SpreadVegaCalculator { leg: Some(2) }),
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
