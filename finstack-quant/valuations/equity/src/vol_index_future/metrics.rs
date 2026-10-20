//! Volatility index future metrics module.
//!
//! Provides metric calculators specific to `VolatilityIndexFuture`, following
//! the shared metrics framework pattern.
//!
//! Exposed metrics:
//! - DeltaVol (exposure to underlying volatility index level)
//!
//! No Dv01 is registered: vol-index futures are daily-margined, so the
//! mark-to-market PV is undiscounted and carries no direct discount-curve
//! sensitivity.

use crate::vol_index_future::VolatilityIndexFuture;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register all VolatilityIndexFuture metrics with the registry.
pub(crate) fn register_vol_index_future_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::pricer::InstrumentType;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::VolatilityIndexFuture,
        metrics: [
            // Register custom DeltaVol metric (not a standard MetricId)
            (DeltaVol, DeltaVolCalculator),
        ]
    }
    Ok(())
}

/// Delta to volatility index calculator for volatility index futures.
///
/// Computes the exposure to the underlying volatility index level.
///
/// # Market Standard Formula
///
/// **DeltaVol:** Δ_vol = sign × contracts × multiplier
///
/// Where:
/// - contracts and multiplier come from `terms`
/// - sign = +1 for long, -1 for short
///
/// # Note
///
/// This metric represents the dollar exposure to a one-point move in the
/// underlying volatility index. For VIX futures with a $1000 multiplier,
/// a 1-point move in VIX results in a $1000 P&L per contract.
pub(crate) struct DeltaVolCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for DeltaVolCalculator {
    fn calculate(
        &self,
        context: &mut finstack_quant_valuations_core::metrics::MetricContext,
    ) -> finstack_quant_core::Result<f64> {
        let future: &VolatilityIndexFuture = context.instrument_as()?;
        future.delta_vol()
    }
}
