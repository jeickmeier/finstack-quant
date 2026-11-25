//! Metric registration and calculators.

use crate::real_estate::{RealEstateAsset, pricer};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::metrics::sensitivities::{
    RfComponentDv01Calculator, RfComponentPriced,
};
use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricRegistryError};
use finstack_quant_valuations_core::pricer::InstrumentType;

impl RfComponentPriced for RealEstateAsset {
    fn pv_with_rf_bump(
        &self,
        _market: &MarketContext,
        as_of: Date,
        bump: f64,
    ) -> finstack_quant_core::Result<f64> {
        pricer::pv_with_rf_bump(self, as_of, bump)
    }
}

/// Multiple on invested capital: total inflows over total outflows (absolute).
///
/// # Arguments
///
/// * `flows` - Signed cashflow amounts (positive inflows, negative outflows).
/// * `metric` - Metric name included in validation error messages.
///
/// # Errors
///
/// Returns `Error::Validation` naming `metric` when there are no outflows.
pub fn moic(
    flows: impl IntoIterator<Item = f64>,
    metric: &str,
) -> finstack_quant_core::Result<f64> {
    let mut inflows = 0.0;
    let mut outflows = 0.0;
    for a in flows {
        if a >= 0.0 {
            inflows += a;
        } else {
            outflows += -a;
        }
    }
    if outflows <= 0.0 {
        return Err(finstack_quant_core::Error::Validation(format!(
            "{metric}: total outflows must be positive"
        )));
    }
    Ok(inflows / outflows)
}

/// Register real estate asset metrics with the registry.
pub(crate) fn register_real_estate_metrics(
    registry: &mut MetricRegistry,
) -> std::result::Result<(), MetricRegistryError> {
    finstack_quant_valuations_core::register_metrics! {
        registry: registry,
        instrument: InstrumentType::RealEstateAsset,
        metrics: [],
        custom: [
            ("real_estate::discount_rate01", RfComponentDv01Calculator::<RealEstateAsset>::new()),
            // Custom real estate deal-style metrics.
            ("real_estate::going_in_cap_rate", super::cap_rates::GoingInCapRate),
            ("real_estate::exit_cap_rate", super::cap_rates::ExitCapRate),
            ("real_estate::unlevered_irr", super::returns::UnleveredIrr),
            ("real_estate::unlevered_multiple", super::returns::UnleveredMultiple),
            ("real_estate::unlevered_cash_on_cash_first", super::returns::UnleveredCashOnCashFirst),
            ("real_estate::cap_rate01", super::sensitivities::CapRate01Calculator),
        ]
    }

    Ok(())
}
