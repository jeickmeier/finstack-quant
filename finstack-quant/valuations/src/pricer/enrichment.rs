//! Private metric enrichment for registry-dispatched valuation results.

use super::registry::attach_metric_measures;
use super::{ModelKey, PricerRegistry, PricingError, PricingErrorContext};
use crate::instruments::common_impl::helpers::{compute_metrics_dyn, MetricBuildOptions};
use crate::instruments::Instrument;
use crate::metrics::risk::MarketHistory;
use crate::metrics::{MetricId, MetricRegistry};
use crate::results::ValuationResult;
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use std::sync::Arc;

pub(super) struct EnrichmentRequest<'a> {
    pub(super) instrument: &'a dyn Instrument,
    pub(super) model: ModelKey,
    pub(super) market: Arc<MarketContext>,
    pub(super) as_of: Date,
    pub(super) metrics: &'a [MetricId],
    pub(super) cfg: Option<Arc<FinstackConfig>>,
    pub(super) market_history: Option<Arc<MarketHistory>>,
    pub(super) metric_registry: Option<Arc<MetricRegistry>>,
    pub(super) recalibration_provider: Option<Arc<dyn crate::recalibration::RecalibrationProvider>>,
    pub(super) pricer_registry: Arc<PricerRegistry>,
    pub(super) base_result: ValuationResult,
}

pub(super) fn enrich(
    request: EnrichmentRequest<'_>,
) -> std::result::Result<ValuationResult, PricingError> {
    let EnrichmentRequest {
        instrument,
        model,
        market,
        as_of,
        metrics,
        cfg,
        market_history,
        metric_registry,
        recalibration_provider,
        pricer_registry,
        mut base_result,
    } = request;
    let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);

    if let Some(composite) = instrument
        .as_any()
        .downcast_ref::<crate::instruments::CompositeInstrument>()
    {
        let options = crate::instruments::PricingOptions {
            config: cfg,
            market_history,
            model: None,
            registry: Some(Arc::clone(&pricer_registry)),
            metric_registry,
            recalibration_provider,
            instrument_validated: false,
        };
        let (metric_measures, details) = composite
            .valuation_details_with_metrics(market.as_ref(), as_of, metrics, options)
            .map_err(|error| {
                PricingError::model_failure_with_context(error.to_string(), err_ctx.clone())
            })?;
        base_result.details = Some(crate::results::ValuationDetails::Composite(details));
        attach_metric_measures(&mut base_result, metric_measures);
        return Ok(base_result);
    }

    if model == ModelKey::Discounting || !instrument.has_custom_metrics_equivalent() {
        let metric_measures = compute_metrics_dyn(
            Arc::from(instrument.clone_box()),
            market,
            as_of,
            base_result.value,
            metrics,
            MetricBuildOptions {
                pricing: crate::instruments::PricingOptions {
                    config: cfg,
                    market_history,
                    recalibration_provider,
                    metric_registry,
                    ..crate::instruments::PricingOptions::default()
                },
                pricing_dispatch: crate::pricer::PricingDispatch::registered(
                    model,
                    pricer_registry,
                ),
            },
        )
        .map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), err_ctx.clone())
        })?;
        attach_metric_measures(&mut base_result, metric_measures);
        return Ok(base_result);
    }

    let (spread_metrics, risk_metrics): (Vec<_>, Vec<_>) = metrics
        .iter()
        .cloned()
        .partition(|metric| MetricId::SPREAD_EQUIVALENT_METRICS.contains(metric));

    let mut metric_measures = if spread_metrics.is_empty() {
        indexmap::IndexMap::new()
    } else {
        compute_metrics_dyn(
            Arc::from(instrument.metrics_equivalent()),
            Arc::clone(&market),
            as_of,
            base_result.value,
            &spread_metrics,
            MetricBuildOptions {
                pricing: crate::instruments::PricingOptions {
                    config: cfg.clone(),
                    market_history: market_history.clone(),
                    recalibration_provider: recalibration_provider.clone(),
                    metric_registry: metric_registry.clone(),
                    ..crate::instruments::PricingOptions::default()
                },
                pricing_dispatch: crate::pricer::PricingDispatch::registered(
                    model,
                    Arc::clone(&pricer_registry),
                ),
            },
        )
        .map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), err_ctx.clone())
        })?
    };

    if !risk_metrics.is_empty() {
        let risk_measures = compute_metrics_dyn(
            Arc::from(instrument.clone_box()),
            market,
            as_of,
            base_result.value,
            &risk_metrics,
            MetricBuildOptions {
                pricing: crate::instruments::PricingOptions {
                    config: cfg,
                    market_history,
                    recalibration_provider,
                    metric_registry,
                    ..crate::instruments::PricingOptions::default()
                },
                pricing_dispatch: crate::pricer::PricingDispatch::registered(
                    model,
                    pricer_registry,
                ),
            },
        )
        .map_err(|error| {
            PricingError::model_failure_with_context(error.to_string(), err_ctx.clone())
        })?;
        for (key, value) in risk_measures {
            metric_measures.insert(key, value);
        }
    }

    attach_metric_measures(&mut base_result, metric_measures);
    Ok(base_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::Instrument;
    use crate::instruments::Bond;
    use crate::metrics::{MetricCalculator, MetricContext, MetricRegistry};
    use crate::pricer::{InstrumentType, Pricer, PricerKey};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::StubKind;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::Rate;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use time::macros::date;

    struct FixedHazardBondPricer {
        calls: Arc<AtomicUsize>,
    }

    impl Pricer for FixedHazardBondPricer {
        fn key(&self) -> PricerKey {
            PricerKey::new(InstrumentType::Bond, ModelKey::HazardRate)
        }

        fn price_dyn(
            &self,
            instrument: &dyn Instrument,
            _market: &MarketContext,
            as_of: Date,
        ) -> std::result::Result<ValuationResult, PricingError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ValuationResult::stamped(
                instrument.id(),
                as_of,
                Money::new(321.0, Currency::USD),
            ))
        }
    }

    struct RepriceSpreadMetric;

    impl MetricCalculator for RepriceSpreadMetric {
        fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            context.reprice_raw(context.curves.as_ref(), context.as_of)
        }
    }

    #[test]
    fn spread_equivalent_metric_preserves_selected_custom_registry() {
        let as_of = date!(2025 - 01 - 15);
        let bond = Bond::fixed(
            "SPREAD-DISPATCH",
            Money::new(1_000.0, Currency::USD),
            Rate::from_decimal(0.04),
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            StubKind::ShortFront,
            "USD-OIS",
        )
        .expect("valid bond");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut pricers = PricerRegistry::new();
        pricers
            .register(FixedHazardBondPricer {
                calls: Arc::clone(&calls),
            })
            .expect("unique custom pricer");
        let mut metrics = MetricRegistry::new();
        metrics
            .register_metric(
                MetricId::ZSpread,
                Arc::new(RepriceSpreadMetric),
                &[InstrumentType::Bond],
            )
            .expect("unique custom metric");

        let result = pricers
            .price_with_metrics(
                &bond,
                ModelKey::HazardRate,
                &MarketContext::new(),
                as_of,
                &[MetricId::ZSpread],
                crate::instruments::PricingOptions::default()
                    .with_metric_registry(Arc::new(metrics)),
            )
            .expect("spread metric must reprice through the selected registry");

        assert_eq!(result.value.amount(), 321.0);
        assert_eq!(result.measures.get(&MetricId::ZSpread), Some(&321.0));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "base PV and spread-metric reprice must use the selected pricer"
        );
    }
}
