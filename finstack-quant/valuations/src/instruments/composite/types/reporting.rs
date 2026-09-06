//! Execution, exposure, and valuation reporting types for composites.

use super::instrument::CompositeInstrument;
use super::spec_support::ResolvedCompositeLeg;
use crate::instruments::{InstrumentEnvelope, InstrumentJson};
use crate::metrics::MetricId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Primitive execution delta produced by initialization or rebalance.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeTrade {
    /// Primitive instrument identifier.
    pub instrument_id: InstrumentId,
    /// Canonical primitive instrument type discriminator.
    pub instrument_type: String,
    /// Signed change in primitive quantity.
    pub quantity_delta: f64,
}

/// Result of resolving a new composite holdings state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeRebalanceResult {
    /// Newly resolved, immutable, priceable composite.
    pub instrument: CompositeInstrument,
    /// Net primitive execution deltas required to reach the new state.
    pub trades: Vec<CompositeTrade>,
}

/// Path-level primitive exposure in a resolved composite tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrimitiveExposure {
    /// Composite and leg identifiers from the root to the primitive.
    pub path: Vec<String>,
    /// Primitive instrument identifier.
    pub instrument_id: InstrumentId,
    /// Canonical primitive instrument type discriminator.
    pub instrument_type: String,
    /// Signed primitive quantity after multiplying every nested state quantity.
    pub quantity: f64,
    /// Reporting-currency signed value for this path.
    pub value: Money,
    /// Reporting-currency additive risk measures for this path.
    pub measures: IndexMap<MetricId, f64>,
    #[serde(skip)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    pub(crate) instrument: Option<InstrumentJson>,
}

impl PrimitiveExposure {
    /// Borrow the canonical primitive definition retained for recursive consumers.
    ///
    /// Runtime definitions are intentionally omitted from serialized exposure
    /// reports; callers use this accessor only while processing a live report.
    ///
    /// # Returns
    ///
    /// The embedded primitive definition when this path was created by a live
    /// composite decomposition.
    #[must_use]
    pub fn instrument_definition(&self) -> Option<&InstrumentJson> {
        self.instrument.as_ref()
    }
}

/// Net and gross exposure aggregated by primitive instrument identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PrimitiveAggregate {
    /// Primitive instrument identifier.
    pub instrument_id: InstrumentId,
    /// Canonical primitive instrument type discriminator.
    pub instrument_type: String,
    /// Algebraic sum of path quantities.
    pub net_quantity: f64,
    /// Sum of absolute path quantities.
    pub gross_quantity: f64,
    /// Algebraic reporting-currency value.
    pub net_value: Money,
    /// Sum of absolute reporting-currency path values.
    pub gross_value: Money,
    /// Algebraic additive risk by metric.
    pub net_measures: IndexMap<MetricId, f64>,
    /// Sum of absolute path risk by metric.
    pub gross_measures: IndexMap<MetricId, f64>,
}

/// Complete primitive decomposition with path, net, and gross views.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeExposureReport {
    /// Composite reporting currency.
    pub reporting_currency: Currency,
    /// Every primitive path before overlap netting.
    pub paths: Vec<PrimitiveExposure>,
    /// Primitive aggregates ordered by identifier.
    pub aggregates: Vec<PrimitiveAggregate>,
}

/// Rich structured details attached to composite valuation results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeValuationDetails {
    /// Effective date of the frozen holdings state used for pricing.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub state_effective_date: Date,
    /// Currency of all reported values and additive risk measures.
    pub reporting_currency: Currency,
    /// Frozen top-level quantities used for this valuation.
    pub resolved_legs: Vec<ResolvedCompositeLeg>,
    /// Scalar inputs retained when the state was resolved.
    pub weighting_inputs: IndexMap<String, f64>,
    /// Native and reporting-currency results for every top-level leg.
    pub leg_results: Vec<CompositeLegValuation>,
    /// Recursive path-level and net/gross primitive exposures.
    pub exposure_report: CompositeExposureReport,
}

/// One top-level leg result retained in composite valuation details.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeLegValuation {
    /// Identifier of the top-level leg specification.
    pub instrument_id: InstrumentId,
    /// Signed frozen leg quantity applied to the unit valuation.
    pub quantity: f64,
    /// Quantity-scaled value in the underlying instrument's native currency.
    pub native_value: Money,
    /// Quantity-scaled value converted to the composite reporting currency.
    pub reporting_value: Money,
    /// Complete unit-instrument valuation, including nested details where applicable.
    pub valuation: crate::results::ValuationResult,
}

pub(super) fn flatten_composite(
    composite: &CompositeInstrument,
    multiplier: f64,
    path: &mut Vec<String>,
    out: &mut Vec<PrimitiveExposure>,
) -> Result<()> {
    for (leg, state) in composite
        .spec
        .legs
        .iter()
        .zip(&composite.state.resolved_legs)
    {
        path.push(leg.instrument_id.to_string());
        let quantity = multiplier * state.quantity;
        match leg.instrument.as_ref() {
            InstrumentJson::Composite(nested) => {
                flatten_composite(nested, quantity, path, out)?;
            }
            primitive => out.push(PrimitiveExposure {
                path: path.clone(),
                instrument_id: leg.instrument_id.clone(),
                instrument_type: primitive.type_tag().to_string(),
                quantity,
                value: Money::from((0_i64, composite.spec.reporting_currency)),
                measures: IndexMap::new(),
                instrument: Some(primitive.clone()),
            }),
        }
        let _ = path.pop();
    }
    Ok(())
}

pub(super) fn register_execution_definition(
    definitions: &mut BTreeMap<String, String>,
    exposure: &PrimitiveExposure,
) -> Result<()> {
    let definition = exposure.instrument_definition().ok_or_else(|| {
        Error::Internal("primitive exposure lost its runtime instrument".to_string())
    })?;
    let hash = InstrumentEnvelope::new(definition.clone()).content_hash()?;
    match definitions.get(exposure.instrument_id.as_str()) {
        Some(existing) if existing != &hash => Err(Error::Validation(format!(
            "primitive instrument id '{}' has conflicting definitions across composite states",
            exposure.instrument_id
        ))),
        Some(_) => Ok(()),
        None => {
            definitions.insert(exposure.instrument_id.to_string(), hash);
            Ok(())
        }
    }
}

pub(super) fn aggregate_primitive_paths(
    reporting_currency: Currency,
    paths: Vec<PrimitiveExposure>,
) -> finstack_quant_core::Result<CompositeExposureReport> {
    let mut aggregate = BTreeMap::<String, PrimitiveAggregate>::new();
    for path in &paths {
        let entry = aggregate
            .entry(path.instrument_id.to_string())
            .or_insert_with(|| PrimitiveAggregate {
                instrument_id: path.instrument_id.clone(),
                instrument_type: path.instrument_type.clone(),
                net_quantity: 0.0,
                gross_quantity: 0.0,
                net_value: Money::from((0_i64, reporting_currency)),
                gross_value: Money::from((0_i64, reporting_currency)),
                net_measures: IndexMap::new(),
                gross_measures: IndexMap::new(),
            });
        entry.net_quantity += path.quantity;
        entry.gross_quantity += path.quantity.abs();
        entry.net_value = Money::new(
            entry.net_value.amount() + path.value.amount(),
            reporting_currency,
        )?;
        entry.gross_value = Money::new(
            entry.gross_value.amount() + path.value.amount().abs(),
            reporting_currency,
        )?;
        for (metric, value) in &path.measures {
            *entry.net_measures.entry(metric.clone()).or_default() += *value;
            *entry.gross_measures.entry(metric.clone()).or_default() += value.abs();
        }
    }
    Ok(CompositeExposureReport {
        reporting_currency,
        paths,
        aggregates: aggregate.into_values().collect(),
    })
}
