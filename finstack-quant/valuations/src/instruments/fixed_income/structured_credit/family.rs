//! The structured-credit family: deal-level securitisations and their
//! tranche views.

use std::sync::Arc;

use indexmap::IndexMap;

use super::{StochasticPricingResult, StructuredCredit};
use crate::instruments::common_impl::traits::Instrument;
use crate::metrics::{MetricId, MetricRegistry, MetricRegistryError, PricerPublishedMetric};
use crate::pricer::{
    expect_inst, register_generic, Family, InstrumentType, ModelKey, Pricer, PricerKey,
    PricerRegistry, PricingError, PricingErrorContext,
};
use crate::results::{ValuationDetails, ValuationResult};
use finstack_quant_core::market_data::context::MarketContext;

/// Registration unit for the structured-credit instruments.
pub static STRUCTURED_CREDIT: Family =
    Family::new("structured_credit", register_pricers, register_metrics);

fn register_pricers(registry: &mut PricerRegistry) -> Result<(), PricingError> {
    register_generic!(registry, InstrumentType::StructuredCredit, StructuredCredit);
    registry.register(StructuredCreditStochasticPricer)?;
    Ok(())
}

fn register_metrics(registry: &mut MetricRegistry) -> Result<(), MetricRegistryError> {
    super::metrics::register_structured_credit_metrics(registry)?;
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
        let stochastic = structured_credit
            .price_stochastic_base(market, as_of)
            .map_err(|e| {
                PricingError::from_core(
                    e,
                    PricingErrorContext::from_instrument(structured_credit)
                        .model(ModelKey::StructuredCreditStochastic),
                )
            })?;

        let measures = stochastic_summary_measures(&stochastic);
        Ok(
            ValuationResult::stamped(structured_credit.id(), as_of, stochastic.npv)
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
