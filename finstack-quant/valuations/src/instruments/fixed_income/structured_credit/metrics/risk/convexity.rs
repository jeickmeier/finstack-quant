//! Modified convexity for structured-credit tranches.
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
//! This is the *modified* convexity of the tranche's already-projected
//! cashflows: the bump is applied to discounting only, holding the cashflows
//! fixed (mirroring [`super::duration`]), so it is always non-negative. It is
//! **not** an *effective* convexity — it does not re-project the cashflows under
//! the rate shift, so prepayment-driven negative convexity does not appear here;
//! an effective measure would reprice through the cashflow engine at ±Δy. Units
//! are years².

use crate::cashflow::traits::DatedFlows;
use crate::metrics::{MetricCalculator, MetricContext, MetricId};
use finstack_quant_core::dates::{Date, DayCountContext};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::summation::NeumaierAccumulator;
use finstack_quant_core::Result;

/// Parallel rate bump for the central second difference (0.1%).
///
/// Larger than the 1 bp duration bump to keep the second difference clear of
/// floating-point cancellation noise while remaining within the `O(Δy²)`
/// truncation tolerance.
const CONVEXITY_BUMP: f64 = 1e-3;

/// Calculate the modified convexity of a tranche from its (fixed) cashflows.
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
    let day_count = crate::instruments::fixed_income::structured_credit::metrics::METRIC_TIME_BASIS;
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
    // SC-m04: guard relative to the cashflow scale, not `f64::EPSILON`.
    //
    // `f64::EPSILON` is 2.2e-16 — an absolute threshold on a CURRENCY amount.
    // A tranche whose PV has collapsed to a residual 1e-12 sails past it and
    // divides by that residual, producing an astronomically large convexity
    // from what is numerically zero. Scaling to the undiscounted cashflow
    // magnitude makes the guard meaningful at any notional, from a JPY deal to
    // a small equity strip.
    let scale: f64 = cashflows
        .iter()
        .filter(|(date, _)| *date > as_of)
        .map(|(_, amount)| amount.amount().abs())
        .sum();
    if p0.abs() <= scale * 1e-12 {
        return Ok(0.0);
    }
    Ok((pv_up.total() + pv_dn.total() - 2.0 * p0) / (p0 * CONVEXITY_BUMP * CONVEXITY_BUMP))
}

/// Modified convexity calculator for structured credit.
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
