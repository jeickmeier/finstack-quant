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
        explain: finstack_quant_core::explain::ExplainOpts::default(),
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

    let compute = |instrument, metrics: &[MetricId], pricer_seed| {
        compute_metrics_dyn(
            instrument,
            Arc::clone(&market),
            as_of,
            base_result.value,
            metrics,
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
                pricer_seed,
            },
        )
        .map_err(|error| PricingError::from_core(error, err_ctx.clone()))
    };

    let equivalent = (model != ModelKey::Discounting
        && metrics
            .iter()
            .any(|metric| MetricId::SPREAD_EQUIVALENT_METRICS.contains(metric)))
    .then(|| instrument.metrics_equivalent())
    .flatten();

    let metric_measures = if let Some(equivalent) = equivalent {
        let (spread_metrics, risk_metrics): (Vec<_>, Vec<_>) = metrics
            .iter()
            .cloned()
            .partition(|metric| MetricId::SPREAD_EQUIVALENT_METRICS.contains(metric));
        // A native pricer seed cannot be installed into a normalized equivalent.
        let mut measures = compute(Arc::from(equivalent), &spread_metrics, None)?;
        if !risk_metrics.is_empty() {
            let risk_measures = compute(
                Arc::from(instrument.clone_box()),
                &risk_metrics,
                pricer_seed,
            )?;
            for (key, value) in risk_measures {
                measures.insert(key, value);
            }
        }
        measures
    } else {
        compute(Arc::from(instrument.clone_box()), metrics, pricer_seed)?
    };

    attach_metric_measures(&mut base_result, metric_measures);
    Ok(base_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::MarketDependencies;
    use crate::metrics::{MetricCalculator, MetricContext, MetricSeed};
    use crate::pricer::InstrumentType;
    use finstack_quant_core::{currency::Currency, money::Money, types::Attributes};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use time::macros::date;

    #[derive(Clone, Default)]
    struct Probe {
        attrs: Attributes,
        clones: Arc<AtomicUsize>,
        hooks: Arc<AtomicUsize>,
        visits: Arc<std::sync::Mutex<Vec<(bool, bool)>>>,
        normalizes: bool,
        equivalent: bool,
    }

    crate::impl_empty_cashflow_provider!(
        Probe,
        finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for Probe {
        fn id(&self) -> &str {
            "probe"
        }
        fn key(&self) -> InstrumentType {
            InstrumentType::Bond
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn attributes(&self) -> &Attributes {
            &self.attrs
        }
        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attrs
        }
        fn market_dependencies(&self) -> finstack_quant_core::Result<MarketDependencies> {
            Ok(MarketDependencies::new())
        }
        fn base_value(&self, _: &MarketContext, _: Date) -> finstack_quant_core::Result<Money> {
            Money::new(100.0, Currency::USD)
        }
        fn clone_box(&self) -> Box<dyn Instrument> {
            self.clones.fetch_add(1, Ordering::SeqCst);
            Box::new(self.clone())
        }
        fn metrics_equivalent(&self) -> Option<Box<dyn Instrument>> {
            self.hooks.fetch_add(1, Ordering::SeqCst);
            self.normalizes.then(|| {
                let mut equivalent = self.clone();
                equivalent.equivalent = true;
                Box::new(equivalent) as Box<dyn Instrument>
            })
        }
        fn seed_metric_context(&self, context: &mut MetricContext, _: &MarketContext, _: Date) {
            self.visits
                .lock()
                .expect("visits")
                .push((self.equivalent, context.ext::<NativeSeed>().is_some()));
        }
    }

    #[derive(Debug)]
    struct NativeSeed;
    impl MetricSeed for NativeSeed {
        fn seed(&self, context: &mut MetricContext) {
            context.set_ext(NativeSeed);
        }
    }

    struct BasisMetric;
    impl MetricCalculator for BasisMetric {
        fn calculate(&self, context: &mut MetricContext) -> finstack_quant_core::Result<f64> {
            let instrument = context
                .instrument
                .as_any()
                .downcast_ref::<Probe>()
                .expect("probe");
            assert_eq!(context.base_value.amount(), 123.0);
            Ok(if instrument.equivalent { 1.0 } else { 2.0 })
        }
    }

    fn run(probe: &Probe, model: ModelKey, metrics: &[MetricId]) -> ValuationResult {
        let mut registry = MetricRegistry::new();
        for metric in [MetricId::ZSpread, MetricId::Dv01, MetricId::ExpectedLoss] {
            registry
                .register_metric(metric, Arc::new(BasisMetric), &[])
                .expect("register");
        }
        let as_of = date!(2026 - 01 - 01);
        let mut base_result = ValuationResult::stamped(
            "probe",
            as_of,
            Money::new(123.0, Currency::USD).expect("money"),
        )
        .with_metric_seed(Arc::new(NativeSeed));
        base_result.measures.insert(MetricId::ExpectedLoss, 42.0);
        let original_meta = serde_json::to_value(&base_result.meta).expect("metadata");
        let result = enrich(EnrichmentRequest {
            instrument: probe,
            model,
            market: &MarketContext::new(),
            as_of,
            metrics,
            cfg: None,
            market_history: None,
            metric_registry: Some(Arc::new(registry)),
            recalibration_provider: None,
            pricer_registry: Arc::new(PricerRegistry::new()),
            base_result,
        })
        .expect("enrichment");
        assert!(result.metric_seed.is_none());
        assert_eq!(result.value.amount(), 123.0);
        assert_eq!(
            serde_json::to_value(&result.meta).expect("metadata"),
            original_meta
        );
        assert_eq!(result.metric(MetricId::ExpectedLoss), Some(42.0));
        result
    }

    #[test]
    fn equivalent_hook_is_only_resolved_for_non_discounting_spread_requests() {
        for (model, metrics, hooks, clones, visits) in [
            (ModelKey::HazardRate, vec![], 0, 0, vec![]),
            (
                ModelKey::Discounting,
                vec![MetricId::ZSpread],
                0,
                1,
                vec![(false, true)],
            ),
            (
                ModelKey::HazardRate,
                vec![MetricId::Dv01],
                0,
                1,
                vec![(false, true)],
            ),
            (
                ModelKey::HazardRate,
                vec![MetricId::ZSpread],
                1,
                0,
                vec![(true, false)],
            ),
        ] {
            let probe = Probe {
                normalizes: true,
                ..Probe::default()
            };
            run(&probe, model, &metrics);
            assert_eq!(probe.hooks.load(Ordering::SeqCst), hooks);
            assert_eq!(probe.clones.load(Ordering::SeqCst), clones);
            assert_eq!(*probe.visits.lock().expect("visits"), visits);
        }
    }

    #[test]
    fn absent_equivalent_uses_one_native_pass_and_clone() {
        let probe = Probe::default();
        let result = run(
            &probe,
            ModelKey::HazardRate,
            &[MetricId::ZSpread, MetricId::Dv01],
        );
        assert_eq!(probe.hooks.load(Ordering::SeqCst), 1);
        assert_eq!(probe.clones.load(Ordering::SeqCst), 1);
        assert_eq!(*probe.visits.lock().expect("visits"), vec![(false, true)]);
        assert_eq!(result.metric(MetricId::ZSpread), Some(2.0));
        assert_eq!(result.metric(MetricId::Dv01), Some(2.0));
    }

    #[test]
    fn split_metrics_keep_native_seed_and_spread_first_merge_order() {
        let probe = Probe {
            normalizes: true,
            ..Probe::default()
        };
        let result = run(
            &probe,
            ModelKey::HazardRate,
            &[MetricId::Dv01, MetricId::ZSpread, MetricId::ExpectedLoss],
        );
        assert_eq!(
            *probe.visits.lock().expect("visits"),
            vec![(true, false), (false, true)]
        );
        assert_eq!(probe.clones.load(Ordering::SeqCst), 1);
        assert_eq!(result.metric(MetricId::ZSpread), Some(1.0));
        assert_eq!(result.metric(MetricId::Dv01), Some(2.0));
        assert_eq!(
            result.measures.keys().cloned().collect::<Vec<_>>(),
            vec![MetricId::ZSpread, MetricId::Dv01, MetricId::ExpectedLoss]
        );
    }
}
