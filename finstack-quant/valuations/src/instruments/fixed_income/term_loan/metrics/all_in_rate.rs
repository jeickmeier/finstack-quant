//! All-in rate metric for term loans.
//!
//! Computes the effective annualized borrower **cash cost** including cash interest
//! and periodic fees, divided by time-weighted outstanding principal.
//!
//! This metric reads cash flows directly from the full cashflow schedule,
//! ensuring perfect consistency with the cashflow generator. PIK interest is
//! excluded from the numerator (cash cost only) but affects outstanding.
//!
//! # Scope: running cash cost, not the fee-inclusive effective rate
//!
//! Only flows strictly **after** the valuation date enter the numerator (they
//! must align with the post-`as_of` time-weighted outstanding denominator). A
//! one-time origination / upfront fee is dated at the issue date, so for a
//! freshly issued loan (`issue_date == as_of`) it is **excluded** here — this
//! metric reports the ongoing cash running cost, which for a plain bullet loan
//! equals the coupon.
//!
//! The fee-inclusive "all-in" borrower cost — the IFRS-9 effective interest
//! rate that amortizes upfront fees and OID over the loan's life — is a
//! separate metric, [`super::oid_eir::OidEirAmortizationCalculator`]
//! (`"oid_eir_rate"`), which solves an XIRR over the full flow set (including
//! the upfront fee when `OidEirSpec::include_fees`). Use that metric when the
//! borrower's effective rate inclusive of origination economics is required.

use crate::instruments::TermLoan;
use crate::metrics::{MetricCalculator, MetricContext};

use super::irr_helpers::cached_full_schedule;

/// All-in rate calculator for term loans.
///
/// Returns the cash-cost all-in rate: (cash interest + fees) / time-weighted outstanding.
/// PIK interest is excluded from the numerator (cash cost only) but affects outstanding
/// (through the outstanding path derived from the full schedule).
///
/// Cash flows are read directly from the generated `CashFlowSchedule` to ensure
/// perfect consistency with the cashflow generator. No fee or rate logic is
/// duplicated.
pub(crate) struct AllInRateCalculator;

impl MetricCalculator for AllInRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        // Snapshot scalar fields off the loan before borrowing the cached schedule.
        let (day_count, maturity) = {
            let loan: &TermLoan = context.instrument_as()?;
            (loan.day_count, loan.maturity)
        };
        let as_of = context.as_of;

        // Use the cached full cashflow schedule — the single source of truth for
        // all flows. Cached across other yield/spread calculators on the same
        // loan to avoid repeated rebuilds in multi-metric requests.
        let schedule = cached_full_schedule(context)?;

        crate::instruments::fixed_income::loan_quotes::all_in_rate_from_schedule(
            &schedule, as_of, day_count, maturity,
        )
    }
}
