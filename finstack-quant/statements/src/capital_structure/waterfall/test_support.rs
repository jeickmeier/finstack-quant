//! Test-only waterfall drivers.

use super::{WaterfallPeriodResult, execute_waterfall};
use crate::capital_structure::{CapitalStructureState, CashflowBreakdown, WaterfallSpec};
use crate::error::Result;
use crate::evaluator::EvaluationContext;
use indexmap::IndexMap;

/// Validate the specification, as loading a model does, then run the
/// waterfall on a copy of `state` and commit it only on success, as the
/// evaluator does for a whole period.
pub(crate) fn execute_waterfall_staged(
    context: &EvaluationContext,
    waterfall_spec: &WaterfallSpec,
    state: &mut CapitalStructureState,
    contractual_flows: &IndexMap<String, CashflowBreakdown>,
    pool_currency: Option<finstack_quant_core::currency::Currency>,
) -> Result<WaterfallPeriodResult> {
    waterfall_spec.validate()?;
    let mut staged = state.clone();
    let result = execute_waterfall(
        context,
        waterfall_spec,
        &mut staged,
        contractual_flows,
        pool_currency,
    )?;
    *state = staged;
    Ok(result)
}

/// Run the waterfall for fixtures that state one aggregate
/// principal amount per instrument, by dating that amount inside the
/// allocation period (due and effective together) and setting the economic
/// close to `opening + PIK - scheduled principal`. An instrument whose dated
/// claims the test already supplied is left as it is.
pub(crate) fn execute_waterfall_same_period(
    context: &EvaluationContext,
    waterfall_spec: &WaterfallSpec,
    state: &mut CapitalStructureState,
    contractual_flows: &IndexMap<String, CashflowBreakdown>,
) -> Result<WaterfallPeriodResult> {
    use finstack_quant_core::dates::{Date, Period};
    waterfall_spec.validate()?;
    let period = Period {
        id: context.period_id,
        start: Date::MIN,
        end: Date::MAX,
        is_actual: false,
    };
    let mut dated = contractual_flows.clone();
    // Staged, so a failed run leaves the caller's state as it was.
    let mut staged = state.clone();
    for (instrument_id, breakdown) in &mut dated {
        if staged.period_principal_flows.contains_key(instrument_id) {
            continue;
        }
        let currency = breakdown.interest_expense_cash.currency();
        breakdown.debt_balance = staged
            .get_opening_balance(instrument_id, currency)
            .checked_add(breakdown.interest_expense_pik)?
            .checked_sub(breakdown.principal_payment)?;
        let claims = if breakdown.principal_payment.amount() == 0.0 {
            Vec::new()
        } else {
            vec![crate::capital_structure::PrincipalClaim::at_par(
                Date::MIN,
                Date::MIN,
                breakdown.principal_payment,
            )]
        };
        staged.set_period_principal_flows(instrument_id.clone(), &period, claims)?;
    }
    let result = execute_waterfall(context, waterfall_spec, &mut staged, &dated, None)?;
    *state = staged;
    Ok(result)
}
