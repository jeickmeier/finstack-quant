//! Closed-world assembly of built-in instrument families into [`STANDARD`].

use super::{Family, InstrumentType, PricerRegistry, PricingError};
use crate::metrics::{MetricRegistry, MetricRegistryError};
use crate::register_generic;
use finstack_quant_valuations_credit::CREDIT;
use finstack_quant_valuations_equity::EQUITY;
use finstack_quant_valuations_exotics::EXOTICS;
use finstack_quant_valuations_fixed_income::FIXED_INCOME;
use finstack_quant_valuations_fx::FX;
use finstack_quant_valuations_rates::RATES;
use finstack_quant_valuations_structured_credit::STRUCTURED_CREDIT;

/// The built-in instrument families, each owning a disjoint set of
/// instrument types.
///
/// The order is the order [`STANDARD`] registers them in; it has no effect on
/// lookups.
pub static FAMILIES: [&Family; 7] = [
    &RATES,
    &FIXED_INCOME,
    &STRUCTURED_CREDIT,
    &CREDIT,
    &EQUITY,
    &FX,
    &EXOTICS,
];

/// The umbrella family: every built-in family merged, plus the composite
/// instruments.
///
/// `Composite` and `Basket` hold legs of any family, and levered real estate
/// holds financing legs from several families, so they belong here rather
/// than to one asset class. The JSON entry points and registry
/// introspection (`list_models`, `list_standard_metrics`) read this family's
/// registries.
pub static STANDARD: Family = Family::new(
    "standard",
    register_standard_pricers,
    register_standard_metrics,
);

fn register_standard_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    for family in FAMILIES {
        family.register_pricers(registry)?;
    }
    register_generic!(
        registry,
        InstrumentType::Composite,
        crate::instruments::composite::CompositeInstrument
    );
    register_generic!(
        registry,
        InstrumentType::Basket,
        crate::instruments::basket::Basket
    );
    // Levered real estate holds financing legs from several families, so it
    // registers here rather than with `EQUITY` (unlevered `RealEstateAsset`
    // registers with `EQUITY`).
    register_generic!(
        registry,
        InstrumentType::LeveredRealEstateEquity,
        crate::instruments::levered_real_estate::LeveredRealEstateEquity
    );
    Ok(())
}

fn register_standard_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    for family in FAMILIES {
        family.register_metrics(registry)?;
    }
    crate::instruments::basket::metrics::register_basket_metrics(registry)?;
    crate::instruments::levered_real_estate::metrics::register_levered_real_estate_metrics(
        registry,
    )?;
    crate::metrics::risk::register_risk_metrics(registry)
}
