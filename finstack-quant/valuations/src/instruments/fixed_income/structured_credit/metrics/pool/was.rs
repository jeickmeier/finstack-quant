//! Weighted Average Spread calculator for CLO

use crate::metrics::MetricContext;

/// CLO WAS calculator - in basis points
///
/// Market standard: WAS uses the **spread component only**, not the all-in
/// coupon, over **performing** assets only. Fixed-rate assets without an
/// explicit `spread_bp` are excluded (no all-in-rate fallback), as are
/// defaulted assets — see [`AssetPool::weighted_avg_spread_bp`].
///
/// [`AssetPool::weighted_avg_spread_bp`]: crate::instruments::fixed_income::structured_credit::types::AssetPool::weighted_avg_spread_bp
pub struct CloWasCalculator;

impl crate::metrics::MetricCalculator for CloWasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context
            .instrument_as::<crate::instruments::fixed_income::structured_credit::StructuredCredit>(
            )?;

        Ok(clo.pool.weighted_avg_spread_bp())
    }
}
