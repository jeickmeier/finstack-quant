//! ECF calculation, cash-interest deduction rules, and sweep sizing.

use crate::capital_structure::waterfall_spec::EcfSweepSpec;
use crate::error::Result;
use crate::evaluator::EvaluationContext;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;

use super::eval_value_or_formula;

/// Debt service already paid by priorities ahead of the ECF sweep.
#[derive(Default)]
pub(super) struct EcfDeductions {
    /// Cash interest paid, including carried coupon arrears.
    pub cash_interest: f64,
    /// Scheduled principal paid, including carried amortization arrears.
    pub scheduled_principal: f64,
    /// Fees paid, including carried fee arrears.
    pub fees: f64,
}

/// Calculate Excess Cash Flow and determine sweep amount.
///
/// Per S&P LCD / standard LPA definitions, ECF deducts **cash interest paid**,
/// not contractual interest. When `ecf_spec.cash_interest_node` is omitted, the
/// fallback deducts cash interest actually allocated by earlier priorities,
/// including carried arrears. PIK coupons and unpaid claims consume no cash.
/// Fees and scheduled principal paid by earlier priorities are also deducted
/// before applying the sweep percentage. Later priorities reserve no cash.
pub(super) fn calculate_ecf_sweep(
    context: &EvaluationContext,
    ecf_spec: &EcfSweepSpec,
    paid: &EcfDeductions,
    currency: Currency,
    warnings: &mut Vec<crate::evaluator::EvalWarning>,
) -> Result<Money> {
    if !(0.0..=1.0).contains(&ecf_spec.sweep_percentage) {
        return Err(crate::error::Error::capital_structure(format!(
            "sweep_percentage must be in [0.0, 1.0], got {}",
            ecf_spec.sweep_percentage
        )));
    }

    let ebitda = eval_value_or_formula(context, &ecf_spec.ebitda_node, warnings)?;

    let taxes = ecf_spec
        .taxes_node
        .as_ref()
        .map(|expr| eval_value_or_formula(context, expr, warnings))
        .transpose()?
        .unwrap_or(0.0);

    let capex = ecf_spec
        .capex_node
        .as_ref()
        .map(|expr| eval_value_or_formula(context, expr, warnings))
        .transpose()?
        .unwrap_or(0.0);

    let wc_change = ecf_spec
        .working_capital_node
        .as_ref()
        .map(|expr| eval_value_or_formula(context, expr, warnings))
        .transpose()?
        .unwrap_or(0.0);

    let cash_interest = if let Some(ref expr) = ecf_spec.cash_interest_node {
        eval_value_or_formula(context, expr, warnings)?
    } else {
        paid.cash_interest
    }
    .max(0.0);

    let ecf =
        ebitda - taxes - capex - wc_change - cash_interest - paid.scheduled_principal - paid.fees;
    let sweep_amount = ecf * ecf_spec.sweep_percentage;

    // Every ECF input is finite (`eval_value_or_formula` enforces it), but the
    // arithmetic above can still overflow `Decimal`'s range on extreme inputs.
    super::money_from_expr(sweep_amount.max(0.0), currency, &ecf_spec.ebitda_node)
}
