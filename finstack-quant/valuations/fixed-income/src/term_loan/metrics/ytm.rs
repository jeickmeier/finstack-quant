//! YTM metric for term loans via IRR solving.
//!
//! Yield-to-maturity is computed from the fixings-applied pricing schedule's
//! cash-settlement flows (including future DDTL funding legs, excluding PIK
//! capitalization) with an initial price leg at settlement equal to the
//! settlement-date base PV. Uses the same IRR engine and day-count as the loan
//! for consistency.

use crate::term_loan::TermLoan;
use finstack_quant_core::money::Money;
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext};

use super::irr_helpers::{cached_full_schedule, target_price_from_quote_or_model};

/// Yield-to-maturity calculator for term loans.
///
/// Solves for the IRR using the fixings-applied schedule's cash-settlement
/// flows (coupons, fees, amortization, redemptions, and future funding legs)
/// plus an initial price leg at the settlement date.
pub(crate) struct YtmCalculator;

impl MetricCalculator for YtmCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let as_of = context.as_of;

        // Populate the cached full schedule first (mutable context borrow),
        // then borrow the loan for the remaining read-only work.
        let schedule = cached_full_schedule(context)?;
        let loan: &TermLoan = context.instrument_as()?;

        // Compute settlement date using loan calendar/business-day conventions.
        let settlement_date = loan.settlement_date(as_of)?;

        // Initial price leg at settlement (negative = outflow for purchase).
        // Quoted dirty prices and the carried model PV share the settlement origin.
        let target_price = target_price_from_quote_or_model(
            loan,
            &schedule,
            &context.curves,
            as_of,
            context.base_value,
        )?;
        let price_leg = Money::new(-target_price.amount(), target_price.currency())?;

        crate::loan_quotes::ytm_from_schedule(
            &schedule,
            settlement_date,
            price_leg.amount(),
            loan.day_count,
        )
    }
}
