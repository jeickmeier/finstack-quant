//! FX option metrics module.
//!
//! Defines FX option metric calculators per greek and registers
//! them with the `MetricRegistry`. Calculators reuse the pricing engine
//! helpers to ensure consistency between PV and greeks.

mod delta_conventions;

use crate::fx_option::FxOption;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register FX option metrics with the registry.
pub(crate) fn register_fx_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::sensitivities::cross_factor::{
        make_fx_bumper, make_rates_bumper, make_vol_bumper, CrossFactorCalculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::FxOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::vega()),
            (Dv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::parallel_combined())),
            // Override universal theta (carry) with model theta for FX options.
            (Theta, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::theta()),
            (ImpliedVol, ImpliedVolCalculator),
            (BucketedDv01, finstack_quant_valuations_core::metrics::sensitivities::dv01::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::sensitivities::dv01::Dv01CalculatorConfig::triangular_key_rate())),
            (Vanna, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::volga()),
            // Standard metrics for rho split by domestic/foreign.
            (Rho, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::rho()),
            (ForeignRho, finstack_quant_valuations_core::metrics::sensitivities::option_greeks::OptionGreekCalculator::<crate::fx_option::FxOption>::foreign_rho()),
            (DeltaForward, delta_conventions::DeltaForwardCalculator),
            (DeltaPremiumAdjustedSpot, delta_conventions::DeltaPremiumAdjustedSpotCalculator),
            (DeltaPremiumAdjustedForward, delta_conventions::DeltaPremiumAdjustedForwardCalculator),
            (CrossGammaFxVol, CrossFactorCalculator::new(make_fx_bumper, make_vol_bumper)),
            (CrossGammaFxRates, CrossFactorCalculator::new(make_fx_bumper, make_rates_bumper)),
        ]
    }
    Ok(())
}

/// Implied volatility metric for FX options.
///
/// Solves for σ such that model PV(σ) equals the observed option premium in
/// `instrument_pricing_overrides.market_quotes.quoted_premium`, using the
/// configured pricer (Hybrid solver under the hood) with log-σ
/// parameterization.
pub(crate) struct ImpliedVolCalculator;

impl finstack_quant_valuations_core::metrics::MetricCalculator for ImpliedVolCalculator {
    fn calculate(
        &self,
        context: &mut finstack_quant_valuations_core::metrics::MetricContext,
    ) -> finstack_quant_core::Result<f64> {
        let option: &FxOption = context.instrument_as()?;
        let target_price = option
            .instrument_pricing_overrides
            .market_quotes
            .required_quoted_premium()?;
        option.implied_vol(&context.curves, context.as_of, target_price)
    }
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
