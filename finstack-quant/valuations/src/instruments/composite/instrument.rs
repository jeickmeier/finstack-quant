//! Priceable composite instrument and its [`Instrument`] implementation.

use super::reporting::{
    CompositeRebalanceResult, CompositeTrade, aggregate_primitive_paths, flatten_composite,
    register_execution_definition,
};
use super::spec::CompositeSpec;
use super::spec_support::{
    CompositeLegSpec, CompositeMarketObservation, CompositeState, MIN_ABS_INPUT, RebalanceRule,
    WeightingMethod, convert_amount, validate_history,
};
use crate::instruments::MarketDependencies;
use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use crate::instruments::pricing_overrides::{
    InstrumentPricingOverrides, MetricPricingOverrides, ScenarioPricingOverrides,
};
use crate::instruments::{FxPair, Instrument, PricingOptions};
use crate::metrics::{MetricAggregation, MetricId};
use crate::results::composite::{
    CompositeExposureReport, CompositeLegValuation, CompositeValuationDetails, PrimitiveExposure,
};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Narrow a composite's requested metric list to the subset one leg supports.
///
/// Pricing a single instrument rejects any requested metric that has no
/// calculator for that instrument type. A composite aggregates heterogeneous
/// legs and only requires that *some* primitive support each requested metric,
/// so it selects the per-leg subset explicitly here rather than relying on the
/// single-instrument path to drop the rest. Metrics unsupported by every
/// primitive are still rejected, by the caller's own coverage check.
///
/// # Arguments
///
/// * `instrument` - Composite leg whose [`Instrument::applicable_metrics`]
///   decides the subset (a nested composite answers for its own legs).
/// * `metrics` - Metric identifiers requested for the composite as a whole, in
///   caller order; the returned subset preserves that order.
/// * `options` - Pricing options whose `metric_registry`, when present, decides
///   applicability; otherwise the leg's own family registry is used.
fn metrics_supported_by_leg(
    instrument: &dyn Instrument,
    metrics: &[MetricId],
    options: &PricingOptions,
) -> Vec<MetricId> {
    let registry = match options.metric_registry.as_deref() {
        Some(registry) => registry,
        None => instrument.family().metrics(),
    };
    instrument.applicable_metrics(metrics, registry)
}

/// Priceable composite instrument containing an unresolved policy and resolved state.
///
/// Valuation and risk use the current `spec` and `state` exactly as stored.
/// Call [`Self::rebalance`] to obtain a distinct instrument with newly resolved
/// quantities. Direct field edits must preserve the specification/state invariants.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::instruments::Instrument;
/// use finstack_quant_valuations::instruments::composite::CompositeInstrument;
///
/// let composite = CompositeInstrument::example()?;
/// assert_eq!(composite.id(), "COMPOSITE-EXAMPLE");
/// assert_eq!(composite.state.resolved_legs.len(), 2);
/// # Ok::<(), finstack_quant_core::Error>(())
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeInstrument {
    /// Economic definition and future rebalance policy.
    pub spec: CompositeSpec,
    /// Frozen quantities used for every valuation until explicit rebalance.
    pub state: CompositeState,
    /// Instrument-owned pricing inputs.
    ///
    /// A composite has no pricing model of its own, so this must stay empty;
    /// set quotes and model inputs on each leg instead.
    #[serde(default, skip_serializing_if = "InstrumentPricingOverrides::is_empty")]
    pub instrument_pricing_overrides: InstrumentPricingOverrides,
    /// Metric-time pricing configuration.
    ///
    /// Also applied to every leg as defaults: a leg's own setting wins, and
    /// `bump_config.adaptive_bumps` is enabled on a leg when either sets it.
    #[serde(default, skip_serializing_if = "MetricPricingOverrides::is_empty")]
    pub metric_pricing_overrides: MetricPricingOverrides,
    /// Scenario-only pricing adjustments.
    ///
    /// Applied to the composite value after leg aggregation.
    #[serde(default, skip_serializing_if = "ScenarioPricingOverrides::is_empty")]
    pub scenario_pricing_overrides: ScenarioPricingOverrides,
}

impl CompositeInstrument {
    fn boxed_legs(&self) -> Result<Vec<Box<dyn Instrument>>> {
        self.spec
            .legs
            .iter()
            .map(|leg| leg.instrument.as_ref().clone().into_boxed())
            .collect()
    }

    /// Return a canonical fixed-quantity long/short equity example.
    ///
    /// Long `COMPOSITE-LONG` at 100 and short `COMPOSITE-SHORT` at 90, each
    /// with quantity `±1`, effective `2025-01-01`, and USD 100 capital.
    ///
    /// # Errors
    ///
    /// Returns an error if the generated example fails validation.
    pub fn example() -> Result<Self> {
        let long =
            crate::instruments::equity::spot::Equity::new("COMPOSITE-LONG", "LONG", Currency::USD)
                .with_quantity(1.0)
                .with_quoted_spot(100.0);
        let short = crate::instruments::equity::spot::Equity::new(
            "COMPOSITE-SHORT",
            "SHORT",
            Currency::USD,
        )
        .with_quantity(1.0)
        .with_quoted_spot(90.0);
        let spec = CompositeSpec::new(
            "COMPOSITE-EXAMPLE",
            Currency::USD,
            Money::from((100_i64, Currency::USD)),
            vec![
                CompositeLegSpec::new("COMPOSITE-LONG", InstrumentJson::Equity(long), 1.0),
                CompositeLegSpec::new("COMPOSITE-SHORT", InstrumentJson::Equity(short), -1.0),
            ],
            WeightingMethod::FixedQuantity,
            RebalanceRule::Manual,
        );
        Ok(spec
            .initialize_fixed(time::macros::date!(2025 - 01 - 01))?
            .instrument)
    }

    pub(super) fn validate_state(&self) -> Result<()> {
        if self.state.resolved_legs.len() != self.spec.legs.len() {
            return Err(Error::Validation(format!(
                "composite '{}' state leg count does not match its specification",
                self.spec.id
            )));
        }
        for (spec, state) in self.spec.legs.iter().zip(&self.state.resolved_legs) {
            if spec.instrument_id != state.instrument_id {
                return Err(Error::Validation(format!(
                    "composite '{}' state leg order or identifier mismatch",
                    self.spec.id
                )));
            }
            if !state.quantity.is_finite() || state.quantity.abs() <= MIN_ABS_INPUT {
                return Err(Error::Validation(format!(
                    "composite state quantity for '{}' must be finite and non-zero",
                    state.instrument_id
                )));
            }
        }
        if self
            .state
            .weighting_inputs
            .values()
            .any(|value| !value.is_finite())
        {
            return Err(Error::Validation(
                "composite state weighting inputs must be finite".to_string(),
            ));
        }
        Ok(())
    }

    /// Explicitly resolve a new immutable state and primitive execution deltas.
    ///
    /// The receiver is not mutated. Trades are net primitive quantity deltas
    /// from this state to the newly resolved state.
    ///
    /// # Arguments
    ///
    /// * `market` - Complete current market used to resolve dynamic quantities
    ///   and convert notionals, metrics, and FX into `reporting_currency`.
    /// * `as_of` - Effective date for the distinct returned state; no later
    ///   history observation is permitted.
    /// * `history` - Strictly increasing observations available through
    ///   `as_of`. Required for volatility weighting and must end on `as_of`.
    /// * `options` - Pricing options used when metric-weighted or
    ///   user-defined weighting prices the legs; quote-recalibrated weighting
    ///   metrics such as `cs01` need a recalibration provider.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid market/history inputs or weighting failures.
    pub fn rebalance(
        &self,
        market: &MarketContext,
        as_of: Date,
        history: &[CompositeMarketObservation],
        options: PricingOptions,
    ) -> Result<CompositeRebalanceResult> {
        self.spec.validate()?;
        validate_history(history, Some(as_of))?;
        let (quantities, inputs) = self
            .spec
            .resolve_quantities(market, as_of, history, options)?;
        self.spec
            .build_rebalance_result(as_of, quantities, inputs, Some(self))
    }

    /// Return frozen top-level quantities as execution deltas from an optional prior state.
    ///
    /// Nested composites are flattened first. Deltas smaller than `1e-12` in
    /// absolute value are omitted. A reused primitive identifier must carry
    /// the same definition in both states.
    ///
    /// # Arguments
    ///
    /// * `previous` - Prior resolved state used as the trade baseline, or
    ///   `None` to treat every current primitive quantity as an establishment
    ///   trade.
    ///
    /// # Errors
    ///
    /// Returns an error when either state is invalid or primitive definitions conflict.
    pub fn execution_trades(
        &self,
        previous: Option<&CompositeInstrument>,
    ) -> Result<Vec<CompositeTrade>> {
        let current = self.flatten_primitives()?;
        let prior = match previous {
            Some(previous) => previous.flatten_primitives()?,
            None => Vec::new(),
        };
        let mut deltas = BTreeMap::<String, (String, f64)>::new();
        let mut definitions = BTreeMap::<String, String>::new();
        for exposure in prior {
            register_execution_definition(&mut definitions, &exposure)?;
            let entry = deltas
                .entry(exposure.instrument_id.to_string())
                .or_insert((exposure.instrument_type, 0.0));
            entry.1 -= exposure.quantity;
        }
        for exposure in current {
            register_execution_definition(&mut definitions, &exposure)?;
            let entry = deltas
                .entry(exposure.instrument_id.to_string())
                .or_insert((exposure.instrument_type, 0.0));
            entry.1 += exposure.quantity;
        }
        Ok(deltas
            .into_iter()
            .filter_map(|(instrument_id, (instrument_type, quantity_delta))| {
                (quantity_delta.abs() > MIN_ABS_INPUT).then(|| CompositeTrade {
                    instrument_id: InstrumentId::new(instrument_id),
                    instrument_type,
                    quantity_delta,
                })
            })
            .collect())
    }

    /// Recursively flatten the frozen state into path-level primitive quantities.
    ///
    /// # Errors
    ///
    /// Returns an error when the composite tree is invalid or exceeds resource limits.
    pub fn flatten_primitives(&self) -> Result<Vec<PrimitiveExposure>> {
        self.validate_invariants()?;
        let mut out = Vec::new();
        let mut path = vec![self.spec.id.to_string()];
        flatten_composite(
            self,
            1.0,
            &MetricPricingOverrides::default(),
            &mut path,
            &mut out,
        )?;
        Ok(out)
    }

    /// Price path-level primitives and return net and gross aggregates.
    ///
    /// Only additive metrics can be requested because a top-level value for
    /// yield, duration, implied volatility, or another non-linear measure would
    /// conceal instrument-specific meaning. Path values and additive measures
    /// are converted to `reporting_currency` on `as_of`.
    ///
    /// # Arguments
    ///
    /// * `market` - Complete market used to price every primitive and convert
    ///   native amounts into `reporting_currency`.
    /// * `as_of` - Valuation date and FX conversion date.
    /// * `metrics` - Additive risk metrics to aggregate; an empty slice
    ///   reports value only. Non-additive identifiers are rejected.
    /// * `options` - Pricing options applied to every primitive, as for
    ///   the host `price_instrument` entry points. Quote-recalibrated metrics
    ///   such as `cs01` need a recalibration provider; hosts pass
    ///   `finstack_quant_calibration::recalibration::pricing_options()`.
    ///
    /// # Errors
    ///
    /// Returns an error for non-additive metrics, invalid embedded instruments,
    /// missing market data, or a metric unsupported by every primitive.
    pub fn primitive_exposures(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> Result<CompositeExposureReport> {
        self.primitive_exposures_with_cache(market, as_of, metrics, options, &mut BTreeMap::new())
    }

    fn primitive_exposures_with_cache(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
        price_cache: &mut BTreeMap<String, crate::results::ValuationResult>,
    ) -> Result<CompositeExposureReport> {
        let flattened = self.flatten_primitives()?;
        let mut paths = Vec::with_capacity(flattened.len());
        let mut supported = BTreeMap::<String, (MetricAggregation, Option<u64>)>::new();
        for mut exposure in flattened {
            let instrument_json = exposure
                .instrument()
                .and_then(crate::instruments::json_loader::instrument_json_of)
                .ok_or_else(|| {
                    Error::Internal("primitive exposure lost its runtime instrument".to_string())
                })?;
            let result = cached_valuation(
                price_cache,
                instrument_json,
                &exposure.metric_defaults,
                |definition| {
                    let mut instrument = definition.into_boxed()?;
                    if let Some(overrides) = instrument.get_metric_pricing_overrides_mut() {
                        overrides.apply_defaults(&exposure.metric_defaults);
                    }
                    let mut leg_metrics =
                        metrics_supported_by_leg(instrument.as_ref(), metrics, &options);
                    if leg_metrics.contains(&MetricId::Theta) {
                        leg_metrics.push(MetricId::ThetaPeriodDays);
                    }
                    Ok(instrument.price_with_metrics(
                        market,
                        as_of,
                        &leg_metrics,
                        options.clone(),
                    )?)
                },
            )?;
            let unit_value = convert_amount(
                market,
                result.value.amount(),
                result.value.currency(),
                self.spec.reporting_currency,
                as_of,
            )?;
            exposure.value = Money::from_model_value(
                unit_value * exposure.quantity,
                self.spec.reporting_currency,
            )?;
            let instrument = exposure.instrument().ok_or_else(|| {
                Error::Internal("primitive exposure lost its runtime instrument".into())
            })?;
            let basis = options
                .metric_registry
                .as_deref()
                .unwrap_or_else(|| instrument.family().metrics());
            let mut measures = IndexMap::new();
            let horizon = result.measures.get(&MetricId::ThetaPeriodDays).copied();
            for (metric_id, value) in result.measures {
                let requested = metrics.iter().any(|requested| {
                    requested == &metric_id
                        || metric_id
                            .as_str()
                            .strip_prefix(requested.as_str())
                            .is_some_and(|suffix| suffix.starts_with("::"))
                });
                if !requested {
                    continue;
                }
                let aggregation = basis
                    .metric_aggregation(&metric_id, instrument.key())
                    .ok_or_else(|| {
                        Error::Validation(format!(
                            "metric '{metric_id}' is not additive on leg '{}'",
                            instrument.id()
                        ))
                    })?;
                if aggregation == MetricAggregation::FactorQuantity
                    && !metric_id.as_str().contains("::")
                {
                    continue;
                }
                let horizon = if aggregation == MetricAggregation::ThetaPeriodMoney {
                    Some(
                        horizon
                            .ok_or_else(|| {
                                Error::Validation(format!(
                                    "metric '{metric_id}' requires a reported horizon from '{}'",
                                    instrument.id()
                                ))
                            })?
                            .to_bits(),
                    )
                } else {
                    None
                };
                let basis = (aggregation, horizon);
                match supported.insert(metric_id.to_string(), basis) {
                    Some(previous) if previous != basis => {
                        return Err(Error::Validation(format!(
                            "incompatible aggregation bases for '{metric_id}'"
                        )));
                    }
                    _ => {}
                }
                let converted = match aggregation {
                    MetricAggregation::NativeMoney | MetricAggregation::ThetaPeriodMoney => {
                        convert_amount(
                            market,
                            value,
                            result.value.currency(),
                            self.spec.reporting_currency,
                            as_of,
                        )?
                    }
                    MetricAggregation::FactorQuantity => value,
                };
                measures.insert(metric_id, converted * exposure.quantity);
            }
            exposure.measures = measures;
            paths.push(exposure);
        }
        for metric in metrics {
            let is_supported = supported.keys().any(|key| {
                key == metric.as_str()
                    || key
                        .strip_prefix(metric.as_str())
                        .is_some_and(|suffix| suffix.starts_with("::"))
            });
            if !is_supported {
                return Err(Error::Validation(format!(
                    "metric '{}' is not additive on any primitive in composite '{}'",
                    metric, self.spec.id
                )));
            }
        }
        aggregate_primitive_paths(self.spec.reporting_currency, paths)
    }

    fn top_level_leg_results(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
        price_cache: &mut BTreeMap<String, crate::results::ValuationResult>,
    ) -> Result<Vec<CompositeLegValuation>> {
        self.boxed_legs()?
            .iter()
            .zip(&self.spec.legs)
            .zip(&self.state.resolved_legs)
            .map(|((leg_instrument, leg), resolved)| {
                let defaulted = self.leg_with_metric_defaults(leg_instrument.as_ref());
                let instrument = defaulted.as_ref().unwrap_or(leg_instrument);
                let leg_metrics = metrics_supported_by_leg(instrument.as_ref(), metrics, &options);
                // The request owns the cache: market/date/options and requested metrics
                // are identical to the primitive pass. Definition and inherited defaults
                // distinguish only results that can safely be reused.
                let valuation = cached_valuation(
                    price_cache,
                    leg.instrument.as_ref().clone(),
                    &self.metric_pricing_overrides,
                    |_| {
                        Ok(instrument.price_with_metrics(
                            market,
                            as_of,
                            &leg_metrics,
                            options.clone(),
                        )?)
                    },
                )?;
                let native_value = Money::from_model_value(
                    valuation.value.amount() * resolved.quantity,
                    valuation.value.currency(),
                )?;
                let reporting_value = Money::from_model_value(
                    convert_amount(
                        market,
                        native_value.amount(),
                        native_value.currency(),
                        self.spec.reporting_currency,
                        as_of,
                    )?,
                    self.spec.reporting_currency,
                )?;
                Ok(CompositeLegValuation {
                    instrument_id: leg.instrument_id.clone(),
                    quantity: resolved.quantity,
                    native_value,
                    reporting_value,
                    valuation,
                })
            })
            .collect()
    }

    /// Clone `leg` with this composite's metric overrides applied as defaults.
    ///
    /// Returns `None` when there is nothing to apply, so callers keep using
    /// the materialized leg.
    fn leg_with_metric_defaults(&self, leg: &dyn Instrument) -> Option<Box<dyn Instrument>> {
        if self.metric_pricing_overrides.is_empty() {
            return None;
        }
        let mut leg = leg.clone_box();
        leg.get_metric_pricing_overrides_mut()?
            .apply_defaults(&self.metric_pricing_overrides);
        Some(leg)
    }

    fn base_value_raw_impl(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        self.value_legs(market, as_of, |instrument| {
            instrument.value_raw_with_currency(market, as_of)
        })
    }

    fn value_legs(
        &self,
        market: &MarketContext,
        as_of: Date,
        mut reprice: impl FnMut(&dyn Instrument) -> Result<(f64, Currency)>,
    ) -> Result<f64> {
        let values = self
            .boxed_legs()?
            .iter()
            .zip(&self.state.resolved_legs)
            .map(|(instrument, resolved)| {
                let (amount, currency) = reprice(instrument.as_ref())?;
                let converted = convert_amount(
                    market,
                    amount,
                    currency,
                    self.spec.reporting_currency,
                    as_of,
                )?;
                Ok(converted * resolved.quantity)
            })
            .collect::<Result<Vec<_>>>()?;
        neumaier_sum(values)
    }

    fn value_with_fx_shock(
        &self,
        market: &MarketContext,
        as_of: Date,
        pair: FxPair,
        relative: f64,
    ) -> Result<f64> {
        self.value_legs(market, as_of, |instrument| {
            if let Some(nested) = instrument.as_any().downcast_ref::<CompositeInstrument>() {
                let value = nested.value_with_fx_shock(market, as_of, pair, relative)?;
                return Ok((value, nested.spec.reporting_currency));
            }
            if let Some((base, quote)) = instrument.fx_exposure() {
                let oriented = if (base, quote) == (pair.base, pair.quote) {
                    Some(relative)
                } else if (quote, base) == (pair.base, pair.quote) {
                    Some(1.0 / (1.0 + relative) - 1.0)
                } else {
                    None
                };
                if let Some(bumped) =
                    oriented.and_then(|move_by| instrument.with_bumped_fx_quote(move_by))
                {
                    return bumped.value_raw_with_currency(market, as_of);
                }
            }
            instrument.value_raw_with_currency(market, as_of)
        })
    }

    fn fx01(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        use finstack_quant_core::market_data::bumps::MarketBump;
        use finstack_quant_core::money::fx::FxConversionPolicy;

        let pair = self.spec.fx01_pair.ok_or_else(|| {
            Error::Validation("composite FX01 requires a declared ordered FX pair".into())
        })?;
        let reprice = |pct| {
            let shocked;
            let market = if market.fx().is_some() {
                shocked = market.bump([MarketBump::FxPct {
                    base: pair.base,
                    quote: pair.quote,
                    pct,
                    as_of,
                    policy: FxConversionPolicy::CashflowDate,
                }])?;
                &shocked
            } else {
                market
            };
            self.value_with_fx_shock(market, as_of, pair, pct / 100.0)
        };
        Ok((reprice(1.0)? - reprice(-1.0)?) / 2.0)
    }
}

/// Price `definition` through the per-request `price_cache`; `price` receives
/// the definition back on a miss.
///
/// The key is the content hash of `definition`, extended by the debug form of
/// `defaults` when any are inherited, so only results priced from the same
/// definition under the same defaults are reused.
fn cached_valuation(
    price_cache: &mut BTreeMap<String, crate::results::ValuationResult>,
    definition: InstrumentJson,
    defaults: &MetricPricingOverrides,
    price: impl FnOnce(InstrumentJson) -> Result<crate::results::ValuationResult>,
) -> Result<crate::results::ValuationResult> {
    let envelope = InstrumentEnvelope::new(definition);
    let mut cache_key = envelope.content_hash()?;
    if !defaults.is_empty() {
        cache_key.push_str(&format!("{defaults:?}"));
    }
    if let Some(cached) = price_cache.get(&cache_key) {
        return Ok(cached.clone());
    }
    let priced = price(envelope.instrument)?;
    price_cache.insert(cache_key, priced.clone());
    Ok(priced)
}

impl Instrument for CompositeInstrument {
    fn includes_valuation_date_cashflows(&self) -> bool {
        false
    }
    fn id(&self) -> &str {
        self.spec.id.as_str()
    }

    fn key(&self) -> crate::pricer::InstrumentType {
        crate::pricer::InstrumentType::Composite
    }

    fn family(&self) -> &'static crate::pricer::Family {
        &crate::pricer::STANDARD
    }

    fn composite_valuation(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> Result<Option<(IndexMap<MetricId, f64>, crate::results::ValuationDetails)>> {
        let fx01_requested = metrics.contains(&MetricId::Fx01);
        let metrics: Vec<_> = metrics
            .iter()
            .filter(|metric| **metric != MetricId::Fx01)
            .cloned()
            .collect();
        let mut price_cache = BTreeMap::new();
        let report = self.primitive_exposures_with_cache(
            market,
            as_of,
            &metrics,
            options.clone(),
            &mut price_cache,
        )?;
        let leg_results =
            self.top_level_leg_results(market, as_of, &metrics, options, &mut price_cache)?;
        let mut measures = IndexMap::<MetricId, f64>::new();
        for path in &report.paths {
            for (metric, value) in &path.measures {
                *measures.entry(metric.clone()).or_default() += *value;
            }
        }
        if fx01_requested {
            measures.insert(MetricId::Fx01, self.fx01(market, as_of)?);
        }
        let details = CompositeValuationDetails {
            state_effective_date: self.state.effective_date,
            reporting_currency: self.spec.reporting_currency,
            resolved_legs: self.state.resolved_legs.clone(),
            weighting_inputs: self.state.weighting_inputs.clone(),
            leg_results,
            exposures: report,
        };
        Ok(Some((
            measures,
            crate::results::ValuationDetails::Composite(details),
        )))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        &self.spec.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.spec.attributes
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    /// A composite computes a metric by aggregating it over its legs, so the
    /// applicable menu entries are the additive ones at least one leg
    /// supports: the same rule its pricing enforces. Legs that cannot be
    /// built leave nothing applicable; pricing then reports the build error.
    fn applicable_metrics(
        &self,
        menu: &[MetricId],
        registry: &crate::metrics::MetricRegistry,
    ) -> Vec<MetricId> {
        let Ok(legs) = self.boxed_legs() else {
            return Vec::new();
        };
        menu.iter()
            .filter(|metric| {
                if **metric == MetricId::Fx01 {
                    return self.spec.fx01_pair.is_some();
                }
                legs.iter().any(|leg| {
                    leg.applicable_metrics(std::slice::from_ref(*metric), registry)
                        .iter()
                        .any(|id| registry.metric_aggregation(id, leg.key()).is_some())
                })
            })
            .cloned()
            .collect()
    }

    fn validate_invariants(&self) -> Result<()> {
        self.spec.validate()?;
        if !self.instrument_pricing_overrides.is_empty() {
            return Err(Error::Validation(format!(
                "composite '{}' instrument.spec.instrument_pricing_overrides must be empty; \
                 set quotes and model inputs on each leg's instrument_pricing_overrides",
                self.spec.id
            )));
        }
        self.validate_state()
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        Money::from_model_value(
            self.base_value_raw_impl(market, as_of)?,
            self.spec.reporting_currency,
        )
    }

    fn base_value_raw(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        self.base_value_raw_impl(market, as_of)
    }

    fn base_value_raw_with_currency(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<(f64, Currency)> {
        Ok((
            self.base_value_raw_impl(market, as_of)?,
            self.spec.reporting_currency,
        ))
    }

    fn market_dependencies(&self) -> Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        for instrument in self.boxed_legs()? {
            dependencies.merge(instrument.market_dependencies()?);
        }
        if let Some(pair) = self.spec.fx01_pair {
            dependencies.add_fx_pair(pair.base, pair.quote);
        }
        Ok(dependencies)
    }

    fn market_dependencies_at(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<MarketDependencies> {
        let mut dependencies = MarketDependencies::new();
        for instrument in self.boxed_legs()? {
            dependencies.merge(instrument.market_dependencies_at(market, as_of)?);
            let (_, currency) = instrument.value_raw_with_currency(market, as_of)?;
            if currency != self.spec.reporting_currency {
                dependencies.add_fx_pair(currency, self.spec.reporting_currency);
            }
        }
        Ok(dependencies)
    }

    crate::impl_focused_pricing_overrides!();
}

crate::impl_empty_cashflow_provider!(
    CompositeInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::Placeholder
);

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    fn assert_current_definition(composite: &CompositeInstrument, expected: f64) {
        let market = MarketContext::new();
        let as_of = date!(2025 - 01 - 01);
        composite.validate_invariants().expect("valid composite");
        for _ in 0..2 {
            assert_eq!(
                composite.value(&market, as_of).expect("value").amount(),
                expected
            );
        }
        let copy = composite.clone();
        let decoded: CompositeInstrument =
            serde_json::from_str(&serde_json::to_string(composite).expect("serialize composite"))
                .expect("deserialize composite");
        for equivalent in [&copy, &decoded] {
            assert_eq!(
                equivalent.value(&market, as_of).expect("value").amount(),
                expected
            );
            assert_eq!(
                equivalent
                    .market_dependencies()
                    .expect("dependencies")
                    .market_scalar_ids,
                composite
                    .market_dependencies()
                    .expect("dependencies")
                    .market_scalar_ids,
            );
        }
    }

    #[test]
    fn composite_values_current_legs_after_nested_edit_reorder_and_resize() {
        let mut composite = CompositeInstrument::example().expect("example");
        assert_current_definition(&composite, 10.0);

        let InstrumentJson::Equity(equity) = composite.spec.legs[0].instrument.as_mut() else {
            panic!("equity example leg")
        };
        equity.quoted_spot = Some(200.0);
        equity.spot_id = Some("CURRENT-LONG-SPOT".into());
        assert_current_definition(&composite, 110.0);
        assert!(
            composite
                .market_dependencies()
                .expect("dependencies")
                .market_scalar_ids
                .contains(&"CURRENT-LONG-SPOT".to_string())
        );

        composite.spec.legs.swap(0, 1);
        composite.state.resolved_legs.swap(0, 1);
        assert_current_definition(&composite, 110.0);

        let added_equity =
            crate::instruments::equity::spot::Equity::new("THIRD", "THIRD", Currency::USD)
                .with_quantity(1.0)
                .with_quoted_spot(25.0);
        composite.spec.legs.push(CompositeLegSpec::new(
            "THIRD",
            InstrumentJson::Equity(added_equity),
            2.0,
        ));
        composite
            .state
            .resolved_legs
            .push(crate::results::composite::ResolvedCompositeLeg {
                instrument_id: "THIRD".into(),
                quantity: 2.0,
            });
        assert_current_definition(&composite, 160.0);
        let removed_leg = composite.spec.legs.remove(0);
        let removed_state = composite.state.resolved_legs.remove(0);
        assert_current_definition(&composite, 250.0);
        composite.spec.legs.push(removed_leg);
        composite.state.resolved_legs.push(removed_state);
        assert_current_definition(&composite, 160.0);

        composite.spec.legs.pop();
        assert!(
            composite
                .value(&MarketContext::new(), date!(2025 - 01 - 01))
                .is_err()
        );
    }
}
