//! Rebuild a borrowing's schedule after the waterfall changes its principal.
//!
//! The instrument's owner recompiles the contractual schedule from its own
//! terms and the ledger of principal changes the waterfall has posted
//! (prepayments) and of the coupon elections its PIK toggle has made.
//! Statements infers no rate and rewrites no coupon row.

use crate::error::{Error, Result};
use finstack_quant_cashflows::builder::CashFlowSchedule;
use finstack_quant_core::cashflow::{CFKind, CashFlow, CashFlowParams};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::loan_terms::{
    CouponElection, PrincipalPosting,
};
use finstack_quant_valuations::instruments::fixed_income::revolving_credit::RevolvingCredit;
use finstack_quant_valuations::instruments::fixed_income::term_loan::TermLoan;

/// Recompile `instrument`'s schedule with `postings` applied.
///
/// A posting made at a period close takes effect on the following day: the
/// close day still earns on the old principal. The owner dates the posting's
/// zero-cash principal row on that effective date; it is moved back to the
/// close here, so the schedule's balance at the close and the balance carried
/// into the next period both include the change.
///
/// # Arguments
///
/// * `instrument` - The borrowing. Only a term loan or a deterministic
///   revolver can recompile its schedule from postings.
/// * `market` - Market the owner projects floating coupons and reads locked
///   fixings from.
/// * `as_of` - Valuation date of the run, passed to the owner unchanged.
/// * `postings` - Every principal change posted to the instrument so far, in
///   its currency, each with the first day it earns interest.
/// * `elections` - Every dated coupon election made for the instrument so
///   far; only a term loan can carry any.
///
/// # Errors
///
/// Returns a capital-structure error when the instrument cannot rebuild from
/// postings or the owner rejects one (wrong currency, outside the loan's
/// life, before a revolver's balance anchor, beyond the principal or the
/// commitment), and when the rebuilt schedule does not carry a posting.
pub(crate) fn rebuild_from_postings(
    instrument: &dyn Instrument,
    market: &MarketContext,
    as_of: Date,
    postings: &[PrincipalPosting],
    elections: &[CouponElection],
) -> Result<CashFlowSchedule> {
    let any = instrument.as_any();
    let rebuilt = if let Some(loan) = any.downcast_ref::<TermLoan>() {
        loan.cashflow_schedule_with_postings(market, postings, elections)
    } else if let Some(revolver) = any.downcast_ref::<RevolvingCredit>() {
        if !elections.is_empty() {
            return Err(Error::capital_structure(format!(
                "instrument '{}' is a revolving credit facility, which has no PIK coupon \
                 terms to elect",
                instrument.id()
            )));
        }
        revolver.cashflow_schedule_with_postings(market, as_of, postings)
    } else {
        return Err(Error::capital_structure(format!(
            "instrument '{}' cannot rebuild its schedule after a principal change: only a \
             term loan or a revolving credit facility recompiles its coupons, principal and \
             fees from posted principal",
            instrument.id()
        )));
    }
    .map_err(|error| {
        Error::capital_structure(format!(
            "instrument '{}' could not rebuild its schedule after a principal change: {error}",
            instrument.id()
        ))
    })?;

    let currency = rebuilt.get_notional().initial.currency();
    let zero = Money::from((0_i64, currency));
    let mut flows: Vec<CashFlow> = rebuilt.get_flows().to_vec();
    let mut moved = vec![false; flows.len()];
    for posting in postings {
        let close = posting
            .effective_date
            .previous_day()
            .ok_or_else(|| Error::capital_structure("principal posting date is out of range"))?;
        let index = flows
            .iter()
            .enumerate()
            .position(|(index, flow)| {
                !moved[index]
                    && flow.get_kind() == CFKind::PrePayment
                    && flow.get_date() == posting.effective_date
                    && flow.get_amount() == zero
                    && flow.get_principal_delta() == Some(posting.delta)
            })
            .ok_or_else(|| {
                Error::capital_structure(format!(
                    "instrument '{}': the rebuilt schedule does not carry the principal change \
                     of {} effective {}",
                    instrument.id(),
                    posting.delta,
                    posting.effective_date
                ))
            })?;
        flows[index] = CashFlowParams::new(close, None, zero, CFKind::PrePayment, 0.0, None)
            .with_principal_delta(posting.delta)
            .with_principal_date(close)
            .build()?;
        moved[index] = true;
    }
    Ok(CashFlowSchedule::from_flows(
        flows,
        rebuilt.get_notional().clone(),
        rebuilt.get_day_count(),
        rebuilt.get_meta().clone(),
    ))
}
