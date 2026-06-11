//! Weighted Average Spread calculator for CLO

use crate::metrics::MetricContext;

/// CLO WAS calculator - in basis points
///
/// Market standard: WAS uses the **spread component only**, not the all-in
/// coupon, over **performing** assets only. Fixed-rate assets without an
/// explicit `spread_bps` are excluded (no all-in-rate fallback), as are
/// defaulted assets — see [`AssetPool::weighted_avg_spread`].
///
/// [`AssetPool::weighted_avg_spread`]: crate::instruments::fixed_income::structured_credit::types::AssetPool::weighted_avg_spread
pub struct CloWasCalculator;

impl crate::metrics::MetricCalculator for CloWasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let clo = context
            .instrument
            .as_any()
            .downcast_ref::<crate::instruments::fixed_income::structured_credit::StructuredCredit>()
            .ok_or(finstack_core::InputError::Invalid)?;

        Ok(clo.pool.weighted_avg_spread())
    }
}
