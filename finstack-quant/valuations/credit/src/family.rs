//! The credit family: single-name CDS, CDS indices, index tranches and CDS
//! options.

use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::Instrument;
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

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations_test_support::date::date;
    use finstack_quant_valuations_test_support::discount_forward_curves::flat_discount_with_tenor;

    /// Price `instrument` through the credit registry adapter for `key` against
    /// a market holding only the `USD-OIS` discount curve (no hazard curve).
    fn price_without_hazard(
        instrument: &dyn Instrument,
        key: PricerKey,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let as_of = date(2024, 1, 2);
        let market =
            MarketContext::new().insert(flat_discount_with_tenor("USD-OIS", as_of, 0.03, 10.0));
        let registry = CREDIT.build_pricers().expect("credit pricers register");
        registry
            .get_pricer(key)
            .expect("credit pricer registered")
            .price_dyn(instrument, &market, as_of)
    }

    fn assert_missing_market_data(
        result: std::result::Result<ValuationResult, PricingError>,
        instrument_id: &str,
        model: ModelKey,
    ) {
        match result {
            Err(PricingError::MissingMarketData {
                missing_id,
                context,
            }) => {
                assert!(
                    missing_id.contains("CDX.NA.IG.HAZARD") || missing_id.contains("CORP-HAZARD"),
                    "missing id should name the hazard curve: {missing_id}"
                );
                assert_eq!(context.instrument_id.as_deref(), Some(instrument_id));
                assert_eq!(context.model, Some(model));
            }
            other => panic!("expected MissingMarketData for {instrument_id}, got {other:?}"),
        }
    }

    #[test]
    fn cds_index_adapter_reports_missing_hazard_curve_as_missing_market_data() {
        let index = crate::CdsIndex::example().expect("example index");
        let result = price_without_hazard(
            &index,
            PricerKey::new(InstrumentType::CdsIndex, ModelKey::HazardRate),
        );
        assert_missing_market_data(result, index.id(), ModelKey::HazardRate);
    }

    #[test]
    fn cds_option_adapter_reports_missing_hazard_curve_as_missing_market_data() {
        let mut option = crate::CdsOption::example().expect("example option");
        // Quote σ directly so the vol surface is not needed and the hazard
        // curve is the missing input.
        option
            .instrument_pricing_overrides
            .market_quotes
            .implied_volatility = Some(0.3);
        let result = price_without_hazard(
            &option,
            PricerKey::new(InstrumentType::CdsOption, ModelKey::BloombergCdso),
        );
        assert_missing_market_data(result, option.id(), ModelKey::BloombergCdso);
    }

    #[test]
    fn cds_tranche_adapter_reports_missing_hazard_curve_as_missing_market_data() {
        let tranche = crate::CdsTranche::example().expect("example tranche");
        let result = price_without_hazard(
            &tranche,
            PricerKey::new(InstrumentType::CdsTranche, ModelKey::HazardRate),
        );
        assert_missing_market_data(result, tranche.id(), ModelKey::HazardRate);
    }
}
