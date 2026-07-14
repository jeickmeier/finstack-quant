//! Weighted Average Life (WAL) calculator for bonds.

use crate::cashflow::primitives::CFKind;
use crate::instruments::Bond;
use crate::metrics::{MetricCalculator, MetricContext};

/// Calculates Weighted Average Life (WAL) for bonds.
///
/// WAL measures the average time until principal is repaid, weighted by the
/// amount of principal returned at each date. For bullet bonds WAL equals
/// the time to maturity; for amortizing bonds it is shorter.
///
/// # Formula
///
/// ```text
/// WAL = Σ(Principal_i × Time_i) / Σ(Principal_i)
/// ```
///
/// where `Principal_i` is the principal repayment at date `i` and `Time_i` is
/// the year fraction from the valuation date to that payment using ACT/365F.
///
/// # Principal Flow Selection
///
/// The calculator uses the bond's full internal cashflow schedule and
/// selects only `CFKind::Amortization` and positive `CFKind::Notional` flows.
/// This correctly excludes coupons, PIK accruals, and the initial negative
/// notional draw, giving an accurate principal-only WAL.
pub(crate) struct BondWalCalculator;

impl MetricCalculator for BondWalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let bond: &Bond = context.instrument_as()?;
        let schedule = bond.full_cashflow_schedule(&context.curves)?;

        let mut weighted_sum = 0.0;
        let mut total_principal = 0.0;

        for cf in schedule.get_flows() {
            if cf.date <= context.as_of {
                continue;
            }

            let principal = match cf.kind {
                CFKind::Amortization => cf.amount.amount(),
                CFKind::Notional if cf.amount.amount() > 0.0 => cf.amount.amount(),
                _ => continue,
            };

            if principal <= 0.0 {
                continue;
            }

            // Propagate day-count failures: a silent 0.0 would zero-weight the
            // flow and understate WAL with no diagnostic.
            let years = finstack_quant_core::dates::DayCount::Act365F.year_fraction(
                context.as_of,
                cf.date,
                finstack_quant_core::dates::DayCountContext::default(),
            )?;

            weighted_sum += principal * years;
            total_principal += principal;
        }

        if total_principal > 0.0 {
            Ok(weighted_sum / total_principal)
        } else {
            Ok(0.0)
        }
    }
}
