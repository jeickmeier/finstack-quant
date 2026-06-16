//! Dollar roll risk and carry metrics.
//!
//! Standard rate-sensitivity metrics (DV01, bucketed DV01, theta) plus
//! carry-specific analytics: implied financing rate and roll specialness.

use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};

/// Implied financing rate metric calculator.
///
/// Computes the annualized implied repo rate from the dollar roll drop,
/// expected coupon income, and principal paydown between settlement dates.
/// Uses the MBS cashflow engine for carry inputs.
///
/// Uses 0.5% SMM (5 CPR) as default prepayment assumption.
pub(crate) struct ImpliedFinancingRateCalculator;

impl MetricCalculator for ImpliedFinancingRateCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let roll: &crate::instruments::DollarRoll = context.instrument_as()?;
        let result = super::carry::implied_financing_rate(roll, 0.005)?;
        Ok(result.implied_rate)
    }
}

/// Roll specialness metric calculator.
///
/// Returns specialness in basis points (repo rate - implied financing rate).
/// Positive means rolling is cheaper than repo financing.
///
/// Uses 0.5% SMM and 5% repo rate as defaults.
pub(crate) struct RollSpecialnessCalculator;

impl MetricCalculator for RollSpecialnessCalculator {
    fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
        let roll: &crate::instruments::DollarRoll = context.instrument_as()?;
        super::carry::roll_specialness(roll, 0.005, 0.05)
    }
}

/// Register dollar roll metrics with the registry.
pub(crate) fn register_dollar_roll_metrics(registry: &mut MetricRegistry) {
    use crate::pricer::InstrumentType;
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::DollarRoll,
        metrics: [
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::DollarRoll,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::DollarRoll,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (ImpliedFinancingRate, ImpliedFinancingRateCalculator),
            (RollSpecialness, RollSpecialnessCalculator)
        ]
    }
}
