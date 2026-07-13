//! RMBS-specific metrics (LTV, FICO, WAL with PSA adjustments).

use crate::cashflow::builder::schedule::weighted_average_life_from_principal;
use crate::instruments::fixed_income::structured_credit::pricing::run_simulation;
use crate::instruments::fixed_income::structured_credit::{DealType, StructuredCredit};
use crate::metrics::MetricContext;

/// RMBS Weighted Average LTV calculator
pub struct RmbsLtvCalculator {
    default_ltv: f64,
}

impl RmbsLtvCalculator {
    /// Create a new RMBS LTV calculator with specified default LTV (as percentage)
    pub fn new(default_ltv: f64) -> Self {
        Self { default_ltv }
    }
}

impl crate::metrics::MetricCalculator for RmbsLtvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let rmbs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Use credit factors LTV or calculate from pool
        if let Some(ltv) = rmbs.credit_factors.ltv {
            Ok(ltv * 100.0)
        } else {
            Ok(self.default_ltv)
        }
    }
}

/// RMBS Weighted Average FICO calculator
pub struct RmbsFicoCalculator {
    default_fico: f64,
}

impl RmbsFicoCalculator {
    /// Create a new RMBS FICO calculator with specified default FICO score
    pub fn new(default_fico: f64) -> Self {
        Self { default_fico }
    }
}

impl crate::metrics::MetricCalculator for RmbsFicoCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let rmbs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Use credit factors credit score or default
        if let Some(fico) = rmbs.credit_factors.credit_score {
            Ok(fico as f64)
        } else {
            Ok(self.default_fico)
        }
    }
}

/// RMBS WAL calculator with PSA prepayment adjustments
pub struct RmbsWalCalculator;

impl crate::metrics::MetricCalculator for RmbsWalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let rmbs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        if rmbs.deal_type != DealType::RMBS {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        let tranche_flows = run_simulation(rmbs, context.curves.as_ref(), context.as_of)?;
        weighted_average_life_from_principal(
            tranche_flows
                .values()
                .flat_map(|flows| flows.principal_flows.iter().copied()),
            context.as_of,
        )
    }
}
