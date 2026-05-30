//! Yield-to-worst for term loans with callable schedules.
//!
//! Computes the minimum yield across all valid call dates and final maturity,
//! using kind-aware cashflow filtering and pre-exercise outstanding principal
//! for redemption amounts.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::dates::Date;
use finstack_core::money::Money;

use super::irr_helpers::{
    cached_full_schedule, outstanding_before, settlement_discount_factor, solve_irr_to_exercise,
    target_price_from_quote_or_model,
};

/// Yield-to-worst calculator for callable term loans.
///
/// Solves for the worst (minimum) yield across all call dates and maturity.
pub(crate) struct YtwCalculator;

impl MetricCalculator for YtwCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        // Snapshot scalar fields off the loan before borrowing the cached schedule.
        let (currency, maturity, dirty_now, candidate_calls) = {
            let loan: &TermLoan = context.instrument_as()?;
            let settle_df = settlement_discount_factor(loan, &context.curves, context.as_of)?;
            let dirty_now = target_price_from_quote_or_model(loan, context.base_value, settle_df);
            let calls: Vec<_> = if let Some(cs) = &loan.call_schedule {
                cs.calls
                    .iter()
                    .filter(|c| {
                        c.date >= context.as_of
                            && c.date <= loan.maturity
                            && !matches!(
                                c.call_type,
                                crate::instruments::fixed_income::term_loan::LoanCallType::MakeWhole { .. }
                            )
                    })
                    .map(|c| (c.date, c.price_pct_of_par))
                    .collect()
            } else {
                Vec::new()
            };
            (loan.currency, loan.maturity, dirty_now, calls)
        };
        let as_of = context.as_of;

        // Use the cached internal schedule (reused across all candidates).
        let schedule = cached_full_schedule(context)?;
        let out_path = schedule.outstanding_by_date()?;

        // Candidate exercises: each exercisable call and final maturity.
        let mut candidates: Vec<(Date, Money)> = Vec::with_capacity(candidate_calls.len() + 1);
        for (date, price_pct) in candidate_calls {
            let out = outstanding_before(&out_path, date, currency);
            let redemption = Money::new(out.amount() * (price_pct / 100.0), currency);
            candidates.push((date, redemption));
        }

        // Always include maturity: pre-redemption outstanding
        // (at maturity itself, outstanding is 0 after the final redemption)
        let final_out = outstanding_before(&out_path, maturity, currency);
        candidates.push((maturity, final_out));

        // Re-borrow the loan for the IRR solver (only immutable borrows of
        // context outstanding at this point).
        let loan: &TermLoan = context.instrument_as()?;

        // Worst yield = min yield across candidates
        let mut y_worst = f64::INFINITY;
        for (exercise_date, redemption) in candidates {
            let y = solve_irr_to_exercise(
                loan,
                &schedule,
                as_of,
                dirty_now,
                exercise_date,
                redemption,
            )?;
            if y < y_worst {
                y_worst = y;
            }
        }
        Ok(y_worst)
    }
}
