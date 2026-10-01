//! Shared clean redemption for dated loan call provisions.

use crate::cashflow::builder::CashFlowSchedule;
use crate::instruments::fixed_income::bond::pricing::engine::tree::BondValuator;
use crate::instruments::fixed_income::term_loan::{LoanCall, LoanCallType};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::Result;

/// Clean call redemption; the caller adds accrued cash interest once.
///
/// # Arguments
///
/// * `call` - Effective call provision, including its clean percentage-of-par floor.
/// * `schedule` - Canonical loan schedule; PIK capitalization is not cash consideration.
/// * `market` - Reference discount curve for a make-whole provision.
/// * `exercise` - Exercise date, which excludes already-due flows from the reference PV.
/// * `outstanding` - Principal immediately before exercise-date events, in loan currency.
/// * `accrued` - Cash interest accrued to exercise, subtracted before applying the clean floor.
pub(crate) fn clean_call_price(
    call: &LoanCall,
    schedule: &CashFlowSchedule,
    market: &MarketContext,
    exercise: Date,
    outstanding: f64,
    accrued: f64,
) -> Result<f64> {
    let floor = outstanding * call.price_pct_of_par / 100.0;
    match &call.call_type {
        LoanCallType::MakeWhole(spec) => {
            let reference = market.get_discount(&spec.reference_curve_id)?;
            let flows = schedule
                .get_flows()
                .iter()
                .filter(|flow| flow.kind != CFKind::Pik)
                .map(|flow| (flow.date, flow.amount))
                .collect::<Vec<_>>();
            BondValuator::make_whole_call_price(
                spec,
                reference.as_ref(),
                exercise,
                &flows,
                floor,
                accrued,
            )
        }
        _ => Ok(floor),
    }
}
