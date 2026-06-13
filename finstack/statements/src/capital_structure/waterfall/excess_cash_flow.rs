//! ECF calculation, cash-interest deduction rules, and sweep sizing.

use crate::capital_structure::cashflows::CashflowBreakdown;
use crate::capital_structure::state::CapitalStructureState;
use crate::capital_structure::waterfall_spec::EcfSweepSpec;
use crate::error::Result;
use crate::evaluator::EvaluationContext;
use finstack_core::money::Money;
use indexmap::IndexMap;

use super::eval_value_or_formula;
use super::payment_in_kind::is_pik_enabled;

/// Calculate Excess Cash Flow and determine sweep amount.
///
/// Per S&P LCD / standard LPA definitions, ECF deducts **cash interest paid**,
/// not contractual interest. When `ecf_spec.cash_interest_node` is omitted, the
/// fallback sums contractual cash interest only for instruments that are **not**
/// in PIK mode for the current period (PIK'd coupons are not paid in cash).
/// This keeps ECF consistent with the PIK toggle evaluated earlier in the same
/// waterfall step.
///
/// When `deduct_scheduled_principal` is true (scheduled `Amortization` ranks
/// ahead of the prepayment priority in the waterfall), total scheduled
/// principal is also deducted from ECF before the sweep percentage is applied,
/// per the standard LPA ECF definition. Without this deduction the sweep would
/// double-spend the cash already consumed by scheduled amortization.
///
/// Fees paid ahead of the prepayment priority are likewise deducted here so the
/// sweep amount is computed from post-fee excess cash instead of being reduced
/// after the sweep percentage has already been applied.
pub(super) fn calculate_ecf_sweep(
    context: &EvaluationContext,
    ecf_spec: &EcfSweepSpec,
    contractual_flows: &IndexMap<String, CashflowBreakdown>,
    state: &CapitalStructureState,
    deduct_scheduled_principal: bool,
    deduct_fees: bool,
) -> Result<Money> {
    if !(0.0..=1.0).contains(&ecf_spec.sweep_percentage) {
        return Err(crate::error::Error::capital_structure(format!(
            "sweep_percentage must be in [0.0, 1.0], got {}",
            ecf_spec.sweep_percentage
        )));
    }

    let ebitda = eval_value_or_formula(context, &ecf_spec.ebitda_node)?;

    let taxes = ecf_spec
        .taxes_node
        .as_ref()
        .map(|expr| eval_value_or_formula(context, expr))
        .transpose()?
        .unwrap_or(0.0);

    let capex = ecf_spec
        .capex_node
        .as_ref()
        .map(|expr| eval_value_or_formula(context, expr))
        .transpose()?
        .unwrap_or(0.0);

    let wc_change = ecf_spec
        .working_capital_node
        .as_ref()
        .map(|expr| eval_value_or_formula(context, expr))
        .transpose()?
        .unwrap_or(0.0);

    let cash_interest = if let Some(ref expr) = ecf_spec.cash_interest_node {
        eval_value_or_formula(context, expr)?
    } else {
        contractual_flows
            .iter()
            .filter(|(instrument_id, _)| !is_pik_enabled(state, instrument_id))
            .map(|(_, cf)| cf.interest_expense_cash.amount())
            .sum()
    }
    .max(0.0);

    let scheduled_principal = if deduct_scheduled_principal {
        contractual_flows
            .values()
            .map(|cf| cf.principal_payment.amount().max(0.0))
            .sum()
    } else {
        0.0
    };

    let fees = if deduct_fees {
        contractual_flows
            .values()
            .map(|cf| cf.fees.amount().max(0.0))
            .sum()
    } else {
        0.0
    };

    let ecf = ebitda - taxes - capex - wc_change - cash_interest - scheduled_principal - fees;
    let sweep_amount = ecf * ecf_spec.sweep_percentage;
    let currency = base_currency(contractual_flows)?;

    Ok(Money::new(sweep_amount.max(0.0), currency))
}

/// Get base currency from contractual flows (assumes all same currency).
fn base_currency(
    flows: &IndexMap<String, CashflowBreakdown>,
) -> Result<finstack_core::currency::Currency> {
    flows
        .values()
        .next()
        .map(|cf| cf.interest_expense_cash.currency())
        .ok_or_else(|| {
            crate::error::Error::capital_structure(
                "Cannot determine base currency for ECF sweep: no contractual flows provided",
            )
        })
}
