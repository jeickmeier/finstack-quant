//! Private metric enrichment for registry-dispatched valuation results.

use super::registry::attach_metric_measures;
use super::{ModelKey, PricerRegistry, PricingError, PricingErrorContext};
use crate::instruments::common_impl::helpers::{compute_metrics_dyn, MetricBuildOptions};
use crate::instruments::Instrument;
use crate::metrics::risk::MarketHistory;
use crate::metrics::MetricId;
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
        pricer_registry,
        mut base_result,
    } = request;
    let err_ctx = PricingErrorContext::from_instrument(instrument).model(model);
    let metric_registry = pricer_registry.metric_registry_override();

    if model == ModelKey::Discounting || !instrument.has_custom_metrics_equivalent() {
        let metric_measures = compute_metrics_dyn(
            Arc::from(instrument.clone_box()),
            market,
            as_of,
            base_result.value,
            metrics,
            MetricBuildOptions {
                cfg,
                market_history,
                metric_registry,
                pricing_model: Some(model),
                pricer_registry: Some(pricer_registry),
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
                cfg: cfg.clone(),
                market_history: market_history.clone(),
                metric_registry: metric_registry.clone(),
                ..MetricBuildOptions::default()
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
                cfg,
                market_history,
                metric_registry,
                pricing_model: Some(model),
                pricer_registry: Some(pricer_registry),
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
