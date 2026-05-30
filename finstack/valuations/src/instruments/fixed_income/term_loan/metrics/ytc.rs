//! Yield-to-first-call for term loans.
//!
//! Computes the IRR to the earliest valid call date, using the full cashflow
//! schedule with kind-aware filtering plus the call redemption based on
//! pre-exercise outstanding principal.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::money::Money;

use super::irr_helpers::{
    cached_full_schedule, outstanding_before, settlement_discount_factor, solve_irr_to_exercise,
    target_price_from_quote_or_model,
};

/// Yield-to-call calculator for callable term loans.
///
/// For loans with call schedules, solves for IRR to the first valid call date.
/// Redemption amount equals pre-exercise outstanding principal times the call price.
pub(crate) struct YtcCalculator;

impl MetricCalculator for YtcCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;
        let as_of = context.as_of;

        // No exercisable calls → fallback to YTM.
        // MakeWhole calls are excluded: by design the borrower pays at least
        // the continuation value, making the option non-economic.
        let first_call = match &loan.call_schedule {
            Some(cs) => cs
                .calls
                .iter()
                .filter(|c| {
                    c.date >= as_of
                        && c.date <= loan.maturity
                        && !matches!(c.call_type, crate::instruments::fixed_income::term_loan::LoanCallType::MakeWhole { .. })
                })
                .min_by_key(|c| c.date)
                .cloned(),
            None => None,
        };

        let Some(call) = first_call else {
            // use YTM calculator already registered
            return crate::instruments::fixed_income::term_loan::metrics::ytm::YtmCalculator
                .calculate(context);
        };

        // Resolve scalar fields off `loan` before re-borrowing context for the cache.
        let settle_df = settlement_discount_factor(loan, &context.curves, as_of)?;
        let target_price = target_price_from_quote_or_model(loan, context.base_value, settle_df);
        let currency = loan.currency;

        // Use the cached internal schedule (rebuilt only if absent).
        let schedule = cached_full_schedule(context)?;

        // Use pre-exercise outstanding (< call.date) for redemption calculation.
        // outstanding_by_date returns balances AFTER each date, so < gives the
        // balance before any events on the call date.
        let out_path = schedule.outstanding_by_date()?;
        let outstanding = outstanding_before(&out_path, call.date, currency);

        // Redemption = outstanding * call price (as percentage of par)
        let redemption = Money::new(
            outstanding.amount() * (call.price_pct_of_par / 100.0),
            currency,
        );

        // Re-fetch the loan reference (cache write path released the borrow).
        let loan: &TermLoan = context.instrument_as()?;
        solve_irr_to_exercise(loan, &schedule, as_of, target_price, call.date, redemption)
    }
}
