//! Embedded call option value for callable term loans.
//!
//! Prices the callable and straight loans through the same selected model at
//! one OAS:
//! 1) With call schedule -> P_callable
//! 2) Without call schedule -> P_straight
//!
//! The metric is the holder/lender value:
//!   V_embedded = P_callable - P_straight
//!
//! This is negative when callability reduces lender value (borrower owns the call),
//! matching the bond `EmbeddedOptionValue` convention.

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
            .is_some_and(|cs| !cs.calls.is_empty());
        if !has_calls {
            return Ok(0.0);
        }
        super::oas::require_tree_model(loan, context)?;

        super::risk_view::with_term_loan_risk_view(context, |ctx| {
            let priced: TermLoan = ctx.instrument_as::<TermLoan>()?.clone();
            let price_callable =
                ctx.reprice_instrument_raw(&priced, ctx.curves.as_ref(), ctx.as_of)?;
            let mut straight = priced;
            straight.call_schedule = None;
            let price_straight =
                ctx.reprice_instrument_raw(&straight, ctx.curves.as_ref(), ctx.as_of)?;
            Ok(price_callable - price_straight)
        })
    }
}
