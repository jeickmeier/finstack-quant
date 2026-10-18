//! Equity option metrics module.
//!
//! Splits equity option metrics into focused calculators per greek and
//! registers them with the `MetricRegistry`. Calculators reuse the pricing
//! engine helpers to ensure consistency between PV and greeks.

mod charm;
mod color;
mod dividend_risk;
mod implied_vol;
pub(crate) mod selected_model;
mod speed;

use crate::equity_option::EquityOption;
use finstack_quant_valuations_core::metrics::sensitivities::config as sens_config;
use finstack_quant_valuations_core::metrics::{MetricContext, MetricRegistry};

/// Relative spot bump shared by the charm, speed and color stencils.
///
/// Starts from the resolved spot bump (`valuations.sensitivities.v1` layered
/// with `metric_pricing_overrides.bump_config`). When
/// `bump_config.adaptive_bumps` is set and no explicit
/// `bump_config.spot_bump_decimal` is given, the bump widens with moneyness,
/// capped at 5× the resolved bump, so deep ITM/OTM stencils stay out of the
/// noise floor.
fn spot_bump_decimal(
    context: &MetricContext,
    option: &EquityOption,
    spot: f64,
) -> finstack_quant_core::Result<f64> {
    let resolved = sens_config::resolve(context)?.spot_bump_decimal;
    let adaptive = context.get_metric_pricing_overrides().is_some_and(|po| {
        po.bump_config.adaptive_bumps && po.bump_config.spot_bump_decimal.is_none()
    });
    if adaptive {
        let moneyness = (spot - option.strike).abs() / option.strike;
        Ok(resolved * (1.0 + 2.0 * moneyness).min(5.0))
    } else {
        Ok(resolved)
    }
}

/// Register equity option metrics with the registry.
pub(crate) fn register_equity_option_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        make_spot_bumper, make_vol_bumper, CrossFactorCalculator, MetricId,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;
    use std::sync::Arc;

    // Custom metric: Dividend risk (dividend yield sensitivity per 1bp)
    registry.register_metric(
        MetricId::Dividend01,
        Arc::new(dividend_risk::DividendRiskCalculator),
        &[InstrumentType::EquityOption],
    )?;
    registry.register_metric(
        MetricId::CrossGammaSpotVol,
        Arc::new(CrossFactorCalculator::new(
            make_spot_bumper,
            make_vol_bumper,
        )),
        &[InstrumentType::EquityOption],
    )?;

    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::EquityOption,
        metrics: [
            (Delta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::delta()),
            (Gamma, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::gamma()),
            (Vega, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::vega()),
            (BucketedVega, finstack_quant_valuations_core::metrics::KeyRateVega::<
                crate::EquityOption,
            >::default()),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Theta, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::theta()),
            (Rho, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::rho()),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
            (Vanna, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::vanna()),
            (Volga, finstack_quant_valuations_core::metrics::OptionGreekCalculator::<crate::EquityOption>::volga()),
            (Charm, charm::CharmCalculator),
            (Color, color::ColorCalculator),
            (Speed, speed::SpeedCalculator),
        ]
    }
    Ok(())
}
