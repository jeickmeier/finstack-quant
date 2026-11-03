//! Recovery01 and jump-to-default on a term loan priced on a hazard curve.
//!
//! A loan with a `credit_curve_id` prices on the rates-credit tree, which takes
//! recovery from that hazard curve and applies it as fractional recovery of par:
//! a default pays `R` times the funded balance, with no accrued interest. Both
//! metrics use that source and that claim, and the bump convention is the shared
//! one ([`finstack_quant_valuations_core::metrics::sensitivities::recovery01`]).
//! A loan priced any other way has no default in its price, and asking for one
//! is an error rather than a zero. A loan's notional must be positive, so a short
//! is a position with negative quantity, which flips both signs.

use super::irr_helpers::cached_full_schedule;
use super::risk_view::{uses_credit_tree, with_term_loan_risk_view};
use crate::term_loan::TermLoan;
use finstack_quant_core::types::CurveId;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::sensitivities::recovery01::{
    RecoveryReplayTerms, market_with_recovery, recovery01_fd,
};
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

/// The hazard curve of a loan on the credit tree, or the error saying it is not.
fn hazard_curve_id(
    context: &MetricContext,
    loan: &TermLoan,
    metric: &str,
) -> finstack_quant_core::Result<CurveId> {
    match &loan.credit_curve_id {
        Some(id) if uses_credit_tree(context, loan) => Ok(id.clone()),
        _ => Err(finstack_quant_core::Error::Validation(format!(
            "{metric} for term loan '{}' requires hazard pricing: a credit_curve_id and the \
             'tree' model",
            loan.id
        ))),
    }
}

/// `recovery01`: PV change in currency per +1% (0.01 absolute) move in the hazard
/// curve's recovery rate.
///
/// A central finite difference of the full tree PV at `R ± 0.01`, one-sided at a
/// bound. A curve carrying its calibration recipe is re-bootstrapped to the same
/// par spreads at the bumped recovery; a hand-built curve keeps its hazard rates.
/// A quoted loan is held at its calibrated spread, as CS01 is.
pub(super) struct TermLoanRecovery01Calculator;

impl MetricCalculator for TermLoanRecovery01Calculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let hazard_id = {
            let loan: &TermLoan = context.instrument_as()?;
            hazard_curve_id(context, loan, "recovery01")?
        };
        with_term_loan_risk_view(context, |context| {
            let loan: &TermLoan = context.instrument_as()?;
            let as_of = context.as_of;
            let terms = RecoveryReplayTerms::on_discount_curve(loan.discount_curve_id.clone());
            let base_recovery = context.curves.get_hazard(&hazard_id)?.recovery_rate();
            recovery01_fd(base_recovery, context.base_value.amount(), |recovery| {
                let market = market_with_recovery(
                    context,
                    &context.curves,
                    hazard_id.as_str(),
                    recovery,
                    &terms,
                    "Term loan Recovery01",
                )?;
                loan.value_raw(&market, as_of)
            })
        })
    }
}

/// `jump_to_default`: value on immediate default less current value, in currency,
/// for the whole loan, from the lender's perspective.
///
/// ```text
/// jump_to_default = R × funded balance(as_of) − PV(as_of)
/// ```
///
/// The value on default is what the tree pays: `R` of the curve times the funded
/// balance after events on `as_of`, without accrued interest. An undrawn delayed
/// draw is not part of the claim.
pub(super) struct TermLoanJumpToDefaultCalculator;

impl MetricCalculator for TermLoanJumpToDefaultCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let hazard_id = {
            let loan: &TermLoan = context.instrument_as()?;
            hazard_curve_id(context, loan, "jump_to_default")?
        };
        let recovery = context
            .curves
            .get_hazard(&hazard_id)?
            .recovery_rate()
            .clamp(0.0, 1.0);
        let schedule = cached_full_schedule(context)?;
        let balance = crate::bond::pricing::settlement::balance_on(
            &schedule.outstanding_by_date()?,
            context.as_of,
            schedule.get_notional().initial.amount(),
        );
        Ok(recovery * balance - context.base_value.amount())
    }
}
