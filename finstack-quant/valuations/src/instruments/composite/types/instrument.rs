//! Priceable composite instrument and its [`Instrument`] implementation.

use super::reporting::{
    aggregate_primitive_paths, flatten_composite, register_execution_definition,
    CompositeExposureReport, CompositeLegValuation, CompositeRebalanceResult, CompositeTrade,
    CompositeValuationDetails, PrimitiveExposure,
};
use super::spec::CompositeSpec;
use super::spec_support::{
    convert_amount, validate_history, CompositeLegSpec, CompositeMarketObservation, CompositeState,
    RebalanceRule, WeightingMethod, MIN_ABS_INPUT,
};
use crate::instruments::common_impl::dependencies::MarketDependencies;
use crate::instruments::{
    Attributes, Instrument, InstrumentEnvelope, InstrumentJson, InstrumentPricingOverrides,
    MetricPricingOverrides, PricingOptions, ScenarioPricingOverrides,
};
use crate::metrics::{is_additive_metric, MetricId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::math::summation::neumaier_sum;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Runtime cache of boxed composite legs. Not serialized.
#[derive(Default)]
pub(super) struct BoxedLegCache(OnceLock<Vec<Box<dyn Instrument>>>);

impl Clone for BoxedLegCache {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl std::fmt::Debug for BoxedLegCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoxedLegCache")
    }
}

/// Priceable composite instrument containing an unresolved policy and immutable state.
///
/// Valuation and risk use `state` exactly as stored. Call [`Self::rebalance`]
/// to obtain a distinct instrument; this type does not mutate in place.
///
/// # Examples
///
/// ```
/// use finstack_quant_valuations::instruments::{CompositeInstrument, Instrument};
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
    /// Boxed legs materialized once per instance.
    #[serde(skip)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    pub(super) boxed_legs: BoxedLegCache,
}

impl CompositeInstrument {
    /// Create and validate a resolved composite instrument.
    ///
    /// # Arguments
    ///
    /// * `spec` - Self-contained composite definition, including future
    ///   rebalance policy.
    /// * `state` - Immutable resolved quantities, effective date, and finite
    ///   weighting-audit inputs; leg order must match `spec.legs`.
    ///
    /// # Errors
    ///
    /// Returns an error when the specification or state is inconsistent.
    pub fn new(spec: CompositeSpec, state: CompositeState) -> Result<Self> {
        let instrument = Self {
            spec,
            state,
            boxed_legs: BoxedLegCache::default(),
        };
        instrument.validate_invariants()?;
        Ok(instrument)
    }

    fn boxed_legs(&self) -> Result<&[Box<dyn Instrument>]> {
        if let Some(legs) = self.boxed_legs.0.get() {
            return Ok(legs.as_slice());
        }
        let legs = self
            .spec
            .legs
            .iter()
            .map(|leg| leg.instrument.as_ref().clone().into_boxed())
            .collect::<Result<Vec<_>>>()?;
        let _ = self.boxed_legs.0.set(legs);
        self.boxed_legs.0.get().map(Vec::as_slice).ok_or_else(|| {
            Error::Internal("boxed composite legs cache missing after initialization".into())
        })
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
        let long = crate::instruments::Equity::new("COMPOSITE-LONG", "LONG", Currency::USD)
            .with_shares(1.0)
            .with_price(100.0);
        let short = crate::instruments::Equity::new("COMPOSITE-SHORT", "SHORT", Currency::USD)
            .with_shares(1.0)
            .with_price(90.0);
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
    ///
    /// # Errors
    ///
    /// Returns an error for invalid market/history inputs or weighting failures.
    pub fn rebalance(
        &self,
        market: &MarketContext,
        as_of: Date,
        history: &[CompositeMarketObservation],
    ) -> Result<CompositeRebalanceResult> {
        self.spec.validate()?;
        validate_history(history, Some(as_of))?;
        let (quantities, inputs) = self.spec.resolve_quantities(market, as_of, history)?;
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
        flatten_composite(self, 1.0, &mut path, &mut out)?;
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
    ///
    /// # Errors
    ///
    /// Returns an error for non-additive metrics, invalid embedded instruments,
    /// missing market data, or a metric unsupported by every primitive.
    pub fn primitive_exposure_report(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
    ) -> Result<CompositeExposureReport> {
        self.primitive_exposure_report_with_options(
            market,
            as_of,
            metrics,
            PricingOptions::default(),
        )
    }

    pub(crate) fn primitive_exposure_report_with_options(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> Result<CompositeExposureReport> {
        if let Some(metric) = metrics.iter().find(|metric| !is_additive_metric(metric)) {
            return Err(Error::Validation(format!(
                "metric '{}' is not additive and cannot be aggregated for composite '{}'",
                metric, self.spec.id
            )));
        }
        let flattened = self.flatten_primitives()?;
        let mut paths = Vec::with_capacity(flattened.len());
        let mut supported = BTreeMap::<String, usize>::new();
        let mut price_cache = BTreeMap::<String, crate::results::ValuationResult>::new();
        for mut exposure in flattened {
            let instrument_json = exposure.instrument.as_ref().ok_or_else(|| {
                Error::Internal("primitive exposure lost its runtime instrument".to_string())
            })?;
            let cache_key = InstrumentEnvelope::new(instrument_json.clone()).content_hash()?;
            let result = match price_cache.get(&cache_key) {
                Some(cached) => cached.clone(),
                None => {
                    let instrument = instrument_json.clone().into_boxed()?;
                    let priced =
                        instrument.price_with_metrics(market, as_of, metrics, options.clone())?;
                    price_cache.insert(cache_key, priced.clone());
                    priced
                }
            };
            let unit_value = convert_amount(
                market,
                result.value.amount(),
                result.value.currency(),
                self.spec.reporting_currency,
                as_of,
            )?;
            exposure.value =
                Money::new(unit_value * exposure.quantity, self.spec.reporting_currency)?;
            for (metric_id, value) in result.measures {
                if !is_additive_metric(&metric_id) {
                    continue;
                }
                let converted = convert_amount(
                    market,
                    value,
                    result.value.currency(),
                    self.spec.reporting_currency,
                    as_of,
                )?;
                exposure
                    .measures
                    .insert(metric_id.clone(), converted * exposure.quantity);
                *supported.entry(metric_id.to_string()).or_default() += 1;
            }
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
                    "metric '{}' is unsupported by every primitive in composite '{}'",
                    metric, self.spec.id
                )));
            }
        }
        aggregate_primitive_paths(self.spec.reporting_currency, paths)
    }

    pub(crate) fn valuation_details_with_metrics(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> Result<(IndexMap<MetricId, f64>, CompositeValuationDetails)> {
        let report =
            self.primitive_exposure_report_with_options(market, as_of, metrics, options.clone())?;
        let leg_results = self.top_level_leg_results(market, as_of, metrics, options)?;
        let mut measures = IndexMap::<MetricId, f64>::new();
        for path in &report.paths {
            for (metric, value) in &path.measures {
                *measures.entry(metric.clone()).or_default() += *value;
            }
        }
        Ok((
            measures,
            CompositeValuationDetails {
                state_effective_date: self.state.effective_date,
                reporting_currency: self.spec.reporting_currency,
                resolved_legs: self.state.resolved_legs.clone(),
                weighting_inputs: self.state.weighting_inputs.clone(),
                leg_results,
                exposure_report: report,
            },
        ))
    }

    fn top_level_leg_results(
        &self,
        market: &MarketContext,
        as_of: Date,
        metrics: &[MetricId],
        options: PricingOptions,
    ) -> Result<Vec<CompositeLegValuation>> {
        self.boxed_legs()?
            .iter()
            .zip(&self.spec.legs)
            .zip(&self.state.resolved_legs)
            .map(|((instrument, leg), resolved)| {
                let valuation =
                    instrument.price_with_metrics(market, as_of, metrics, options.clone())?;
                let native_value = Money::new(
                    valuation.value.amount() * resolved.quantity,
                    valuation.value.currency(),
                )?;
                let reporting_value = Money::new(
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

    fn base_value_raw_impl(&self, market: &MarketContext, as_of: Date) -> Result<f64> {
        let values = self
            .boxed_legs()?
            .iter()
            .zip(&self.state.resolved_legs)
            .map(|(instrument, resolved)| {
                let (amount, currency) = instrument.value_raw_with_currency(market, as_of)?;
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
        Ok(neumaier_sum(values))
    }
}

impl Instrument for CompositeInstrument {
    fn id(&self) -> &str {
        self.spec.id.as_str()
    }

    fn key(&self) -> crate::pricer::InstrumentType {
        crate::pricer::InstrumentType::Composite
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

    fn validate_invariants(&self) -> Result<()> {
        self.spec.validate()?;
        self.validate_state()
    }

    fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
        Money::new(
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
        for (leg, instrument) in self.spec.legs.iter().zip(self.boxed_legs()?) {
            dependencies.merge(MarketDependencies::from_instrument_json(
                leg.instrument.as_ref(),
            )?);
            if let Some(notional) = instrument.notional()? {
                if notional.currency() != self.spec.reporting_currency {
                    dependencies.add_fx_pair(notional.currency(), self.spec.reporting_currency);
                }
            }
        }
        Ok(dependencies)
    }

    fn valuation_details(
        &self,
        market: &MarketContext,
        as_of: Date,
    ) -> Option<crate::results::ValuationDetails> {
        self.primitive_exposure_report(market, as_of, &[])
            .ok()
            .and_then(|exposure_report| {
                let leg_results = self
                    .top_level_leg_results(market, as_of, &[], PricingOptions::default())
                    .ok()?;
                Some(crate::results::ValuationDetails::Composite(
                    CompositeValuationDetails {
                        state_effective_date: self.state.effective_date,
                        reporting_currency: self.spec.reporting_currency,
                        resolved_legs: self.state.resolved_legs.clone(),
                        weighting_inputs: self.state.weighting_inputs.clone(),
                        leg_results,
                        exposure_report,
                    },
                ))
            })
    }

    fn get_instrument_pricing_overrides(&self) -> Option<&InstrumentPricingOverrides> {
        Some(&self.spec.instrument_pricing_overrides)
    }

    fn get_instrument_pricing_overrides_mut(&mut self) -> Option<&mut InstrumentPricingOverrides> {
        Some(&mut self.spec.instrument_pricing_overrides)
    }

    fn get_metric_pricing_overrides(&self) -> Option<&MetricPricingOverrides> {
        Some(&self.spec.metric_pricing_overrides)
    }

    fn get_metric_pricing_overrides_mut(&mut self) -> Option<&mut MetricPricingOverrides> {
        Some(&mut self.spec.metric_pricing_overrides)
    }

    fn get_scenario_pricing_overrides(&self) -> Option<&ScenarioPricingOverrides> {
        Some(&self.spec.scenario_pricing_overrides)
    }

    fn get_scenario_pricing_overrides_mut(&mut self) -> Option<&mut ScenarioPricingOverrides> {
        Some(&mut self.spec.scenario_pricing_overrides)
    }
}

crate::impl_empty_cashflow_provider!(
    CompositeInstrument,
    crate::cashflow::builder::CashflowRepresentation::Placeholder
);
