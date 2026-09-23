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
/// forward over the remaining reset grid (one point per payment period), so the
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

        let base_rate = match &facility.base_rate_spec {
            crate::instruments::fixed_income::revolving_credit::types::BaseRateSpec::Fixed {
                rate,
            } => *rate + facility.margin_delta_bp_at(context.as_of) * 1e-4,
            crate::instruments::fixed_income::revolving_credit::types::BaseRateSpec::Floating(
                spec,
            ) => {
                let fwd = context.curves.get_forward(spec.index_id.as_str())?;
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
        let commitment_amount = facility.commitment_at(as_of).amount();

        if commitment_amount == 0.0 {
            return Ok(0.0);
        }

        let drawn_amt = drawn_balance_as_of(facility, context.as_of)?.amount();
        let lc_amt = facility.lc_outstanding_at(as_of).amount();
        let undrawn_amt = (commitment_amount - drawn_amt - lc_amt).max(0.0);

        // Interest on drawn
        let interest_cost = drawn_amt * base_rate;

        // Commitment fee on undrawn (evaluating tiers at current utilization)
        let utilization = if commitment_amount > 0.0 {
            (drawn_amt + lc_amt) / commitment_amount
        } else {
            0.0
        };
        let commitment_cost =
            undrawn_amt * (facility.fees.commitment_fee_bp_at(utilization, as_of)? * 1e-4);

        // Usage fee on drawn (evaluating tiers at current utilization)
        let usage_cost = drawn_amt * (facility.fees.usage_fee_bp_at(utilization, as_of)? * 1e-4);

        // Facility fee on total commitment
        let facility_cost = commitment_amount * (facility.fees.facility_fee_bp_at(as_of) * 1e-4);

        // Letter-of-credit and fronting fees on the LC face
        let lc_cost = lc_amt * ((facility.lc_fee_bp_at(as_of) + facility.fronting_fee_bp()) * 1e-4);

        // Total annual cost
        let total_cost = interest_cost + commitment_cost + usage_cost + facility_cost + lc_cost;

        // Weighted average as a percentage of commitment
        let weighted_avg_cost = total_cost / commitment_amount;

        Ok(weighted_avg_cost)
    }
}

/// Time-weighted average of `forward`'s rate over the facility's reset grid
/// from `as_of` to maturity, one point per payment period on an ACT/365F
/// clock. Falls back to the spot forward once `as_of` reaches maturity.
fn average_forward_rate(
    forward: &ForwardCurve,
    facility: &RevolvingCredit,
    as_of: Date,
) -> finstack_quant_core::Result<f64> {
    if as_of >= facility.maturity {
        return Ok(forward.rate(0.0));
    }
    let horizon =
        DayCount::Act365F.year_fraction(as_of, facility.maturity, DayCountContext::default())?;
    let step = facility.frequency.to_years().max(1.0 / 365.0);
    let (mut weighted, mut weight, mut t) = (0.0, 0.0, 0.0);
    while t < horizon {
        let dt = step.min(horizon - t);
        weighted += forward.rate(t) * dt;
        weight += dt;
        t += step;
    }
    Ok(if weight > 0.0 {
        weighted / weight
    } else {
        forward.rate(0.0)
    })
}
