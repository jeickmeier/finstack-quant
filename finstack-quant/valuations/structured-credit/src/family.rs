//! The structured-credit family: deal-level securitisations and their
//! tranche views.

use std::sync::Arc;

use indexmap::IndexMap;

use crate::structured_credit::{StochasticPricingResult, StructuredCredit};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::instruments::Instrument;
use finstack_quant_valuations_core::metrics::{
    MetricId, MetricRegistry, MetricRegistryError, PricerPublishedMetric,
};
use finstack_quant_valuations_core::pricer::{
    expect_inst, Family, InstrumentType, ModelKey, Pricer, PricerKey, PricerRegistry, PricingError,
    PricingErrorContext,
};
use finstack_quant_valuations_core::register_generic;
use finstack_quant_valuations_core::results::{ValuationDetails, ValuationResult};

/// Registration unit for the structured-credit instruments.
pub static STRUCTURED_CREDIT: Family =
    Family::new("structured_credit", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(registry, InstrumentType::StructuredCredit, StructuredCredit);
    registry.register(StructuredCreditStochasticPricer)?;
    // CMOs and asset-backed facilities stay in the umbrella with structured
    // credit until the structured-credit crate exists; they register here.
    register_generic!(registry, InstrumentType::AgencyCmo, crate::cmo::AgencyCmo);
    register_generic!(
        registry,
        InstrumentType::AssetBackedFacility,
        crate::asset_backed_facility::AssetBackedFacility
    );
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    crate::structured_credit::metrics::register_structured_credit_metrics(registry)?;
    crate::cmo::metrics::register_cmo_metrics(registry)?;
    crate::asset_backed_facility::metrics::register_asset_backed_facility_metrics(registry)?;
    // `StructuredCreditStochasticPricer` publishes `expected_loss` and
    // `expected_shortfall` from the deal Monte Carlo pass. The entries record
    // that ownership: `expected_shortfall` has a universal historical-
    // simulation owner, so the instrument-specific entry points at the
    // published value instead.
    registry.register_metric(
        MetricId::ExpectedLoss,
        Arc::new(PricerPublishedMetric::new(MetricId::ExpectedLoss)),
        &[InstrumentType::StructuredCredit],
    )?;
    registry.register_metric(
        MetricId::ExpectedShortfall,
        Arc::new(PricerPublishedMetric::new(MetricId::ExpectedShortfall)),
        &[InstrumentType::StructuredCredit],
    )?;
    Ok(())
}

/// Registry adapter for the deal Monte Carlo engine.
struct StructuredCreditStochasticPricer;

impl Pricer for StructuredCreditStochasticPricer {
    fn key(&self) -> PricerKey {
        PricerKey::new(
            InstrumentType::StructuredCredit,
            ModelKey::StructuredCreditStochastic,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<ValuationResult, PricingError> {
        let structured_credit =
            expect_inst::<StructuredCredit>(instrument, InstrumentType::StructuredCredit)?;
        let to_pricing_error = |e: finstack_quant_core::Error| {
            PricingError::from_core(
                e,
                PricingErrorContext::from_instrument(structured_credit)
                    .model(ModelKey::StructuredCreditStochastic),
            )
        };
        let stochastic = structured_credit
            .price_stochastic_base(market, as_of)
            .map_err(to_pricing_error)?;
        // The registry applies the scenario price shock to the headline value
        // at the valuation boundary, so the headline stays unshocked here. The
        // detail payload and its summary measures are shocked once, the same
        // way `price_stochastic_with_mode` shocks them, so they agree with the
        // published value.
        let unshocked_npv = stochastic.npv;
        let stochastic = structured_credit
            .apply_stochastic_price_scenario(stochastic)
            .map_err(to_pricing_error)?;

        let measures = stochastic_summary_measures(&stochastic);
        Ok(
            ValuationResult::stamped(structured_credit.id(), as_of, unshocked_npv)
                .with_measures(measures)
                .with_details(ValuationDetails::StructuredCreditStochastic(stochastic)),
        )
    }
}

fn stochastic_summary_measures(result: &StochasticPricingResult) -> IndexMap<MetricId, f64> {
    let mut measures = IndexMap::new();
    measures.insert(
        MetricId::custom("expected_loss"),
        result.expected_loss.amount(),
    );
    measures.insert(
        MetricId::custom("unexpected_loss"),
        result.unexpected_loss.amount(),
    );
    measures.insert(
        MetricId::custom("expected_shortfall"),
        result.expected_shortfall.amount(),
    );
    measures.insert(MetricId::custom("pv_std_error"), result.pv_std_error);

    for tranche in &result.tranche_results {
        measures.insert(
            MetricId::composite(
                &MetricId::custom("tranche_npv"),
                &[tranche.tranche_id.as_str()],
            ),
            tranche.npv.amount(),
        );
        measures.insert(
            MetricId::composite(
                &MetricId::custom("tranche_expected_loss"),
                &[tranche.tranche_id.as_str()],
            ),
            tranche.expected_loss.amount(),
        );
        measures.insert(
            MetricId::composite(
                &MetricId::custom("tranche_wal"),
                &[tranche.tranche_id.as_str()],
            ),
            tranche.wal,
        );
    }

    measures
}
