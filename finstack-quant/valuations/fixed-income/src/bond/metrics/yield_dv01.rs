//! Bond price, yield, spread, duration, and risk metric calculations.
//!
use finstack_quant_valuations_core::metrics::{MetricCalculator, MetricContext, MetricId};

/// Calculates yield-basis DV01 for bonds.
///
/// This differs from the generic `MetricId::Dv01` risk metric:
/// - `Dv01`: parallel curve-bump sensitivity to the market discount/projection curves
/// - `YieldDv01`: price sensitivity to a 1bp move in the bond's own quoted yield
///
/// For straight bonds, this is the direct dollar analogue of modified duration:
/// `YieldDv01 = -Price_yield_basis * ModifiedDuration * 1bp`.
///
/// For optioned bonds on the default Workout basis, this uses the same workout
/// yield/cashflow path as `DurationMod`; CallableOas remains an explicit opt-in
/// curve-bump basis.
pub(crate) struct YieldDv01Calculator;

impl MetricCalculator for YieldDv01Calculator {
    fn dependencies(&self) -> &[MetricId] {
        // Ytm is required alongside DurationMod for the price-from-yield repricing.
        // Declaring it explicitly keeps this calculator robust to changes in which
        // metrics are requested in the batch.
        &[MetricId::DurationMod, MetricId::Ytm]
    }

    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let duration_mod = context.computed_metric(&MetricId::DurationMod)?;
        let ytm = context.computed_metric(&MetricId::Ytm)?;
        let price = super::yield_risk_basis(context, ytm)?.price;
        Ok(-(price * duration_mod * 0.0001))
    }
}
