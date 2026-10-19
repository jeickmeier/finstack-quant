//! AssetPool characteristic metrics for structured credit.

pub(crate) mod characteristics;

pub use characteristics::{CdrCalculator, CprCalculator, WamCalculator};
use finstack_quant_core::types::CreditRating;
use finstack_quant_models::credit::moodys_warf_factor;
use finstack_quant_valuations_core::metrics::MetricContext;

/// CLO WARF calculator - Moody's methodology
pub struct CloWarfCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for CloWarfCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context.instrument_as::<crate::structured_credit::StructuredCredit>()?;

        let mut weighted_sum = 0.0;
        let mut total_balance = 0.0;

        // Performing par only: defaulted assets are carried at recovery in
        // OC haircuts, not in the rating-factor average.
        for asset in clo.pool.assets.iter().filter(|a| !a.defaulted) {
            let balance = asset.balance.amount();
            // Assets with no rating use the registry's `NR` (not rated)
            // factor, so the unrated-collateral policy lives in one place
            // (the embedded Moody's table) instead of a hardcoded constant.
            let rating_factor = moodys_warf_factor(asset.rating.unwrap_or(CreditRating::NR))?;

            weighted_sum += balance * rating_factor;
            total_balance += balance;
        }

        if total_balance > 0.0 {
            Ok(weighted_sum / total_balance)
        } else {
            Ok(0.0)
        }
    }
}

/// CLO WAS calculator - in basis points
///
/// Market standard: WAS uses the **spread component only**, not the all-in
/// coupon, over **performing** assets only. Fixed-rate assets without an
/// explicit `spread_bp` are excluded (no all-in-rate fallback), as are
/// defaulted assets — see [`AssetPool::weighted_avg_spread_bp`].
///
/// [`AssetPool::weighted_avg_spread_bp`]: crate::structured_credit::types::AssetPool::weighted_avg_spread_bp
pub struct CloWasCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for CloWasCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context.instrument_as::<crate::structured_credit::StructuredCredit>()?;

        Ok(clo.pool.weighted_avg_spread_bp())
    }
}
