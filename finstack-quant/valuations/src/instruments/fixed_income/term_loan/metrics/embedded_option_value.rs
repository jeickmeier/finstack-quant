//! Embedded call option value for callable term loans.
//!
//! Computed by pricing the loan twice on the same tree model:
//! 1) With call schedule (borrower optimal exercise with friction) -> P_callable
//! 2) Without call schedule (no optionality) -> P_straight
//!
//! The embedded call option value is returned as:
//!   V_call = P_straight - P_callable
//!
//! This is positive when callability reduces lender value (borrower owns the call).

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};

/// Embedded option value calculator for term loans (callability only).
pub(crate) struct EmbeddedOptionValueCalculator;

impl MetricCalculator for EmbeddedOptionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let loan: &TermLoan = context.instrument_as()?;

        let has_calls = loan
            .call_schedule
            .as_ref()
            .map(|cs| !cs.calls.is_empty())
            .unwrap_or(false);
        if !has_calls {
            return Ok(0.0);
        }

        let market = context.curves.as_ref();
        let as_of = context.as_of;

        let pricer =
            crate::instruments::fixed_income::term_loan::pricing::TermLoanTreePricer::new();

        // Price 1: WITH callability
        let price_callable = pricer.price_callable(loan, market, as_of)?.amount();

        // Price 2: WITHOUT callability (straight loan), priced on the same tree model.
        let mut straight = loan.clone();
        straight.call_schedule = None;
        let price_straight = pricer.price_callable(&straight, market, as_of)?.amount();

        Ok(price_straight - price_callable)
    }
}
