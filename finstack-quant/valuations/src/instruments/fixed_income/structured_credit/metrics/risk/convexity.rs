//! Effective convexity for structured-credit tranches.
//!
//! Convexity is the second-order sensitivity of price to a parallel shift in
//! the discount rate. It is computed by a central second difference of the
//! tranche present value under a small parallel rate bump applied to the
//! discount factors (`df · exp(∓Δy · t)`):
//!
//! ```text
//! Convexity = (PV(+Δy) + PV(-Δy) - 2·PV(0)) / (PV(0) · Δy²)
//! ```
//!
//! For vanilla amortizing cashflows this is positive; premium tranches whose
//! prepayment optionality shortens duration as rates fall can exhibit negative
//! *effective* convexity once the projected cashflows respond to rates (handled
//! upstream in cashflow projection). Units are years².

use crate::cashflow::traits::DatedFlows;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::Result;

/// Parallel rate bump for the central second difference (0.1%).
///
/// Larger than the 1 bp duration bump to keep the second difference clear of
/// floating-point cancellation noise while remaining within the `O(Δy²)`
/// truncation tolerance.
const CONVEXITY_BUMP: f64 = 1e-3;

/// Calculate the effective convexity of a tranche from its cashflows.
///
/// # Arguments
///
/// * `cashflows` - The dated cashflows for the tranche.
/// * `discount_curve` - The discount curve for PV calculation.
/// * `as_of` - The valuation date.
///
/// # Returns
///
/// Convexity in years². Returns `0.0` when the base PV is zero.
///
/// # Errors
///
/// Returns an error if a cashflow date cannot be converted to a year fraction
/// or a discount factor cannot be computed.
pub fn calculate_tranche_convexity(
    cashflows: &DatedFlows,
    discount_curve: &DiscountCurve,
    as_of: Date,
) -> Result<f64> {
    let day_count = DayCount::Act365F;
    let mut pv0 = NeumaierAccumulator::new();
    let mut pv_up = NeumaierAccumulator::new();
    let mut pv_dn = NeumaierAccumulator::new();

    for (date, amount) in cashflows {
        if *date <= as_of {
            continue;
        }
        let t = day_count.year_fraction(as_of, *date, DayCountContext::default())?;
        let df = discount_curve.df_between_dates(as_of, *date)?;
        let base = amount.amount() * df;
        pv0.add(base);
        // Higher rate -> lower PV: bump up multiplies by exp(-Δy·t).
        pv_up.add(base * (-CONVEXITY_BUMP * t).exp());
        pv_dn.add(base * (CONVEXITY_BUMP * t).exp());
    }

    let p0 = pv0.total();
    if p0.abs() < f64::EPSILON {
        return Ok(0.0);
    }
    Ok((pv_up.total() + pv_dn.total() - 2.0 * p0) / (p0 * CONVEXITY_BUMP * CONVEXITY_BUMP))
}

/// Effective convexity calculator for structured credit.
///
/// Reads the tranche's projected cashflows and discount curve from the metric
/// context and returns convexity in years² via
/// [`calculate_tranche_convexity`].
pub struct ConvexityCalculator;

impl MetricCalculator for ConvexityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let flows = context.cashflows.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "context.cashflows".to_string(),
            })
        })?;

        let disc_curve_id = context.discount_curve_id.as_ref().ok_or_else(|| {
            finstack_quant_core::Error::from(finstack_quant_core::InputError::NotFound {
                id: "discount_curve_id".to_string(),
            })
        })?;
        let disc = context.curves.get_discount(disc_curve_id.as_str())?;

        calculate_tranche_convexity(flows, disc.as_ref(), context.as_of)
    }

    fn dependencies(&self) -> &[MetricId] {
        &[]
    }
}
