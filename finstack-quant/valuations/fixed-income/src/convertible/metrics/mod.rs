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
mod conversion01;
mod cs01;
mod dividend_risk;
mod greeks;
mod implied_vol;
mod oas;

use crate::convertible::ConvertibleBond;
use finstack_quant_core::Result;
use finstack_quant_valuations_core::metrics::MetricCalculator;
use finstack_quant_valuations_core::metrics::MetricContext;
use finstack_quant_valuations_core::metrics::MetricRegistry;

/// Register convertible bond metrics into the registry.
pub(crate) fn register_convertible_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), finstack_quant_valuations_core::metrics::MetricRegistryError> {
    use finstack_quant_valuations_core::metrics::{
        make_credit_bumper, make_rates_bumper, make_spot_bumper, make_vol_bumper,
        CrossFactorCalculator,
    };
    use finstack_quant_valuations_core::pricer::InstrumentType;

    // Standard metrics using macro
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::Convertible,
        metrics: [
            (Delta, greeks::DeltaCalculator),
            (Gamma, greeks::GammaCalculator),
            (Vega, greeks::VegaCalculator),
            (Rho, greeks::RhoCalculator),
            (Cs01, cs01::Cs01Calculator),
            (BucketedCs01, cs01::BucketedCs01Calculator),
            (Dv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::parallel_combined())),
            // Theta is now registered universally in crate::FIXED_INCOME.metrics()
            (BucketedDv01, finstack_quant_valuations_core::metrics::UnifiedDv01Calculator::new(finstack_quant_valuations_core::metrics::Dv01CalculatorConfig::triangular_key_rate())),
            (Accrued, accrued_interest::AccruedInterestCalculator),
            (CleanPrice, accrued_interest::CleanPriceCalculator),
            (Dividend01, dividend_risk::DividendRiskCalculator),
            (Conversion01, conversion01::Conversion01Calculator),
            (Oas, oas::OasCalculator),
            (CrossGammaSpotVol, CrossFactorCalculator::new(make_spot_bumper, make_vol_bumper)),
            (CrossGammaSpotCredit, CrossFactorCalculator::new(make_spot_bumper, make_credit_bumper)),
            (CrossGammaRatesCredit, CrossFactorCalculator::new(make_rates_bumper, make_credit_bumper)),
            (CrossGammaCreditVol, CrossFactorCalculator::new(make_credit_bumper, make_vol_bumper)),
            (ImpliedVol, implied_vol::ImpliedVolCalculator),
        ],
        custom: [
            // Custom metrics (not in standard MetricId enum)
            ("parity", ParityCalculator),
            ("conversion_premium", ConversionPremiumCalculator),
            ("conversion_value", ConversionValueCalculator),
            ("bond_floor", BondFloorCalculator),
        ]
    }
    Ok(())
}

/// Bond floor (investment value) calculator for convertible bonds.
///
/// The bond floor is the present value of the convertible's cash flows (coupons
/// and principal redemption) discounted at the appropriate rate, ignoring the
/// conversion option entirely. It represents the "straight bond" value -- what
/// the instrument would be worth if it had no equity conversion feature.
///
/// The floor uses the main tree's survival-weighted debt payments plus explicit
/// recovery of outstanding principal, including PIK and prior amortization.
/// Payment-date corrections preserve contractual cashflow discounting.
///
/// # Use Cases
///
/// - Assessing downside protection (bond floor vs market price)
/// - Computing the "equity option value" = CB price - bond floor
/// - Monitoring busted convertibles (trading near bond floor)
pub(crate) struct BondFloorCalculator;

impl MetricCalculator for BondFloorCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;
        crate::convertible::pricing::price_bond_floor(bond, &context.curves, context.as_of)
    }
}

/// Calculator for conversion premium.
///
/// Computes conversion premium = bond_price / (spot * conversion_ratio) - 1.
pub(crate) struct ConversionPremiumCalculator;

impl MetricCalculator for ConversionPremiumCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        let bond_price = context.base_value.amount();
        bond.conversion_premium(&context.curves, bond_price)
    }
}

/// Conversion value calculator.
///
/// For standard policies: `conversion_ratio * spot`.
/// For `MandatoryVariable` (PERCS/DECS/ACES): variable delivery ratio based
/// on spot relative to upper/lower conversion prices.
///
/// Reports the absolute conversion value based on the current spot price and
/// conversion terms. Delegates to the canonical `compute_conversion_value`
/// function which handles all policies including `MandatoryVariable`
/// (PERCS/DECS/ACES).
pub(crate) struct ConversionValueCalculator;

impl MetricCalculator for ConversionValueCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond: &ConvertibleBond = context.instrument_as()?;

        let spot = bond.spot_value(context.curves.get_price(&bond.spot_id)?)?;

        crate::convertible::pricing::compute_conversion_value(bond, spot)
    }
}

/// Calculator for convertible bond parity.
///
/// Computes the parity ratio: equity conversion value divided by bond face value.
/// Leverages pricing helpers from `pricing`.
pub(crate) struct ParityCalculator;

impl MetricCalculator for ParityCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        let bond = context.instrument_as::<ConvertibleBond>()?;
        bond.parity(&context.curves)
    }
}
