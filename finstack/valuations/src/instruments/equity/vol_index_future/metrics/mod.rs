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

mod delta_vol;

use crate::metrics::{MetricId, MetricRegistry};

/// Register all VolatilityIndexFuture metrics with the registry.
pub(crate) fn register_vol_index_future_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Register custom DeltaVol metric (not a standard MetricId)
    registry.register_metric(
        MetricId::DeltaVol,
        Arc::new(delta_vol::DeltaVolCalculator),
        &[InstrumentType::VolatilityIndexFuture],
    );
}
