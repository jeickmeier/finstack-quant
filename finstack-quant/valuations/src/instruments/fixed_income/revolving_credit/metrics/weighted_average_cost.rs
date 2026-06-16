//! Weighted average cost metric for revolving credit facilities.

use crate::instruments::RevolvingCredit;
use crate::metrics::{MetricCalculator, MetricContext};
use rust_decimal::prelude::ToPrimitive;

/// Calculator for approximate weighted average cost across all fees and interest.
///
/// Computes a quick proxy for the effective annualized cost combining:
/// - Base interest rate on drawn amounts
/// - Commitment fee on undrawn
/// - Usage fee on drawn
/// - Facility fee on total commitment
///
/// **Note**: This is an approximation that assumes constant balances and flat fees.
/// It uses a single 3M forward rate for floating facilities and ignores:
/// - Curve term structure
/// - Intra-period event effects
/// - Fee tiering (uses current utilization only)
///
/// Returns the weighted average as a rate (decimal).
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct ApproxWeightedAverageCostCalculator;

impl MetricCalculator for ApproxWeightedAverageCostCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let facility: &RevolvingCredit = context.instrument_as()?;

        // Get base interest rate
        let base_rate = match &facility.base_rate_spec {
            crate::instruments::fixed_income::revolving_credit::types::BaseRateSpec::Fixed {
                rate,
            } => *rate,
            crate::instruments::fixed_income::revolving_credit::types::BaseRateSpec::Floating(
                spec,
            ) => {
                // Use forward curve to get current rate
                let fwd = context.curves.get_forward(spec.index_id.as_str())?;
                let mut index_rate = fwd.rate(0.25); // Use 3M as representative

                // Apply floor/cap to the index rate before adding spread,
                // consistent with the cashflow engines.
                if let Some(floor) = spec.index_floor_bp {
                    let floor_f64 = floor.to_f64().unwrap_or(0.0);
                    index_rate = index_rate.max(floor_f64 * 1e-4);
                }
                if let Some(cap) = spec.index_cap_bp {
                    let cap_f64 = cap.to_f64().unwrap_or(f64::MAX);
                    index_rate = index_rate.min(cap_f64 * 1e-4);
                }

                let spread_bp_f64 = spec.spread_bp.to_f64().unwrap_or_default();
                index_rate + (spread_bp_f64 * 1e-4)
            }
        };

        let commitment_amount = facility.commitment_amount.amount();

        if commitment_amount == 0.0 {
            return Ok(0.0);
        }

        // Calculate total annual cost
        let drawn_amt = facility.drawn_amount.amount();
        let undrawn_amt = commitment_amount - drawn_amt;

        // Interest on drawn
        let interest_cost = drawn_amt * base_rate;

        // Commitment fee on undrawn (evaluating tiers at current utilization)
        let utilization = if commitment_amount > 0.0 {
            drawn_amt / commitment_amount
        } else {
            0.0
        };
        let commitment_cost = undrawn_amt * (facility.fees.commitment_fee_bps(utilization) * 1e-4);

        // Usage fee on drawn (evaluating tiers at current utilization)
        let usage_cost = drawn_amt * (facility.fees.usage_fee_bps(utilization) * 1e-4);

        // Facility fee on total commitment
        let facility_cost = commitment_amount * (facility.fees.facility_fee_bp * 1e-4);

        // Total annual cost
        let total_cost = interest_cost + commitment_cost + usage_cost + facility_cost;

        // Weighted average as a percentage of commitment
        let weighted_avg_cost = total_cost / commitment_amount;

        Ok(weighted_avg_cost)
    }
}
