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
    pub(super) market: &'a MarketContext,
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
    // The pricer's seed serves this request's metric evaluation only; the
    // returned result does not carry it.
    let pricer_seed = base_result.metric_seed.take();

    let leg_options = crate::instruments::PricingOptions {
        config: cfg.clone(),
        market_history: market_history.clone(),
        model: None,
        registry: Some(Arc::clone(&pricer_registry)),
        metric_registry: metric_registry.clone(),
        recalibration_provider: recalibration_provider.clone(),
        explain: finstack_quant_core::explain::ExplainOpts::disabled(),
        instrument_validated: false,
    };
    if let Some((metric_measures, details)) = instrument
        .composite_valuation(market, as_of, metrics, leg_options)
        .map_err(|error| PricingError::from_core(error, err_ctx.clone()))?
    {
        base_result.details = Some(details);
        attach_metric_measures(&mut base_result, metric_measures);
        return Ok(base_result);
    }

    if metrics.is_empty() {
        return Ok(base_result);
    }
    let market = Arc::new(market.clone());

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
                pricer_measures: base_result.measures.clone(),
                pricer_seed: pricer_seed.clone(),
            },
        )
        .map_err(|error| PricingError::from_core(error, err_ctx.clone()))?;
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
                pricer_measures: base_result.measures.clone(),
                // The seed belongs to the priced instrument, not to its
                // metrics equivalent.
                pricer_seed: None,
            },
        )
        .map_err(|error| PricingError::from_core(error, err_ctx.clone()))?
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
                pricer_measures: base_result.measures.clone(),
                pricer_seed: pricer_seed.clone(),
            },
        )
        .map_err(|error| PricingError::from_core(error, err_ctx.clone()))?;
        for (key, value) in risk_measures {
            metric_measures.insert(key, value);
        }
    }

    attach_metric_measures(&mut base_result, metric_measures);
    Ok(base_result)
}
