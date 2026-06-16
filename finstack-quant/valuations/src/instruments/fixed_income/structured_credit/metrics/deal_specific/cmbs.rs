//! CMBS-specific metrics (LTV, DSCR).

use crate::constants::DECIMAL_TO_PERCENT;
use crate::instruments::fixed_income::structured_credit::{DealType, StructuredCredit};
use crate::metrics::MetricContext;
use finstack_quant_core::money::Money;

/// CMBS Weighted Average LTV calculator
pub struct CmbsLtvCalculator {
    default_ltv: f64,
}

impl CmbsLtvCalculator {
    /// Create a new CMBS LTV calculator with specified default LTV (as percentage)
    pub fn new(default_ltv: f64) -> Self {
        Self { default_ltv }
    }
}

impl crate::metrics::MetricCalculator for CmbsLtvCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let cmbs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        // Use credit factors LTV or default
        if let Some(ltv) = cmbs.credit_factors.ltv {
            Ok(ltv * DECIMAL_TO_PERCENT)
        } else {
            Ok(self.default_ltv)
        }
    }
}

/// CMBS DSCR calculator
pub struct CmbsDscrCalculator;

impl CmbsDscrCalculator {
    /// Create a new DSCR calculator.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CmbsDscrCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::metrics::MetricCalculator for CmbsDscrCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let cmbs = context
            .instrument
            .as_any()
            .downcast_ref::<StructuredCredit>()
            .ok_or(finstack_quant_core::InputError::Invalid)?;

        if cmbs.deal_type != DealType::CMBS {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        let noi = required_money(cmbs.credit_factors.annual_noi, "annual_noi")?;
        let debt_service = required_money(
            cmbs.credit_factors.annual_debt_service,
            "annual_debt_service",
        )?;

        if noi.currency() != debt_service.currency() {
            return Err(finstack_quant_core::Error::CurrencyMismatch {
                expected: noi.currency(),
                actual: debt_service.currency(),
            });
        }
        if !debt_service.amount().is_finite() || debt_service.amount() <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "CMBS DSCR requires positive annual_debt_service".to_string(),
            ));
        }
        if !noi.amount().is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "CMBS DSCR requires finite annual_noi".to_string(),
            ));
        }

        Ok(noi.amount() / debt_service.amount())
    }
}

fn required_money(value: Option<Money>, field: &str) -> finstack_quant_core::Result<Money> {
    value.ok_or_else(|| {
        finstack_quant_core::Error::Validation(format!("CMBS DSCR requires credit_factors.{field}"))
    })
}
