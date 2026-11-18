//! Excess-cash-flow sizing from the agreement's elected formula.

use crate::capital_structure::waterfall_spec::{EcfSweepSpec, EcfTerm};
use crate::error::Result;
use crate::evaluator::EvaluationContext;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

use super::{eval_value_or_formula, money_from_expr};

/// Debt service already paid by the priorities ahead of the ECF sweep.
pub(super) struct PaidAhead {
    /// Cash interest paid, including carried coupon arrears.
    pub cash_interest: Money,
    /// Scheduled principal paid, including carried amortization arrears.
    pub scheduled_principal: Money,
    /// Fees paid, including carried fee arrears.
    pub fees: Money,
}

impl PaidAhead {
    pub(super) fn zero(currency: Currency) -> Self {
        let zero = Money::from((0_i64, currency));
        Self {
            cash_interest: zero,
            scheduled_principal: zero,
            fees: zero,
        }
    }
}

/// The amount the sweep requests in the period.
///
/// `ECF = cash_measure - Σ deductions + Σ credits`, every term elected by the
/// specification and summed in exact money; the request is the sweep
/// percentage of a positive ECF and zero otherwise. A paid-debt-service term
/// is the amount the earlier priorities actually allocated this period.
pub(super) fn calculate_ecf_sweep(
    context: &EvaluationContext,
    spec: &EcfSweepSpec,
    paid: &PaidAhead,
    currency: Currency,
    warnings: &mut Vec<crate::evaluator::EvalWarning>,
) -> Result<Money> {
    let mut term = |term: &EcfTerm| -> Result<Money> {
        Ok(match term {
            EcfTerm::Input { expression } => {
                let value = eval_value_or_formula(context, expression, warnings)?;
                money_from_expr(value, currency, expression)?
            }
            EcfTerm::PaidCashInterest => paid.cash_interest,
            EcfTerm::PaidFees => paid.fees,
            EcfTerm::PaidScheduledPrincipal => paid.scheduled_principal,
        })
    };
    let mut ecf = term(&EcfTerm::Input {
        expression: spec.cash_measure.clone(),
    })?;
    for deduction in &spec.deductions {
        ecf = ecf.checked_sub(term(deduction)?)?;
    }
    for credit in &spec.credits {
        ecf = ecf.checked_add(term(credit)?)?;
    }
    if ecf.amount_decimal() <= rust_decimal::Decimal::ZERO {
        return Ok(Money::from((0_i64, currency)));
    }
    // The request is the swept fraction rounded to the currency's minor unit.
    Ok(ecf.checked_mul_f64(spec.sweep_percentage)?.round(
        u32::from(currency.decimals()),
        finstack_quant_core::config::RoundingMode::MidpointAwayFromZero,
    )?)
}
