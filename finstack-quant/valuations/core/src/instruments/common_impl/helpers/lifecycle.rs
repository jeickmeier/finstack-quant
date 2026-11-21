//! Validated pricing lifecycle, scenario application, and metric context dispatch.

use crate::instruments::PricingOptions;
use crate::metrics::{MetricContext, MetricId};
use crate::pricer::PricingDispatch;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use indexmap::IndexMap;
use std::sync::Arc;

/// Publish a Monte Carlo estimate's standard error, 95% confidence bounds and
/// path counts as `mc_*` custom measures on `result`.
///
/// # Arguments
///
/// * `result` - Target valuation result whose `measures` map receives the `mc_*` keys.
/// * `estimate` - Monte Carlo estimate carrying standard error, confidence bounds, and path counts.
pub fn attach_mc_diagnostics(
    result: &mut crate::results::ValuationResult,
    estimate: &finstack_quant_models::monte_carlo::results::MoneyEstimate,
) {
    for (id, value) in [
        ("mc_stderr", estimate.stderr),
        ("mc_ci_95_low", estimate.ci_95.0.amount()),
        ("mc_ci_95_high", estimate.ci_95.1.amount()),
        ("mc_num_paths", estimate.num_paths as f64),
        (
            "mc_num_simulated_paths",
            estimate.num_simulated_paths as f64,
        ),
        ("mc_relative_stderr", estimate.relative_stderr()),
    ] {
        result.measures.insert(MetricId::custom(id), value);
    }
}

/// Validated pricing boundary shared by direct and registry-backed routes.
///
/// Construction performs the only mandatory invariant/override validation
/// step. Callers may then resolve the effective date, run their chosen model
/// kernel, and apply the instrument-owned scenario adjustment exactly once.
pub struct ValidatedPricingLifecycle<'a, I>
where
    I: crate::instruments::Instrument + ?Sized,
{
    instrument: &'a I,
}

impl<'a, I> ValidatedPricingLifecycle<'a, I>
where
    I: crate::instruments::Instrument + ?Sized,
{
    /// Validate the complete instrument pricing boundary.
    ///
    /// # Arguments
    ///
    /// * `instrument` - Instrument whose invariants and pricing overrides are validated.
    pub fn new(instrument: &'a I) -> finstack_quant_core::Result<Self> {
        instrument.validate_for_pricing()?;
        Ok(Self { instrument })
    }

    /// Resolve the instrument's effective valuation date after validation.
    ///
    /// # Arguments
    ///
    /// * `market` - Market context used when an instrument anchors valuation to curve state.
    /// * `requested` - Caller-supplied valuation date before instrument-specific overrides.
    pub fn effective_as_of(&self, market: &MarketContext, requested: Date) -> Date {
        self.instrument.resolve_pricing_as_of(market, requested)
    }

    /// Apply the instrument scenario to a model `Money` result exactly once.
    ///
    /// # Arguments
    ///
    /// * `base_value` - Unshocked model present value in instrument currency.
    pub fn apply_value(&self, base_value: Money) -> finstack_quant_core::Result<Money> {
        apply_scenario_value(self.instrument, base_value)
    }

    /// Apply the instrument scenario to a raw model result exactly once.
    ///
    /// # Arguments
    ///
    /// * `base_value` - Unshocked raw scalar model present value.
    pub fn apply_raw_value(&self, base_value: f64) -> f64 {
        apply_scenario_raw_value(self.instrument, base_value)
    }
}

/// Apply an instrument's scenario price adjustment to a base present value.
///
/// Registry, direct-value, and metric assembly paths all route through this
/// helper so the adjustment remains an exactly-once lifecycle step.
///
/// # Arguments
///
/// * `instrument` - Instrument potentially carrying `ScenarioPricingOverrides`.
/// * `base_value` - Unshocked model present value to adjust.
#[inline]
pub fn apply_scenario_value<I>(
    instrument: &I,
    base_value: Money,
) -> finstack_quant_core::Result<Money>
where
    I: crate::instruments::Instrument + ?Sized,
{
    instrument
        .get_scenario_pricing_overrides()
        .map_or(Ok(base_value), |overrides| {
            overrides.apply_to_value(base_value)
        })
}

/// Apply an instrument's scenario price adjustment without currency rounding.
///
/// This is the raw-f64 counterpart to [`apply_scenario_value`]. Keeping the
/// multiplier outside `Money` preserves the precision expected by finite-
/// difference risk calculations.
///
/// # Arguments
///
/// * `instrument` - Instrument potentially carrying `ScenarioPricingOverrides`.
/// * `base_value` - Unshocked raw scalar present value to scale.
#[inline]
pub fn apply_scenario_raw_value<I>(instrument: &I, base_value: f64) -> f64
where
    I: crate::instruments::Instrument + ?Sized,
{
    instrument
        .get_scenario_pricing_overrides()
        .and_then(|overrides| overrides.scenario_price_shock_decimal)
        .map_or(base_value, |shock| base_value * (1.0 + shock))
}

/// Pricing options, dispatch and pricer-published measures handed from the
/// registry to [`compute_metrics_dyn`].
#[derive(Default)]
pub(crate) struct MetricBuildOptions {
    /// Config, market history and recalibration provider for the metric context.
    pub pricing: PricingOptions,
    /// Pricer dispatch metric calculators use when they reprice bumped markets.
    pub pricing_dispatch: PricingDispatch,
    /// Measures the model pricer already published for this valuation, taken
    /// verbatim from its `ValuationResult`. Standard identifiers among them
    /// seed the metric context; see [`compute_metrics_dyn`].
    pub pricer_measures: IndexMap<MetricId, f64>,
    /// State the model pricer attached to its `ValuationResult`; installed into
    /// the metric context before the instrument seeds it.
    pub pricer_seed: Option<Arc<dyn crate::metrics::MetricSeed>>,
}

/// Compute requested metric measures for an already-priced instrument.
///
/// The caller owns the `ValuationResult` envelope and passes the final,
/// scenario-adjusted base value. This helper only builds the metric context and
/// returns deterministic measures, preventing metric attachment from replacing
/// model details or metadata.
///
/// # Arguments
///
/// * `instrument` - The priced instrument, shared with the metric context.
/// * `curves` - Market data context the base value was priced against.
/// * `as_of` - Valuation date.
/// * `base_value` - Final scenario-adjusted base value (NPV).
/// * `metrics` - Metrics to compute. Every entry is forwarded to the metric
///   registry unfiltered, so each one either lands in the returned map or
///   aborts the call with an error; requested metrics are never silently
///   dropped. Callers that tolerate partial support across heterogeneous
///   instruments (composites) must narrow this list themselves.
/// * `options` - Pricing options (config, market history for historical
///   VaR/ES, recalibration provider), the pricer dispatch used for bumped
///   reprices, and the measures the model pricer already published.
///
/// # Errors
///
/// Returns `Error::UnknownMetric` when a requested identifier is not
/// registered, `Error::MetricNotApplicable` when it is registered but has no
/// calculator for this instrument type, and `Error::MetricCalculationFailed`
/// when a calculator runs and fails (for example on a missing market input).
pub(crate) fn compute_metrics_dyn(
    instrument: Arc<dyn crate::instruments::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_quant_core::Result<IndexMap<crate::metrics::MetricId, f64>> {
    let MetricBuildOptions {
        pricing,
        pricing_dispatch,
        pricer_measures,
        pricer_seed,
    } = options;
    let finstack_config = pricing.config.unwrap_or_else(MetricContext::default_config);
    let mut context = MetricContext::new(
        Arc::clone(&instrument),
        curves,
        as_of,
        base_value,
        finstack_config,
    );

    if let Some(history) = pricing.market_history {
        context = context.with_market_history(history);
    }
    context.set_recalibration_provider(pricing.recalibration_provider);
    context.set_pricer_dispatch(pricing_dispatch);
    context.set_metric_pricing_overrides(instrument.get_metric_pricing_overrides().cloned());

    if let Some(seed) = pricer_seed {
        seed.seed(&mut context);
    }
    let market_ref: Arc<MarketContext> = Arc::clone(&context.curves);
    instrument.seed_metric_context(&mut context, market_ref.as_ref(), as_of);

    if !metrics.is_empty()
        && let PricingDispatch::Registered { model, registry } = context.clone_pricer_dispatch()
    {
        let key = crate::pricer::PricerKey::new(instrument.key(), model);
        if let Some(pricer) = registry.get_pricer(key) {
            pricer.seed_metric_context(&mut context, metrics)?;
        }
    }

    for (metric_id, value) in pricer_measures {
        if !metric_id.is_custom() {
            context.computed.insert(metric_id, value);
        }
    }

    let registry = match pricing.metric_registry.as_deref() {
        Some(registry) => registry,
        None => instrument.family().metrics(),
    };
    let metric_measures = registry.compute(metrics, &mut context)?;

    let mut measures: IndexMap<MetricId, f64> = IndexMap::with_capacity(metrics.len() + 4);
    for metric_id in metrics {
        if let Some(value) = metric_measures.get(metric_id) {
            measures.insert(metric_id.clone(), *value);
        }
    }

    let mut extras: Vec<(&crate::metrics::MetricId, f64)> = context
        .computed
        .iter()
        .filter_map(|(metric_id, value)| {
            if (metric_id.is_custom() || MetricId::SIDE_OUTPUT_METRICS.contains(metric_id))
                && !measures.contains_key(metric_id)
            {
                Some((metric_id, *value))
            } else {
                None
            }
        })
        .collect();
    extras.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
    for (metric_id, value) in extras {
        crate::metrics::registry::validate_measure(metric_id, value)?;
        measures.insert(metric_id.clone(), value);
    }

    Ok(measures)
}

/// Test-only result builder for metric calculator unit fixtures.
///
/// Production pricing enriches the model-produced result envelope in the
/// registry and must not construct a parallel `ValuationResult` here.
#[cfg(test)]
pub(crate) fn build_with_metrics_dyn(
    instrument: Arc<dyn crate::instruments::Instrument>,
    curves: Arc<MarketContext>,
    as_of: Date,
    base_value: Money,
    metrics: &[crate::metrics::MetricId],
    options: MetricBuildOptions,
) -> finstack_quant_core::Result<crate::results::ValuationResult> {
    let cfg = options
        .pricing
        .config
        .clone()
        .unwrap_or_else(MetricContext::default_config);
    let instrument_id = instrument.id().to_string();
    let measures = compute_metrics_dyn(instrument, curves, as_of, base_value, metrics, options)?;
    let mut result = crate::results::ValuationResult::stamped_with_meta(
        &instrument_id,
        as_of,
        base_value,
        finstack_quant_core::config::results_meta(cfg.as_ref()),
    );
    result.measures = measures;
    Ok(result)
}
