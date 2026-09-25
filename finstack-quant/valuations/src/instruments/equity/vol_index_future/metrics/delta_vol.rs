//! Delta to underlying volatility index calculator for volatility index futures.
//!
//! Computes the exposure to the underlying volatility index level.
//!
//! # Market Standard Formula
//!
//! **DeltaVol:** Δ_vol = sign × contracts × multiplier
//!
//! Where:
//! - contracts and multiplier come from `terms`
//! - sign = +1 for long, -1 for short
//!
//! # Note
//!
//! This metric represents the dollar exposure to a one-point move in the
//! underlying volatility index. For VIX futures with a $1000 multiplier,
//! a 1-point move in VIX results in a $1000 P&L per contract.

use crate::instruments::equity::vol_index_future::VolatilityIndexFuture;

/// Delta to volatility index calculator for volatility index futures.
pub(crate) struct DeltaVolCalculator;

impl crate::metrics::MetricCalculator for DeltaVolCalculator {
    fn calculate(
        &self,
        context: &mut crate::metrics::MetricContext,
    ) -> finstack_quant_core::Result<f64> {
        let future: &VolatilityIndexFuture = context.instrument_as()?;
        future.delta_vol()
    }
}
