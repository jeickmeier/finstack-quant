//! Weighted Average Rating Factor calculator for CLO

use crate::metrics::MetricContext;
use finstack_quant_core::types::CreditRating;
use finstack_quant_models::credit::moodys_warf_factor;

/// CLO WARF calculator - Moody's methodology
pub struct CloWarfCalculator;

impl crate::metrics::MetricCalculator for CloWarfCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context
            .instrument_as::<crate::instruments::fixed_income::structured_credit::StructuredCredit>(
            )?;

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
