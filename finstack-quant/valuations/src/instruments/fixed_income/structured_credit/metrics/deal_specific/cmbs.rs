//! CMBS-specific metrics (LTV, DSCR).

use crate::instruments::fixed_income::structured_credit::{DealType, StructuredCredit};
use crate::metrics::MetricContext;
use finstack_quant_core::money::Money;

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
        let cmbs = context.instrument_as::<StructuredCredit>()?;

        if cmbs.deal_type != DealType::Cmbs {
            return Err(finstack_quant_core::InputError::Invalid.into());
        }

        // Loan-level NOI drives the pool DSCR when the assets carry it: debt
        // service is the contractual payment (level-pay) or the interest on
        // an interest-only balance, annualized.
        let mut pool_noi = 0.0_f64;
        let mut pool_debt_service = 0.0_f64;
        let mut currency = None;
        for asset in &cmbs.pool.assets {
            let Some(noi) = asset.noi else {
                continue;
            };
            if asset.is_defaulted {
                continue;
            }
            match currency {
                None => currency = Some(noi.currency()),
                Some(existing) if existing != noi.currency() => {
                    return Err(finstack_quant_core::Error::CurrencyMismatch {
                        expected: existing,
                        actual: noi.currency(),
                    });
                }
                Some(_) => {}
            }
            pool_noi += noi.amount();
            pool_debt_service += match asset.contractual_payment {
                Some(payment) => payment.amount() * 12.0,
                None => asset.balance.amount() * asset.rate,
            };
        }
        if currency.is_some() {
            if !pool_debt_service.is_finite() || pool_debt_service <= 0.0 {
                return Err(finstack_quant_core::Error::Validation(
                    "CMBS DSCR requires positive loan debt service".to_string(),
                ));
            }
            return Ok(pool_noi / pool_debt_service);
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
