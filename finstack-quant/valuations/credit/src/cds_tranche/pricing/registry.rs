//! Registry pricer adapter for CDS tranches.
//!
use super::config::CdsTranchePricer;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;
use finstack_quant_valuations_core::pricer::expect_inst;

/// Registry pricer for CDS Tranche using Gaussian Copula model
pub(crate) struct SimpleCdsTrancheHazardPricer;

impl finstack_quant_valuations_core::pricer::Pricer for SimpleCdsTrancheHazardPricer {
    fn key(&self) -> finstack_quant_valuations_core::pricer::PricerKey {
        finstack_quant_valuations_core::pricer::PricerKey::new(
            finstack_quant_valuations_core::pricer::InstrumentType::CdsTranche,
            finstack_quant_valuations_core::pricer::ModelKey::HazardRate,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<
        finstack_quant_valuations_core::results::ValuationResult,
        finstack_quant_valuations_core::pricer::PricingError,
    > {
        use finstack_quant_valuations_core::instruments::common_impl::traits::Instrument;

        // Type-safe downcasting
        let cds_tranche = expect_inst::<crate::cds_tranche::CdsTranche>(
            instrument,
            finstack_quant_valuations_core::pricer::InstrumentType::CdsTranche,
        )?;

        // Use the provided as_of date for valuation
        // Compute present value using the engine
        let pv = CdsTranchePricer::new()
            .price_tranche(cds_tranche, market, as_of)
            .map_err(|e| {
                finstack_quant_valuations_core::pricer::PricingError::model_failure_with_context(
                    e.to_string(),
                    finstack_quant_valuations_core::pricer::PricingErrorContext::default(),
                )
            })?;

        Ok(
            finstack_quant_valuations_core::results::ValuationResult::stamped(
                cds_tranche.id(),
                as_of,
                pv,
            )
            .with_details(
                finstack_quant_valuations_core::results::ValuationDetails::CreditDerivative(
                    finstack_quant_valuations_core::results::CreditDerivativeValuationDetails {
                        model_key: finstack_quant_valuations_core::pricer::ModelKey::HazardRate,
                        integration_method: Some("isda_standard_model".to_string()),
                    },
                ),
            ),
        )
    }
}
