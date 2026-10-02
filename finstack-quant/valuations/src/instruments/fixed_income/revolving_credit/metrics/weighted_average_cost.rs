//! Weighted average cost metric for revolving credit facilities.

use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::term_structures::ForwardCurve;

use super::drawn_balance_as_of;

/// Calculator for approximate weighted average cost across all fees and interest.
///
/// Computes a quick proxy for the effective annualized cost combining:
/// - Base interest rate on drawn amounts
/// - Commitment fee on undrawn
/// - Usage fee on drawn
/// - Facility fee on total commitment
///
/// **Note**: This is an approximation that assumes constant balances and flat fees.
/// Floating facilities are projected at the time-weighted average of the index
/// forward over the remaining payment periods (one point per period), so the
/// curve's term structure enters only through that average. It ignores:
/// - Intra-period event effects
/// - Fee tiering (uses current utilization only)
///
/// Returns the weighted average as a rate (decimal).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ApproxWeightedAverageCostCalculator;

impl MetricCalculator for ApproxWeightedAverageCostCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;

        let base_rate = match &facility.rate {
            crate::instruments::fixed_income::loan_terms::RateSpec::Fixed { rate } => {
                *rate + facility.margin_delta_bp_at(context.as_of) * 1e-4
            }
            crate::instruments::fixed_income::loan_terms::RateSpec::Floating(spec) => {
                let fwd = context.curves.get_forward(spec.forward_curve_id.as_str())?;
                let index_rate = average_forward_rate(&fwd, facility, context.as_of)?;
                let mut params =
                    finstack_quant_cashflows::builder::FloatingRateParams::try_from(spec)?;
                params.spread_bp += facility.margin_delta_bp_at(context.as_of);
                finstack_quant_cashflows::builder::rate_helpers::calculate_floating_rate(
                    index_rate, &params,
                )
            }
        };

        let as_of = context.as_of;
        let commitment = facility.commitment_at(as_of).amount();

        if commitment == 0.0 {
            return Ok(0.0);
        }

        let drawn_amt = drawn_balance_as_of(facility, context.as_of)?.amount();
        let lc_amt = facility.lc_outstanding_at(as_of).amount();
        let undrawn_amt = (commitment - drawn_amt - lc_amt).max(0.0);

        // Interest on drawn
        let interest_cost = drawn_amt * base_rate;

        // Commitment fee on undrawn (evaluating tiers at current utilization)
        let utilization = if commitment > 0.0 {
            (drawn_amt + lc_amt) / commitment
        } else {
            0.0
        };
        let commitment_cost =
            undrawn_amt * (facility.fees.commitment_fee_bp_at(utilization, as_of)? * 1e-4);

        // Usage fee on drawn (evaluating tiers at current utilization)
        let usage_cost = drawn_amt * (facility.fees.usage_fee_bp_at(utilization, as_of)? * 1e-4);

        // Facility fee on total commitment
        let facility_cost = commitment * (facility.fees.facility_fee_bp_at(as_of) * 1e-4);

        // Letter-of-credit and fronting fees on the LC face
        let lc_cost = lc_amt * ((facility.lc_fee_bp_at(as_of) + facility.fronting_fee_bp()) * 1e-4);

        // Total annual cost
        let total_cost = interest_cost + commitment_cost + usage_cost + facility_cost + lc_cost;

        // Weighted average as a percentage of commitment
        let weighted_avg_cost = total_cost / commitment;

        Ok(weighted_avg_cost)
    }
}

/// Time-weighted average of the contractual index rate over remaining payment
/// periods. Each sample uses the later of valuation and accrual start on the
/// curve clock, annualized on the full contractual coupon; remaining calendar
/// time supplies its weight. Matured facilities have no remaining interest cost.
fn average_forward_rate(
    forward: &ForwardCurve,
    facility: &RevolvingCredit,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of >= facility.maturity {
        return Ok(0.0);
    }
    let periods = super::super::utils::build_payment_periods(facility)?;
    let (mut weighted, mut weight) = (0.0, 0.0);
    for period in periods.iter().filter(|period| period.accrual_end > as_of) {
        let remaining_start = period.accrual_start.max(as_of);
        let dt = DayCount::Act365F.year_fraction(
            remaining_start,
            period.accrual_end,
            DayCountContext::default(),
        )?;
        let rate = crate::cashflow::builder::rate_helpers::project_index_rate(
            remaining_start,
            forward,
            period.accrual_start,
            period.accrual_end,
            period.accrual_year_fraction,
        )?;
        weighted += rate * dt;
        weight += dt;
    }
    Ok(if weight > 0.0 { weighted / weight } else { 0.0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::loan_terms::RateSpec;
    use crate::instruments::fixed_income::revolving_credit::{DrawRepaySpec, RevolvingCreditFees};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::{BusinessDayConvention, Tenor};
    use finstack_quant_core::money::Money;
    use time::macros::date;

    #[test]
    fn average_rate_preserves_economics_across_curve_clocks() {
        let start = date!(2025 - 01 - 02);
        let end = date!(2025 - 07 - 02);
        let facility = RevolvingCredit::builder()
            .id("RC-COST-BASIS".into())
            .commitment(Money::from((1_000_000_i64, Currency::USD)))
            .drawn(Money::from((1_000_000_i64, Currency::USD)))
            .issue_date(start)
            .maturity(end)
            .rate(RateSpec::Fixed { rate: 0.04 })
            .day_count(DayCount::Act360)
            .frequency(Tenor::quarterly())
            .business_day_convention(BusinessDayConvention::Unadjusted)
            .fees(RevolvingCreditFees::default())
            .draw_repay_spec(DrawRepaySpec::Deterministic(vec![]))
            .discount_curve_id("USD-OIS".into())
            .recovery_rate(0.0)
            .build()
            .expect("facility");
        // The curve is based inside the first coupon: annualization must
        // retain its full contractual dates even when the start is historical.
        let as_of = date!(2025 - 02 - 03);
        for (day_count, rate) in [
            (DayCount::Act360, 0.04),
            (DayCount::Act365F, 0.04 * 365.0 / 360.0),
        ] {
            let curve = ForwardCurve::builder("TERM", 0.25)
                .base_date(as_of)
                .day_count(day_count)
                .knots([(0.0, rate), (1.0, rate)])
                .build()
                .expect("forward curve");
            let projected = average_forward_rate(&curve, &facility, as_of).expect("average rate");
            assert!(
                (projected - 0.04).abs() < 1e-14,
                "{day_count:?}: {projected}"
            );
            assert_eq!(
                average_forward_rate(&curve, &facility, end).expect("matured"),
                0.0
            );
        }
    }
}
