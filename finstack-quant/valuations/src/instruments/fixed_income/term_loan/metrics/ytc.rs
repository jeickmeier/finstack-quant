//! Yield-to-first-call for term loans.
//!
//! Computes the IRR to the earliest valid call date, using the full cashflow
//! schedule with kind-aware filtering plus the call redemption based on
//! pre-exercise outstanding principal.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};

use super::irr_helpers::{
    cached_full_schedule, exercisable_call_candidates, solve_irr_to_exercise,
    target_price_from_quote_or_model,
};

/// Yield-to-call calculator for callable term loans.
///
/// Solves for IRR to the earliest exercisable call candidate — the first
/// coupon date after settlement when a standing provision is already
/// effective, or the first future-dated provision otherwise. Redemption
/// uses the contractual clean call price or make-whole value plus accrued cash.
pub(crate) struct YtcCalculator;

impl MetricCalculator for YtcCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let as_of = context.as_of;

        // Use the cached internal schedule (rebuilt only if absent).
        let schedule = cached_full_schedule(context)?;

        let loan: &TermLoan = context.instrument_as()?;

        // Earliest exercisable candidate (standing or future-dated provision).
        let candidates = exercisable_call_candidates(loan, &schedule, as_of, &context.curves)?;
        let Some((call_date, redemption)) = candidates.into_iter().next() else {
            // No exercisable calls → fallback to YTM.
            return crate::instruments::fixed_income::term_loan::metrics::ytm::YtmCalculator
                .calculate(context);
        };

        let target_price = {
            let loan: &TermLoan = context.instrument_as()?;
            target_price_from_quote_or_model(
                loan,
                &schedule,
                &context.curves,
                as_of,
                context.base_value,
            )?
        };

        // Re-fetch the loan reference (cache write path released the borrow).
        let loan: &TermLoan = context.instrument_as()?;
        solve_irr_to_exercise(loan, &schedule, as_of, target_price, call_date, redemption)
    }
}
