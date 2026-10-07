//! Registry pricer adapter for CDS tranches.
//!
use super::config::CdsTranchePricer;
use crate::instruments::common_impl::traits::Instrument;
use crate::pricer::expect_inst;
use finstack_quant_core::market_data::context::MarketContext;

/// Registry pricer for CDS Tranche using Gaussian Copula model
pub(crate) struct SimpleCdsTrancheHazardPricer;

impl crate::pricer::Pricer for SimpleCdsTrancheHazardPricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::CdsTranche,
            crate::pricer::ModelKey::HazardRate,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, crate::pricer::PricingError> {
        use crate::instruments::common_impl::traits::Instrument;

        // Type-safe downcasting
        let cds_tranche = expect_inst::<
            crate::instruments::credit_derivatives::cds_tranche::CdsTranche,
        >(instrument, crate::pricer::InstrumentType::CdsTranche)?;

        // Use the provided as_of date for valuation
        // Compute present value using the engine
        let pv = CdsTranchePricer::new()
            .price_tranche(cds_tranche, market, as_of)
            .map_err(|e| {
                crate::pricer::PricingError::model_failure_with_context(
                    e.to_string(),
                    crate::pricer::PricingErrorContext::default(),
                )
            })?;

        Ok(
            crate::results::ValuationResult::stamped(cds_tranche.id(), as_of, pv).with_details(
                crate::results::ValuationDetails::CreditDerivative(
                    crate::results::CreditDerivativeValuationDetails {
                        model_key: crate::pricer::ModelKey::HazardRate,
                        integration_method: Some("isda_standard_model".to_string()),
                    },
                ),
            ),
        )
    }
}
