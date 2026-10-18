//! FX option metrics module.
//!
//! Splits FX option metrics into focused calculators per greek and registers
//! them with the `MetricRegistry`. Calculators reuse the pricing engine
//! helpers to ensure consistency between PV and greeks.

mod delta_conventions;
mod implied_vol;

use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register FX option metrics with the registry.
pub(crate) fn register_fx_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        make_fx_bumper, make_rates_bumper, make_vol_bumper, CrossFactorCalculator, MetricId,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::sync::Arc;

    // Standard metrics for rho split by domestic/foreign.
    registry.register_metric(
        MetricId::Rho,
        Arc::new(
            finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::rho(
            ),
        ),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::ForeignRho,
        Arc::new(finstack_quant_valuations_core::metrics::OptionGreekCalculator::<
            crate::FxOption,
        >::foreign_rho()),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::DeltaForward,
        Arc::new(delta_conventions::DeltaForwardCalculator),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::DeltaPremiumAdjustedSpot,
        Arc::new(delta_conventions::DeltaPremiumAdjustedSpotCalculator),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::DeltaPremiumAdjustedForward,
        Arc::new(delta_conventions::DeltaPremiumAdjustedForwardCalculator),
        &[InstrumentType::FxOption],
    )?;

    registry.register_metric(
        MetricId::CrossGammaFxVol,
        Arc::new(CrossFactorCalculator::new(make_fx_bumper, make_vol_bumper)),
        &[InstrumentType::FxOption],
    )?;
    registry.register_metric(
        MetricId::CrossGammaFxRates,
        Arc::new(CrossFactorCalculator::new(
            make_fx_bumper,
            make_rates_bumper,
        )),
        &[InstrumentType::FxOption],
    )?;

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::vega()),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Override universal theta (carry) with model theta for FX options.
            (Theta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::theta()),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::FxOption>::volga()),
        ]
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_core::metrics::MetricId;
    use finstack_quant_valuations_core::pricer::InstrumentType;

    #[test]
    fn registers_fx_delta_convention_metrics() {
        let mut registry = MetricRegistry::new();
        register_fx_option_metrics(&mut registry).expect("FX option metric registration");
        let metrics = registry.metrics_for_instrument(InstrumentType::FxOption);

        assert!(metrics.contains(&MetricId::DeltaForward));
        assert!(metrics.contains(&MetricId::DeltaPremiumAdjustedSpot));
        assert!(metrics.contains(&MetricId::DeltaPremiumAdjustedForward));
    }
}
