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
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::DayCountContext;

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
        let (dc, maturity) = {
            let loan: &TermLoan = context.instrument_as()?;
            (loan.day_count, loan.maturity)
        };
        let as_of = context.as_of;

        // Use the cached full cashflow schedule — the single source of truth for
        // all flows. Cached across other yield/spread calculators on the same
        // loan to avoid repeated rebuilds in multi-metric requests.
        let schedule = cached_full_schedule(context)?;

        // Sum cash interest and fee flows from the schedule (exclude PIK).
        // Only include flows after as_of to match the time-weighted denominator.
        let cash_cost: f64 = schedule
            .flows
            .iter()
            .filter(|cf| cf.date > as_of)
            .filter_map(|cf| match cf.kind {
                CFKind::Fixed | CFKind::FloatReset | CFKind::Stub => Some(cf.amount.amount()),
                CFKind::Fee | CFKind::CommitmentFee | CFKind::UsageFee | CFKind::FacilityFee => {
                    Some(cf.amount.amount())
                }
                _ => None,
            })
            .sum();

        // Compute time-weighted outstanding from the outstanding path.
        // The outstanding path gives balances after all events on each date.
        // We integrate outstanding × year_fraction between consecutive dates.
        let out_path = schedule.outstanding_by_date()?;

        let mut time_weighted_outstanding = 0.0;

        // Integrate piecewise-constant outstanding over the loan life after as_of.
        let mut prev_date = as_of;
        let mut prev_outstanding = {
            // Look up outstanding at as_of (last entry <= as_of)
            let mut last = 0.0_f64;
            for (d, amt) in &out_path {
                if *d <= as_of {
                    last = amt.amount();
                } else {
                    break;
                }
            }
            last
        };

        // Walk through outstanding path entries after as_of
        for (d, amt) in &out_path {
            if *d <= as_of {
                continue;
            }
            let target = (*d).min(maturity);
            let yf = dc.year_fraction(prev_date, target, DayCountContext::default())?;
            time_weighted_outstanding += prev_outstanding * yf;
            prev_date = target;
            prev_outstanding = amt.amount();
        }

        // Extend to maturity if the last outstanding entry is before maturity
        if prev_date < maturity {
            let yf = dc.year_fraction(prev_date, maturity, DayCountContext::default())?;
            time_weighted_outstanding += prev_outstanding * yf;
        }

        if time_weighted_outstanding <= 0.0 {
            return Ok(0.0);
        }
        Ok(cash_cost / time_weighted_outstanding)
    }
}
