//! Convertible Bond metrics module.
//!
//! Provides metric calculators specific to `ConvertibleBond`, split into
//! focused files. The calculators compose with the shared metrics framework
//! and are registered via `register_convertible_metrics`.
//!
//! Exposed metrics:
//! - Parity
//! - Conversion premium
//! - Conversion value (absolute)
//! - Accrued interest
//! - Clean price
//! - Greeks: Delta, Gamma, Vega, Rho, Theta

mod accrued_interest;
mod bond_floor;
mod conversion01;
mod conversion_premium;
mod conversion_value;
mod cs01;
mod dividend_risk;
mod greeks;
mod implied_vol;
mod oas;
mod parity;
// risk_bucketed_dv01 and theta now using generic implementations

use crate::metrics::MetricRegistry;

/// Register convertible bond metrics into the registry.
pub(crate) fn register_convertible_metrics(registry: &mut MetricRegistry) {
    use crate::metrics::{
        make_credit_bumper, make_rates_bumper, make_spot_bumper, make_vol_bumper,
        CrossFactorCalculator, CrossFactorPair, MetricId,
    };
    use crate::pricer::InstrumentType;
    use std::sync::Arc;

    // Custom metrics (not in standard MetricId enum)
    registry.register_metric(
        MetricId::custom("parity"),
        Arc::new(parity::ParityCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::custom("conversion_premium"),
        Arc::new(conversion_premium::ConversionPremiumCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::custom("conversion_value"),
        Arc::new(conversion_value::ConversionValueCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::Accrued,
        Arc::new(accrued_interest::AccruedInterestCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::CleanPrice,
        Arc::new(accrued_interest::CleanPriceCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::Dividend01,
        Arc::new(dividend_risk::DividendRiskCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::Conversion01,
        Arc::new(conversion01::Conversion01Calculator),
        &[InstrumentType::Convertible],
    );

    registry.register_metric(
        MetricId::Oas,
        Arc::new(oas::OasCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::CrossGammaSpotVol,
        Arc::new(CrossFactorCalculator::new(
            CrossFactorPair::SpotVol,
            make_spot_bumper,
            make_vol_bumper,
        )),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::CrossGammaSpotCredit,
        Arc::new(CrossFactorCalculator::new(
            CrossFactorPair::SpotCredit,
            make_spot_bumper,
            make_credit_bumper,
        )),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::CrossGammaRatesCredit,
        Arc::new(CrossFactorCalculator::new(
            CrossFactorPair::RatesCredit,
            make_rates_bumper,
            make_credit_bumper,
        )),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::custom("bond_floor"),
        Arc::new(bond_floor::BondFloorCalculator),
        &[InstrumentType::Convertible],
    );
    registry.register_metric(
        MetricId::ImpliedVol,
        Arc::new(implied_vol::ImpliedVolCalculator),
        &[InstrumentType::Convertible],
    );

    // Standard metrics using macro
    crate::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Convertible,
        metrics: [
            (Delta, greeks::DeltaCalculator),
            (Gamma, greeks::GammaCalculator),
            (Vega, greeks::VegaCalculator),
            (Rho, greeks::RhoCalculator),
            (Cs01, cs01::Cs01Calculator),
            (BucketedCs01, cs01::BucketedCs01Calculator),
            (Dv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::ConvertibleBond,
            >::new(crate::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Theta is now registered universally in metrics::standard_registry()
            (BucketedDv01, crate::metrics::UnifiedDv01Calculator::<
                crate::instruments::ConvertibleBond,
            >::new(crate::metrics::Dv01CalculatorConfig::triangular_key_rate())),
        ]
    }
}
