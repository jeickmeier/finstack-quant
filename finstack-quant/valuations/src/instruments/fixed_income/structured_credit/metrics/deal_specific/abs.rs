//! ABS-specific metrics (speed, delinquency, excess spread, credit enhancement).

use crate::constants::DECIMAL_TO_PERCENT;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::MetricContext;

/// ABS Charge-Off Rate calculator
pub struct AbsChargeOffCalculator;

impl crate::metrics::MetricCalculator for AbsChargeOffCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context.instrument_as::<StructuredCredit>()?;

        let total_balance = abs.pool.total_balance()?;
        if total_balance.amount() > 0.0 {
            Ok(abs.pool.cumulative_defaults.amount() / total_balance.amount() * DECIMAL_TO_PERCENT)
        } else {
            Ok(0.0)
        }
    }
}

/// Current senior credit enhancement as a percent of collateral value.
/// Performing par, outstanding recovery claims and funded cash accounts support
/// the current senior note balance; future excess-spread income is not capital.
pub struct AbsCreditEnhancementCalculator;

impl crate::metrics::MetricCalculator for AbsCreditEnhancementCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context
            .instrument_as::<StructuredCredit>()?
            .resolved_for_pricing()?;
        let pool = &abs.pool;
        let mut collateral = pool
            .performing_balance()?
            .checked_add(pool.collection_account)?
            .checked_add(pool.reserve_account)?
            .checked_add(pool.excess_spread_account)?;
        for asset in &pool.assets {
            if asset.is_defaulted {
                if let Some(recovery) = asset.recovery_amount {
                    collateral = collateral.checked_add(recovery)?;
                }
            }
        }
        let senior = abs
            .tranches
            .tranches
            .iter()
            .min_by_key(|tranche| tranche.payment_priority);
        match senior {
            Some(tranche) if collateral.amount() > 0.0 => Ok((collateral.amount()
                - tranche.current_balance.amount())
                / collateral.amount()
                * DECIMAL_TO_PERCENT),
            _ => Ok(0.0),
        }
    }
}
