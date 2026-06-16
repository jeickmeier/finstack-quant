//! Weighted Average Rating Factor calculator for CLO

use crate::metrics::MetricContext;
use finstack_quant_core::types::moodys_warf_factor;

/// CLO WARF calculator - Moody's methodology
pub struct CloWarfCalculator;

impl crate::metrics::MetricCalculator for CloWarfCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let clo = context
            .instrument
            .as_any()
            .downcast_ref::<crate::instruments::fixed_income::structured_credit::StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        let mut weighted_sum = 0.0;
        let mut total_balance = 0.0;

        // Performing par only: defaulted assets are carried at recovery in
        // OC haircuts, not in the rating-factor average.
        for asset in clo.pool.assets.iter().filter(|a| !a.is_defaulted) {
            let balance = asset.balance.amount();
            let rating_factor = asset
                .credit_quality
                .map(moodys_warf_factor)
                .transpose()?
                .unwrap_or(3650.0); // Default to B-/CCC+ equivalent when rating is missing

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
