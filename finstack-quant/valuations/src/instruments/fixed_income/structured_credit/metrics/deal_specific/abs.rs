//! ABS-specific metrics (speed, delinquency, excess spread, credit enhancement).

use crate::constants::DECIMAL_TO_PERCENT;
use crate::instruments::fixed_income::structured_credit::StructuredCredit;
use crate::metrics::MetricContext;
use finstack_quant_core::types::{Percentage, Rate};

use crate::instruments::fixed_income::structured_credit::DealType;

/// ABS Speed calculator - monthly absolute prepayment speed
pub struct AbsSpeedCalculator {
    default_abs_speed: f64,
}

impl AbsSpeedCalculator {
    /// Create a new ABS speed calculator with specified default speed (as percentage)
    pub fn new(default_abs_speed: f64) -> Self {
        Self { default_abs_speed }
    }

    /// Create a new ABS speed calculator with a typed default speed (percentage).
    pub fn new_pct(default_abs_speed: Percentage) -> Self {
        Self {
            default_abs_speed: default_abs_speed.as_percent(),
        }
    }
}

impl crate::metrics::MetricCalculator for AbsSpeedCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let sc = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Only applicable to ABS deals
        if sc.deal_type != DealType::ABS {
            return Err(finstack_quant_core::Error::from(
                finstack_quant_core::InputError::Invalid,
            ));
        }

        // Return ABS speed from overrides if set, otherwise default
        Ok(sc
            .behavior_overrides
            .abs_speed
            .unwrap_or(self.default_abs_speed))
    }
}

/// ABS Delinquency Rate calculator
pub struct AbsDelinquencyCalculator {
    delinquency_rate: f64,
}

impl AbsDelinquencyCalculator {
    /// Create a new delinquency calculator with specified rate
    pub fn new(delinquency_rate: f64) -> Self {
        Self { delinquency_rate }
    }

    /// Create a new delinquency calculator with a typed rate.
    pub fn new_pct(delinquency_rate: Percentage) -> Self {
        Self {
            delinquency_rate: delinquency_rate.as_decimal(),
        }
    }
}

impl crate::metrics::MetricCalculator for AbsDelinquencyCalculator {
    /// Delinquency rate metric.
    ///
    /// Returns the configured delinquency rate as a percentage. Individual
    /// asset-level delinquency status (30/60/90-day buckets) is not currently
    /// modeled in the pool structure, so this is a passthrough of the
    /// deal-level assumption provided at construction.
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let _abs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        Ok(self.delinquency_rate * DECIMAL_TO_PERCENT)
    }
}

/// ABS Charge-Off Rate calculator
pub struct AbsChargeOffCalculator;

impl crate::metrics::MetricCalculator for AbsChargeOffCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Calculate charge-off rate
        let total_balance = abs.pool.total_balance()?;
        if total_balance.amount() > 0.0 {
            Ok(abs.pool.cumulative_defaults.amount() / total_balance.amount() * DECIMAL_TO_PERCENT)
        } else {
            Ok(0.0)
        }
    }
}

/// ABS Excess Spread calculator
pub struct AbsExcessSpreadCalculator {
    servicing_fee_rate: f64,
}

impl AbsExcessSpreadCalculator {
    /// Create a new excess spread calculator with specified servicing fee rate
    pub fn new(servicing_fee_rate: f64) -> Self {
        Self { servicing_fee_rate }
    }

    /// Create a new excess spread calculator with a typed servicing fee rate.
    pub fn new_rate(servicing_fee_rate: Rate) -> Self {
        Self {
            servicing_fee_rate: servicing_fee_rate.as_decimal(),
        }
    }
}

impl crate::metrics::MetricCalculator for AbsExcessSpreadCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Excess Spread = WAC - WAC of tranches - servicing fees
        let pool_wac = abs.pool.weighted_avg_coupon();

        // Calculate weighted average tranche rate
        let mut tranche_weighted_rate = 0.0;
        let mut tranche_total_balance = 0.0;

        for tranche in &abs.tranches.tranches {
            let rate = tranche.coupon.current_rate(context.as_of);
            tranche_weighted_rate += rate * tranche.current_balance.amount();
            tranche_total_balance += tranche.current_balance.amount();
        }

        let tranche_wac = if tranche_total_balance > 0.0 {
            tranche_weighted_rate / tranche_total_balance
        } else {
            0.0
        };

        let excess_spread = pool_wac - tranche_wac - self.servicing_fee_rate;

        Ok(excess_spread * DECIMAL_TO_PERCENT) // Return as percentage
    }
}

/// ABS Credit Enhancement Level calculator
pub struct AbsCreditEnhancementCalculator;

impl crate::metrics::MetricCalculator for AbsCreditEnhancementCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let abs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Credit Enhancement = Subordination + OC + Excess Spread
        // Simplified: subordination for most senior tranche
        if let Some(senior_tranche) = abs.tranches.tranches.first() {
            let subordination = abs
                .tranches
                .subordination_amount(senior_tranche.id.as_str());
            let pool_balance = abs.pool.total_balance()?;

            if pool_balance.amount() > 0.0 {
                Ok(subordination.amount() / pool_balance.amount() * DECIMAL_TO_PERCENT)
            } else {
                Ok(0.0)
            }
        } else {
            Ok(0.0)
        }
    }
}
