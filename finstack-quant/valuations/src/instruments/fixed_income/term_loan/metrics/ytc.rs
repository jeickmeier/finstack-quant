//! Yield-to-first-call for term loans.
//!
//! Computes the IRR to the earliest valid call date, using the full cashflow
//! schedule with kind-aware filtering plus the call redemption based on
//! pre-exercise outstanding principal.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::money::Money;

use super::irr_helpers::{
    cached_full_schedule, exercisable_call_candidates, outstanding_before,
    settlement_discount_factor, solve_irr_to_exercise, target_price_from_quote_or_model,
};

/// Yield-to-call calculator for callable term loans.
///
/// Solves for IRR to the earliest exercisable call candidate — the first
/// coupon date after settlement when a standing provision is already
/// effective, or the first future-dated provision otherwise. Redemption
/// equals pre-exercise outstanding principal times the call price.
pub(crate) struct YtcCalculator;

impl MetricCalculator for YtcCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let as_of = context.as_of;

        // Use the cached internal schedule (rebuilt only if absent).
        let schedule = cached_full_schedule(context)?;

        let loan: &TermLoan = context.instrument_as()?;
        let settle_df = settlement_discount_factor(loan, &context.curves, as_of)?;
        let currency = loan.currency;

        // Earliest exercisable candidate (standing or future-dated provision).
        // MakeWhole calls are excluded: by design the borrower pays at least
        // the continuation value, making the option non-economic.
        let candidates = exercisable_call_candidates(loan, &schedule, as_of)?;
        let Some((call_date, price_pct)) = candidates.into_iter().next() else {
            // No exercisable calls → fallback to YTM.
            return crate::instruments::fixed_income::term_loan::metrics::ytm::YtmCalculator
                .calculate(context);
        };

        let target_price = {
            let loan: &TermLoan = context.instrument_as()?;
            target_price_from_quote_or_model(loan, &schedule, as_of, context.base_value, settle_df)?
        };

        // Use pre-exercise outstanding (< call_date) for redemption calculation.
        // outstanding_by_date returns balances AFTER each date, so < gives the
        // balance before any events on the call date.
        let out_path = schedule.outstanding_by_date()?;
        let outstanding = outstanding_before(&out_path, call_date, currency);

        // Redemption = outstanding * call price (as percentage of par)
        let redemption = Money::new(outstanding.amount() * (price_pct / 100.0), currency)?;

        // Re-fetch the loan reference (cache write path released the borrow).
        let loan: &TermLoan = context.instrument_as()?;
        solve_irr_to_exercise(loan, &schedule, as_of, target_price, call_date, redemption)
    }
}
