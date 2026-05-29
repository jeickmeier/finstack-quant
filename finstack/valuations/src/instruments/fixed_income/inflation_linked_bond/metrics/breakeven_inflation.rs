//! ILB breakeven inflation metric calculator.

use crate::instruments::fixed_income::inflation_linked_bond::InflationLinkedBond;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::dates::DayCountContext;

/// Breakeven inflation calculator for ILB.
///
/// **Important**: This metric derives a "nominal yield" from the bond's
/// `discount_curve_id`. If that curve is a **real** rate curve (e.g. "USD-REAL"),
/// the breakeven number will be meaningless. For a correct breakeven, the discount
/// curve should be a nominal curve, or the caller should use
/// [`InflationLinkedBond::breakeven_inflation`] directly with an explicit nominal yield.
pub(crate) struct BreakevenInflationCalculator;

impl MetricCalculator for BreakevenInflationCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_core::Result<f64> {
        let ilb: &InflationLinkedBond = context.instrument_as()?;
        let curves = context.curves.as_ref();
        let disc_curve = curves.get_discount(ilb.discount_curve_id.as_str())?;

        let day_count = disc_curve.day_count();
        let base_date = disc_curve.base_date();
        // Provide the coupon frequency so an ACT/ACT (ISMA) discount-curve day
        // count does not error; ignored by other conventions.
        let t = day_count.year_fraction(
            base_date,
            ilb.maturity,
            DayCountContext {
                frequency: Some(ilb.frequency),
                ..Default::default()
            },
        )?;
        // Use annually-compounded (effective-annual) zero rate so that the
        // nominal yield passed to `breakeven_inflation` is on the same basis as
        // the real yield after its Street→Annual conversion inside that method.
        let nominal_yield = disc_curve.zero_annual(t);

        ilb.breakeven_inflation(nominal_yield, curves, context.as_of)
    }
}
