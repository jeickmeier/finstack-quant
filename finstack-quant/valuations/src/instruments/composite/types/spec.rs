//! Unresolved composite specification: construction, validation,
//! initialization, and quantity resolution.

use super::instrument::{BoxedLegCache, CompositeInstrument};
use super::reporting::CompositeRebalanceResult;
use super::spec_support::{
    convert_amount, leg_index, normalized_scores, sample_std_dev, unit_pnl_series,
    validate_history, CompositeLegSpec, CompositeMarketObservation, CompositeState, RebalanceRule,
    ResolvedCompositeLeg, WeightingMethod, MAX_COMPOSITE_DEPTH, MAX_COMPOSITE_LEGS, MIN_ABS_INPUT,
};
use crate::instruments::{
    Attributes, InstrumentEnvelope, InstrumentJson, InstrumentPricingOverrides,
    MetricPricingOverrides, PricingOptions, ScenarioPricingOverrides,
};
use crate::metrics::{is_additive_metric, MetricId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::expr::{CompiledExpr, EvalOpts, Expr, SimpleContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

struct VolatilityWeightingConfig<'a> {
    anchor_leg_id: &'a InstrumentId,
    anchor_quantity: f64,
    lookback: usize,
    min_observations: usize,
    annualization_factor: f64,
}

/// Unresolved composite definition and rebalance policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeSpec {
    /// Stable composite instrument identifier.
    pub id: InstrumentId,
    /// Currency in which value, P&L, additive risk, and capital are reported.
    pub reporting_currency: Currency,
    /// Positive return denominator per composite unit.
    pub capital: Money,
    /// Self-contained underlying instrument definitions.
    pub legs: Vec<CompositeLegSpec>,
    /// Rule used to resolve quantities at initialization and rebalance.
    pub weighting_method: WeightingMethod,
    /// Dates on which explicit dynamic resolution becomes eligible.
    pub rebalance_rule: RebalanceRule,
    /// Scenario-selection and reporting metadata.
    pub attributes: Attributes,
    /// Instrument-level pricing inputs shared with the normal valuation lifecycle.
    #[serde(default, skip_serializing_if = "InstrumentPricingOverrides::is_empty")]
    pub instrument_pricing_overrides: InstrumentPricingOverrides,
    /// Metric bump and calculation controls.
    #[serde(default, skip_serializing_if = "MetricPricingOverrides::is_empty")]
    pub metric_pricing_overrides: MetricPricingOverrides,
    /// Scenario-only price adjustments applied to the composite after leg aggregation.
    #[serde(default, skip_serializing_if = "ScenarioPricingOverrides::is_empty")]
    pub scenario_pricing_overrides: ScenarioPricingOverrides,
}

impl CompositeSpec {
    /// Construct an unresolved composite specification.
    ///
    /// The constructor does not validate. Call [`Self::validate`] or
    /// [`Self::initialize`] / [`Self::initialize_fixed`] before pricing.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable composite identifier used for pricing, serialization,
    ///   and primitive exposure paths.
    /// * `reporting_currency` - Currency for capital, value, additive risk,
    ///   cashflows, P&L, and period returns.
    /// * `capital` - Positive return denominator per composite unit; amount
    ///   must be finite and denominated in `reporting_currency`.
    /// * `legs` - At least two self-contained signed legs with unique
    ///   identifiers that match their embedded instruments.
    /// * `weighting_method` - Policy used only during initialization or
    ///   explicit rebalance; pricing never re-solves quantities.
    /// * `rebalance_rule` - Manual, explicit-date, or calendar-aware rule
    ///   that marks when a new state becomes eligible.
    #[must_use]
    pub fn new(
        id: impl Into<InstrumentId>,
        reporting_currency: Currency,
        capital: Money,
        legs: Vec<CompositeLegSpec>,
        weighting_method: WeightingMethod,
        rebalance_rule: RebalanceRule,
    ) -> Self {
        Self {
            id: id.into(),
            reporting_currency,
            capital,
            legs,
            weighting_method,
            rebalance_rule,
            attributes: Attributes::new(),
            instrument_pricing_overrides: InstrumentPricingOverrides::default(),
            metric_pricing_overrides: MetricPricingOverrides::default(),
            scenario_pricing_overrides: ScenarioPricingOverrides::default(),
        }
    }

    /// Replace scenario-selection attributes.
    ///
    /// # Arguments
    ///
    /// * `attributes` - Tags and metadata copied onto the unresolved
    ///   specification and retained on every resolved instrument.
    #[must_use]
    pub fn with_attributes(mut self, attributes: Attributes) -> Self {
        self.attributes = attributes;
        self
    }

    /// Validate the complete embedded instrument tree and weighting policy.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid capital, leg counts, identifiers, embedded
    /// instruments, scores, anchors, expressions, schedules, or resource limits.
    pub fn validate(&self) -> Result<()> {
        let mut seen = BTreeMap::<String, String>::new();
        let mut count = 0usize;
        self.validate_tree(0, &mut count, &mut seen)
    }

    fn validate_tree(
        &self,
        depth: usize,
        count: &mut usize,
        seen: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        if depth >= MAX_COMPOSITE_DEPTH {
            return Err(Error::Validation(format!(
                "composite '{}' exceeds maximum nesting depth {MAX_COMPOSITE_DEPTH}",
                self.id
            )));
        }
        if self.legs.len() < 2 {
            return Err(Error::Validation(format!(
                "composite '{}' requires at least two legs",
                self.id
            )));
        }
        if !self.capital.amount().is_finite()
            || self.capital.amount() <= 0.0
            || self.capital.currency() != self.reporting_currency
        {
            return Err(Error::Validation(format!(
                "composite '{}' capital must be positive and denominated in {}",
                self.id, self.reporting_currency
            )));
        }
        self.rebalance_rule.validate()?;
        let mut local_ids = BTreeMap::<String, ()>::new();
        for leg in &self.legs {
            *count += 1;
            if *count > MAX_COMPOSITE_LEGS {
                return Err(Error::Validation(format!(
                    "composite '{}' exceeds maximum total leg count {MAX_COMPOSITE_LEGS}",
                    self.id
                )));
            }
            if !leg.weight.is_finite() || leg.weight.abs() <= MIN_ABS_INPUT {
                return Err(Error::Validation(format!(
                    "composite leg '{}' weight must be finite and non-zero",
                    leg.instrument_id
                )));
            }
            if local_ids
                .insert(leg.instrument_id.to_string(), ())
                .is_some()
            {
                return Err(Error::Validation(format!(
                    "composite '{}' contains duplicate leg id '{}'",
                    self.id, leg.instrument_id
                )));
            }
            let hash = InstrumentEnvelope::new((*leg.instrument).clone()).content_hash()?;
            if let Some(existing) = seen.get(leg.instrument_id.as_str()) {
                if existing != &hash {
                    return Err(Error::Validation(format!(
                        "instrument id '{}' is reused with conflicting definitions",
                        leg.instrument_id
                    )));
                }
            } else {
                seen.insert(leg.instrument_id.to_string(), hash);
            }
            let boxed = leg.instrument.as_ref().clone().into_boxed()?;
            if boxed.id() != leg.instrument_id.as_str() {
                return Err(Error::Validation(format!(
                    "composite leg id '{}' does not match embedded instrument id '{}'",
                    leg.instrument_id,
                    boxed.id()
                )));
            }
            if let InstrumentJson::Composite(nested) = leg.instrument.as_ref() {
                nested.spec.validate_tree(depth + 1, count, seen)?;
                nested.validate_state()?;
            }
        }
        self.validate_weighting()
    }

    fn validate_weighting(&self) -> Result<()> {
        let validate_anchor = |anchor: &InstrumentId, quantity: f64| -> Result<()> {
            if !quantity.is_finite() || quantity.abs() <= MIN_ABS_INPUT {
                return Err(Error::Validation(
                    "composite anchor quantity must be finite and non-zero".to_string(),
                ));
            }
            if !self.legs.iter().any(|leg| leg.instrument_id == *anchor) {
                return Err(Error::Validation(format!(
                    "composite anchor leg '{}' is not present",
                    anchor
                )));
            }
            Ok(())
        };
        match &self.weighting_method {
            WeightingMethod::FixedQuantity => Ok(()),
            WeightingMethod::NotionalWeighted { gross_notional } => {
                if !gross_notional.amount().is_finite()
                    || gross_notional.amount() <= 0.0
                    || gross_notional.currency() != self.reporting_currency
                {
                    return Err(Error::Validation(format!(
                        "gross notional must be positive and denominated in {}",
                        self.reporting_currency
                    )));
                }
                Ok(())
            }
            WeightingMethod::MetricWeighted {
                anchor_leg_id,
                anchor_quantity,
                neutralize,
                ..
            } => {
                validate_anchor(anchor_leg_id, *anchor_quantity)?;
                if *neutralize {
                    let positive = self.legs.iter().any(|leg| leg.weight > 0.0);
                    let negative = self.legs.iter().any(|leg| leg.weight < 0.0);
                    if !(positive && negative) {
                        return Err(Error::Validation(
                            "neutral composite weighting requires both positive and negative scores"
                                .to_string(),
                        ));
                    }
                }
                Ok(())
            }
            WeightingMethod::VolatilityWeighted {
                anchor_leg_id,
                anchor_quantity,
                lookback,
                min_observations,
                annualization_factor,
            } => {
                validate_anchor(anchor_leg_id, *anchor_quantity)?;
                if *lookback == 0
                    || *min_observations < 2
                    || *min_observations > *lookback
                    || !annualization_factor.is_finite()
                    || *annualization_factor <= 0.0
                {
                    return Err(Error::Validation(
                        "volatility weighting requires lookback >= min_observations >= 2 and a positive annualization factor"
                            .to_string(),
                    ));
                }
                Ok(())
            }
            WeightingMethod::UserDefined {
                quantity_expressions,
                ..
            } => {
                if quantity_expressions.len() != self.legs.len() {
                    return Err(Error::Validation(
                        "user-defined weighting requires exactly one expression per leg"
                            .to_string(),
                    ));
                }
                for leg in &self.legs {
                    let expression = quantity_expressions
                        .get(leg.instrument_id.as_str())
                        .ok_or_else(|| {
                            Error::Validation(format!(
                                "missing quantity expression for leg '{}'",
                                leg.instrument_id
                            ))
                        })?;
                    let _ = CompiledExpr::try_new_scalar(expression.clone())?;
                }
                Ok(())
            }
        }
    }

    /// Resolve a fixed-quantity specification without consulting market data.
    ///
    /// Each leg's `weight` becomes its frozen quantity. Dynamic weighting
    /// methods must use [`Self::initialize`] instead.
    ///
    /// Python and WASM expose only [`Self::initialize`]; that path also
    /// resolves [`WeightingMethod::FixedQuantity`] and does not require
    /// historical observations.
    ///
    /// # Arguments
    ///
    /// * `effective_date` - Date from which the fixed quantities are held
    ///   until the next explicit rebalance.
    ///
    /// # Errors
    ///
    /// Returns an error when the specification is invalid or uses a dynamic method.
    pub fn initialize_fixed(&self, effective_date: Date) -> Result<CompositeRebalanceResult> {
        self.validate()?;
        if !matches!(self.weighting_method, WeightingMethod::FixedQuantity) {
            return Err(Error::Validation(
                "initialize_fixed is available only for fixed-quantity composites".to_string(),
            ));
        }
        let quantities: Vec<f64> = self.legs.iter().map(|leg| leg.weight).collect();
        self.build_rebalance_result(effective_date, quantities, IndexMap::new(), None)
    }

    /// Resolve quantities using current and, when required, historical market data.
    ///
    /// Volatility weighting requires `history` to be strictly increasing and
    /// to end on `as_of`. User-defined expressions populate
    /// `leg.{id}.volatility` only when `history` has at least three
    /// observations (two unit-P&L increments), annualized with `sqrt(252)`.
    ///
    /// # Arguments
    ///
    /// * `market` - Complete current market used for unit values, additive
    ///   metrics, notionals, and reporting-currency FX conversion.
    /// * `as_of` - Effective date of the new immutable holdings state; no
    ///   later history observation is permitted.
    /// * `history` - Strictly increasing dated snapshots available through
    ///   `as_of`. Required for volatility weighting; optional otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, missing market data, unsupported
    /// weighting measures, insufficient history, or non-finite resolved quantities.
    pub fn initialize(
        &self,
        market: &MarketContext,
        as_of: Date,
        history: &[CompositeMarketObservation],
    ) -> Result<CompositeRebalanceResult> {
        self.validate()?;
        validate_history(history, Some(as_of))?;
        let (quantities, inputs) = self.resolve_quantities(market, as_of, history)?;
        self.build_rebalance_result(as_of, quantities, inputs, None)
    }

    pub(super) fn build_rebalance_result(
        &self,
        effective_date: Date,
        quantities: Vec<f64>,
        weighting_inputs: IndexMap<String, f64>,
        previous: Option<&CompositeInstrument>,
    ) -> Result<CompositeRebalanceResult> {
        if quantities.len() != self.legs.len() {
            return Err(Error::Internal(
                "composite quantity resolver returned the wrong leg count".to_string(),
            ));
        }
        let resolved_legs: Vec<ResolvedCompositeLeg> = self
            .legs
            .iter()
            .zip(quantities)
            .map(|(leg, quantity)| ResolvedCompositeLeg {
                instrument_id: leg.instrument_id.clone(),
                quantity,
            })
            .collect();
        if resolved_legs
            .iter()
            .any(|leg| !leg.quantity.is_finite() || leg.quantity.abs() <= MIN_ABS_INPUT)
        {
            return Err(Error::Validation(
                "composite resolved quantities must be finite and non-zero".to_string(),
            ));
        }
        let instrument = CompositeInstrument {
            spec: self.clone(),
            state: CompositeState {
                effective_date,
                resolved_legs,
                weighting_inputs,
            },
            boxed_legs: BoxedLegCache::default(),
        };
        instrument.validate_state()?;
        let trades = instrument.execution_trades(previous)?;
        Ok(CompositeRebalanceResult { instrument, trades })
    }

    pub(super) fn resolve_quantities(
        &self,
        market: &MarketContext,
        as_of: Date,
        history: &[CompositeMarketObservation],
    ) -> Result<(Vec<f64>, IndexMap<String, f64>)> {
        match &self.weighting_method {
            WeightingMethod::FixedQuantity => Ok((
                self.legs.iter().map(|leg| leg.weight).collect(),
                IndexMap::new(),
            )),
            WeightingMethod::NotionalWeighted { gross_notional } => {
                self.resolve_notional_weighted(market, as_of, *gross_notional)
            }
            WeightingMethod::MetricWeighted {
                metric,
                anchor_leg_id,
                anchor_quantity,
                neutralize,
            } => self.resolve_metric_weighted(
                market,
                as_of,
                metric,
                anchor_leg_id,
                *anchor_quantity,
                *neutralize,
            ),
            WeightingMethod::VolatilityWeighted {
                anchor_leg_id,
                anchor_quantity,
                lookback,
                min_observations,
                annualization_factor,
            } => self.resolve_volatility_weighted(
                as_of,
                history,
                VolatilityWeightingConfig {
                    anchor_leg_id,
                    anchor_quantity: *anchor_quantity,
                    lookback: *lookback,
                    min_observations: *min_observations,
                    annualization_factor: *annualization_factor,
                },
            ),
            WeightingMethod::UserDefined {
                required_metrics,
                quantity_expressions,
            } => self.resolve_user_defined(
                market,
                as_of,
                history,
                required_metrics,
                quantity_expressions,
            ),
        }
    }

    fn resolve_notional_weighted(
        &self,
        market: &MarketContext,
        as_of: Date,
        gross_notional: Money,
    ) -> Result<(Vec<f64>, IndexMap<String, f64>)> {
        let score_total = self.legs.iter().map(|leg| leg.weight.abs()).sum::<f64>();
        if !score_total.is_finite() || score_total <= MIN_ABS_INPUT {
            return Err(Error::Validation(
                "composite absolute score total must be finite and non-zero".to_string(),
            ));
        }
        let mut notionals = Vec::with_capacity(self.legs.len());
        let mut inputs = IndexMap::new();
        for leg in &self.legs {
            let instrument = leg.instrument.as_ref().clone().into_boxed()?;
            let notional = instrument.notional()?.ok_or_else(|| {
                Error::Validation(format!(
                    "instrument '{}' does not expose a weighting notional",
                    leg.instrument_id
                ))
            })?;
            let converted = convert_amount(
                market,
                notional.amount(),
                notional.currency(),
                self.reporting_currency,
                as_of,
            )?
            .abs();
            if !converted.is_finite() || converted <= MIN_ABS_INPUT {
                return Err(Error::Validation(format!(
                    "instrument '{}' has zero or non-finite weighting notional",
                    leg.instrument_id
                )));
            }
            inputs.insert(format!("leg.{}.notional", leg.instrument_id), converted);
            notionals.push(converted);
        }
        let quantities = self
            .legs
            .iter()
            .zip(notionals)
            .map(|(leg, notional)| {
                leg.weight.signum() * gross_notional.amount() * leg.weight.abs()
                    / score_total
                    / notional
            })
            .collect();
        Ok((quantities, inputs))
    }

    fn resolve_metric_weighted(
        &self,
        market: &MarketContext,
        as_of: Date,
        metric: &MetricId,
        anchor_leg_id: &InstrumentId,
        anchor_quantity: f64,
        neutralize: bool,
    ) -> Result<(Vec<f64>, IndexMap<String, f64>)> {
        let scores = normalized_scores(&self.legs, neutralize)?;
        let mut measures = Vec::with_capacity(self.legs.len());
        let mut inputs = IndexMap::new();
        for leg in &self.legs {
            let instrument = leg.instrument.as_ref().clone().into_boxed()?;
            let result = instrument.price_with_metrics(
                market,
                as_of,
                std::slice::from_ref(metric),
                PricingOptions::default(),
            )?;
            let mut value = result.measures.get(metric).copied().ok_or_else(|| {
                Error::Validation(format!(
                    "metric '{}' is not available for composite leg '{}'",
                    metric, leg.instrument_id
                ))
            })?;
            if is_additive_metric(metric) {
                value = convert_amount(
                    market,
                    value,
                    result.value.currency(),
                    self.reporting_currency,
                    as_of,
                )?;
            }
            if !value.is_finite() || value.abs() <= MIN_ABS_INPUT {
                return Err(Error::Validation(format!(
                    "metric '{}' is zero or non-finite for composite leg '{}'",
                    metric, leg.instrument_id
                )));
            }
            inputs.insert(
                format!("leg.{}.metric.{}", leg.instrument_id, metric),
                value,
            );
            measures.push(value);
        }
        let anchor_index = leg_index(&self.legs, anchor_leg_id)?;
        let anchor_score = scores[anchor_index];
        let anchor_measure = measures[anchor_index];
        if (anchor_quantity * anchor_measure).signum() != anchor_score.signum() {
            return Err(Error::Validation(
                "anchor quantity and unit metric must have the anchor score's sign".to_string(),
            ));
        }
        let anchor_contribution = anchor_quantity * anchor_measure;
        let quantities = scores
            .iter()
            .zip(measures)
            .map(|(score, measure)| (score / anchor_score) * anchor_contribution / measure)
            .collect();
        Ok((quantities, inputs))
    }

    fn resolve_volatility_weighted(
        &self,
        as_of: Date,
        history: &[CompositeMarketObservation],
        config: VolatilityWeightingConfig<'_>,
    ) -> Result<(Vec<f64>, IndexMap<String, f64>)> {
        if history
            .last()
            .is_none_or(|observation| observation.date != as_of)
        {
            return Err(Error::Validation(
                "volatility weighting history must end on the rebalance date".to_string(),
            ));
        }
        let mut volatilities = Vec::with_capacity(self.legs.len());
        let mut inputs = IndexMap::new();
        for leg in &self.legs {
            let pnl = unit_pnl_series(leg.instrument.as_ref(), self.reporting_currency, history)?;
            let start = pnl.len().saturating_sub(config.lookback);
            let sample = &pnl[start..];
            if sample.len() < config.min_observations {
                return Err(Error::Validation(format!(
                    "leg '{}' has {} P&L observations; {} required",
                    leg.instrument_id,
                    sample.len(),
                    config.min_observations
                )));
            }
            let volatility = sample_std_dev(sample)? * config.annualization_factor.sqrt();
            if !volatility.is_finite() || volatility <= MIN_ABS_INPUT {
                return Err(Error::Validation(format!(
                    "leg '{}' has zero or non-finite unit-P&L volatility",
                    leg.instrument_id
                )));
            }
            inputs.insert(format!("leg.{}.volatility", leg.instrument_id), volatility);
            volatilities.push(volatility);
        }
        let anchor_index = leg_index(&self.legs, config.anchor_leg_id)?;
        let anchor_score = self.legs[anchor_index].weight;
        if config.anchor_quantity.signum() != anchor_score.signum() {
            return Err(Error::Validation(
                "anchor quantity must have the anchor score's sign".to_string(),
            ));
        }
        let anchor_volatility = volatilities[anchor_index];
        let quantities = self
            .legs
            .iter()
            .zip(volatilities)
            .map(|(leg, volatility)| {
                (leg.weight / anchor_score) * config.anchor_quantity * anchor_volatility
                    / volatility
            })
            .collect();
        Ok((quantities, inputs))
    }

    fn resolve_user_defined(
        &self,
        market: &MarketContext,
        as_of: Date,
        history: &[CompositeMarketObservation],
        required_metrics: &[MetricId],
        quantity_expressions: &IndexMap<String, Expr>,
    ) -> Result<(Vec<f64>, IndexMap<String, f64>)> {
        let mut columns = IndexMap::<String, f64>::new();
        columns.insert("as_of_days".to_string(), f64::from(as_of.to_julian_day()));
        for leg in &self.legs {
            let instrument = leg.instrument.as_ref().clone().into_boxed()?;
            let result = instrument.price_with_metrics(
                market,
                as_of,
                required_metrics,
                PricingOptions::default(),
            )?;
            let value = convert_amount(
                market,
                result.value.amount(),
                result.value.currency(),
                self.reporting_currency,
                as_of,
            )?;
            let fx_rate = convert_amount(
                market,
                1.0,
                result.value.currency(),
                self.reporting_currency,
                as_of,
            )?;
            columns.insert(format!("leg.{}.weight", leg.instrument_id), leg.weight);
            columns.insert(format!("leg.{}.value", leg.instrument_id), value);
            columns.insert(format!("leg.{}.fx_rate", leg.instrument_id), fx_rate);
            if let Some(notional) = instrument.notional()? {
                let converted = convert_amount(
                    market,
                    notional.amount(),
                    notional.currency(),
                    self.reporting_currency,
                    as_of,
                )?;
                columns.insert(format!("leg.{}.notional", leg.instrument_id), converted);
            }
            for metric in required_metrics {
                let mut value = result.measures.get(metric).copied().ok_or_else(|| {
                    Error::Validation(format!(
                        "required metric '{}' is unavailable for leg '{}'",
                        metric, leg.instrument_id
                    ))
                })?;
                if is_additive_metric(metric) {
                    value *= fx_rate;
                }
                columns.insert(
                    format!("leg.{}.metric.{}", leg.instrument_id, metric),
                    value,
                );
            }
            if history.len() >= 3 {
                let pnl =
                    unit_pnl_series(leg.instrument.as_ref(), self.reporting_currency, history)?;
                let volatility = sample_std_dev(&pnl)? * 252.0_f64.sqrt();
                columns.insert(format!("leg.{}.volatility", leg.instrument_id), volatility);
            }
        }
        let names: Vec<String> = columns.keys().cloned().collect();
        let data: Vec<Vec<f64>> = columns.values().map(|value| vec![*value]).collect();
        let refs: Vec<&[f64]> = data.iter().map(Vec::as_slice).collect();
        let context = SimpleContext::new(names)?;
        let mut quantities = Vec::with_capacity(self.legs.len());
        for leg in &self.legs {
            let expression = quantity_expressions
                .get(leg.instrument_id.as_str())
                .ok_or_else(|| {
                    Error::Validation(format!(
                        "missing quantity expression for leg '{}'",
                        leg.instrument_id
                    ))
                })?;
            let compiled = CompiledExpr::try_new_scalar(expression.clone())?;
            let evaluated = compiled.eval(&context, &refs, EvalOpts::default())?;
            let quantity = evaluated.values.first().copied().ok_or_else(|| {
                Error::Validation(format!(
                    "quantity expression for leg '{}' returned no value",
                    leg.instrument_id
                ))
            })?;
            quantities.push(quantity);
        }
        Ok((quantities, columns))
    }
}
