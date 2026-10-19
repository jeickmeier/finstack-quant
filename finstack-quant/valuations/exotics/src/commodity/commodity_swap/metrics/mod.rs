//! Commodity swap metrics module.
//!
//! Provides metric calculators specific to `CommoditySwap`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_commodity_swap_metrics`.
//!
//! Exposed metrics:
//! - commodity delta (sensitivity to floating index)
//! - theta (time decay)
//! - DV01 (interest rate sensitivity)

use crate::commodity::commodity_swap::CommoditySwap;
use finstack_quant_core::market_data::bumps::BumpMode;
use finstack_quant_core::market_data::bumps::BumpSpec;
use finstack_quant_core::market_data::bumps::BumpType;
use finstack_quant_core::market_data::bumps::BumpUnits;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::types::CurveId;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CommoditySwap metrics with the registry.
pub(crate) fn register_commodity_swap_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommoditySwap,
        metrics: [
            (Delta, DeltaCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Delta calculator for commodity swaps (per 1.0 unit of floating price).
///
/// Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting,
/// consistent with the NPV calculation in `CommoditySwap::floating_leg_pv()`.
///
/// Delta measures the sensitivity of the swap's NPV to changes in the floating price.
/// For each payment period:
///
/// Delta contribution = sign × Q × DF(as_of → payment_date)
///
/// Uses `df_between_dates(as_of, payment_date)` for base-date-safe discounting.
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let swap: &CommoditySwap = context.instrument_as()?;
        let curve_id = CurveId::new(swap.forward_curve_id.as_str());
        let bump = |value| MarketBump::Curve {
            id: curve_id.clone(),
            spec: BumpSpec {
                bump_type: BumpType::Parallel,
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value,
            },
        };
        let up = context.curves.bump([bump(1.0)])?;
        let down = context.curves.bump([bump(-1.0)])?;
        let pv_up = swap.value(&up, context.as_of)?.amount();
        let pv_down = swap.value(&down, context.as_of)?.amount();
        Ok((pv_up - pv_down) / 2.0)
    }
}
