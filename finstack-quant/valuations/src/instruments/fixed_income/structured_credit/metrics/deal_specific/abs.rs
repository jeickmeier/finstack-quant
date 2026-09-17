//! ABS-specific metrics (speed, delinquency, excess spread, credit enhancement).

use crate::constants::DECIMAL_TO_PERCENT;
use crate::instruments::fixed_income::structured_credit::{StructuredCredit, TrancheSeniority};
use crate::metrics::MetricContext;

/// ABS delinquency rate: the pool's delinquent balance (every
/// `PoolAsset::delinquency_buckets` entry) as a percent of the total pool
/// balance at the valuation date.
pub struct AbsDelinquencyCalculator;

impl crate::metrics::MetricCalculator for AbsDelinquencyCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context.instrument_as::<StructuredCredit>()?;
        let total_balance = abs.pool.total_balance()?;
        if total_balance.amount() <= 0.0 {
            return Ok(0.0);
        }
        let delinquent: f64 = abs
            .pool
            .assets
            .iter()
            .filter_map(|asset| asset.delinquency_buckets.as_ref())
            .flat_map(|buckets| buckets.iter().map(|m| m.amount()))
            .sum();
        Ok(delinquent / total_balance.amount() * DECIMAL_TO_PERCENT)
    }
}

/// Static excess spread of a card master trust, in percent per annum:
/// portfolio yield less the balance-weighted debt coupon (over the whole
/// investor interest), the servicing fee and the annual charge-off rate.
/// Requires `credit_model.card`; floating coupons resolve on the valuation
/// date's market.
pub struct AbsExcessSpreadCalculator;

impl crate::metrics::MetricCalculator for AbsExcessSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context.instrument_as::<StructuredCredit>()?;
        let Some(card) = abs.credit_model.card else {
            return Err(finstack_quant_core::Error::Validation(
                "abs_excess_spread requires credit_model.card (a card portfolio model)".to_string(),
            ));
        };
        let pool = abs.pool.total_balance()?.amount();
        if pool <= 0.0 {
            return Ok(0.0);
        }
        let mut weighted_coupon = 0.0;
        for tranche in &abs.tranches.tranches {
            if tranche.seniority == TrancheSeniority::Equity {
                continue;
            }
            let rate = tranche.coupon.try_rate_for_period(
                context.as_of,
                context.as_of,
                &context.curves,
            )?;
            weighted_coupon += rate * tranche.current_balance.amount();
        }
        let servicing = abs
            .fees
            .as_ref()
            .map_or(0.0, |fees| fees.servicing_fee_bp / 10_000.0);
        let excess =
            card.portfolio_yield - weighted_coupon / pool - servicing - card.charge_off_rate;
        Ok(excess * DECIMAL_TO_PERCENT)
    }
}

/// Monthly principal payment rate of a card master trust, in percent of the
/// receivables balance per month. Requires `credit_model.card`.
pub struct AbsPaymentRateCalculator;

impl crate::metrics::MetricCalculator for AbsPaymentRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context.instrument_as::<StructuredCredit>()?;
        let Some(card) = abs.credit_model.card else {
            return Err(finstack_quant_core::Error::Validation(
                "abs_payment_rate requires credit_model.card (a card portfolio model)".to_string(),
            ));
        };
        Ok(card.monthly_payment_rate * DECIMAL_TO_PERCENT)
    }
}

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
