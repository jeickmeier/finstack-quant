//! Commodity forward metrics module.
//!
//! Provides metric calculators specific to `CommodityForward`, split into focused files.
//! The calculators compose with the shared metrics framework and are registered
//! via `register_commodity_forward_metrics`.
//!
//! Exposed metrics:
//! - commodity delta (sensitivity to forward price)
//! - theta (time decay)
//! - DV01 (interest rate sensitivity)

use crate::commodity::commodity_forward::CommodityForward;
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all CommodityForward metrics with the registry.
pub(crate) fn register_commodity_forward_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::CommodityForward,
        metrics: [
            (Delta, DeltaCalculator),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
    Ok(())
}

/// Delta calculator for commodity forwards (per 1.0 unit of forward price).
///
/// Returns the change in NPV for a 1.0 unit increase in the forward price,
/// accounting for position direction (long = positive delta, short = negative delta).
///
/// Uses `df_between_dates(as_of, settlement)` for base-date-safe discounting.
///
/// Delta measures the sensitivity of the forward's NPV to changes in the forward price.
/// For a commodity forward with NPV = sign(position) × (F - K) × Q × M × DF:
///
/// Delta = ∂NPV/∂F = sign(position) × Q × M × DF
///
/// Where:
/// - sign(position) = +1 for long, -1 for short
/// - Q = quantity
/// - M = multiplier
/// - DF = discount factor from as_of to settlement
pub(crate) struct DeltaCalculator;

impl MetricCalculator for DeltaCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let fwd: &CommodityForward = context.instrument_as()?;

        // Expired forward has zero delta
        if fwd.maturity < context.as_of {
            return Ok(0.0);
        }

        let disc = context
            .curves
            .get_discount(fwd.discount_curve_id.as_str())?;

        // Use df_between_dates for base-date-safe discounting (consistent with npv())
        let df = disc.df_between_dates(context.as_of, fwd.maturity)?;

        // Delta = sign(position) × Q × M × DF
        Ok(fwd.position.sign() * fwd.quantity * fwd.multiplier * df)
    }
}
