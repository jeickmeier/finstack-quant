//! The credit family: single-name CDS, CDS indices, index tranches and CDS
//! options.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;
use finstack_quant_valuations_core::metrics::{MetricRegistry, MetricRegistryError};
use finstack_quant_valuations_core::pricer::{
    expect_inst, Family, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingError,
    PricingErrorContext,
};
use finstack_quant_valuations_core::results::{
    CreditDerivativeValuationDetails, ValuationDetails, ValuationResult,
};

/// Registration unit for the credit-derivative instruments.
pub static CREDIT: Family = Family::new("credit", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    registry.register(CdsHazardPricer)?;
    registry.register(crate::cds_index::pricer::SimpleCdsIndexHazardPricer)?;
    registry.register(crate::cds_tranche::pricing::SimpleCdsTrancheHazardPricer)?;
    registry.register(crate::cds_option::pricer::BloombergCdsoPricer)?;
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::cds::metrics::register_cds_metrics(registry)?;
    crate::cds_index::metrics::register_cds_index_metrics(registry)?;
    crate::cds_tranche::metrics::register_cds_tranche_metrics(registry)?;
    crate::cds_option::metrics::register_cds_option_metrics(registry)?;
    Ok(())
}

/// Registry adapter pricing a single-name CDS off its hazard curve.
struct CdsHazardPricer;

impl Pricer for CdsHazardPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(InstrumentType::Cds, ModelKey::HazardRate)
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let cds = expect_inst::<crate::CreditDefaultSwap>(instrument, InstrumentType::Cds)?;
        let value = cds.base_value(market, as_of).map_err(|e| {
            PricingError::from_core(
                e,
                PricingErrorContext::from_instrument(cds).model(ModelKey::HazardRate),
            )
        })?;

        Ok(
            ValuationResult::stamped(cds.id(), as_of, value).with_details(
                ValuationDetails::CreditDerivative(credit_derivative_details(
                    ModelKey::HazardRate,
                    Some("isda_standard_model"),
                )),
            ),
        )
    }
}

fn credit_derivative_details(
    model_key: ModelKey,
    integration_method: Option<&str>,
) -> CreditDerivativeValuationDetails {
    CreditDerivativeValuationDetails {
        model_key,
        integration_method: integration_method.map(str::to_string),
    }
}
