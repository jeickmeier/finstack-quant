//! Execution, exposure, and valuation reporting types for composites.

use super::instrument::CompositeInstrument;
use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use crate::instruments::MetricPricingOverrides;
use crate::results::composite::{CompositeExposureReport, PrimitiveAggregate, PrimitiveExposure};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_core::{Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

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
///
/// On the wire, `instrument` is the canonical `finstack_quant.instrument/1`
/// envelope, so it can be passed straight back to any composite or pricing
/// entry point that takes an instrument envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CompositeRebalanceResult {
    /// Newly resolved, immutable, priceable composite, serialized as its
    /// canonical instrument envelope.
    #[serde(with = "composite_envelope")]
    #[cfg_attr(feature = "json-schema", schemars(with = "InstrumentEnvelope"))]
    pub instrument: CompositeInstrument,
    /// Net primitive execution deltas required to reach the new state.
    pub trades: Vec<CompositeTrade>,
}

/// Serde adapter writing a [`CompositeInstrument`] as its canonical
/// instrument envelope and reading it back through the validating loader.
mod composite_envelope {
    use super::CompositeInstrument;
    use crate::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S: Serializer>(
        instrument: &CompositeInstrument,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        InstrumentEnvelope::new(InstrumentJson::from(instrument.clone())).serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<CompositeInstrument, D::Error> {
        let envelope = InstrumentEnvelope::deserialize(deserializer)?;
        envelope
            .instrument
            .validate_for_pricing()
            .map_err(serde::de::Error::custom)?;
        CompositeInstrument::try_from(envelope.instrument).map_err(serde::de::Error::custom)
    }
}

pub(super) fn flatten_composite(
    composite: &CompositeInstrument,
    multiplier: f64,
    inherited_metric_defaults: &MetricPricingOverrides,
    path: &mut Vec<String>,
    out: &mut Vec<PrimitiveExposure>,
) -> Result<()> {
    let mut metric_defaults = composite.metric_pricing_overrides.clone();
    metric_defaults.apply_defaults(inherited_metric_defaults);
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
                flatten_composite(nested, quantity, &metric_defaults, path, out)?;
            }
            primitive => out.push(PrimitiveExposure {
                path: path.clone(),
                instrument_id: leg.instrument_id.clone(),
                instrument_type: primitive.type_tag().to_string(),
                quantity,
                value: Money::from((0_i64, composite.spec.reporting_currency)),
                measures: IndexMap::new(),
                instrument: Some(Arc::from(primitive.clone().into_boxed_unchecked()?)),
                metric_defaults: metric_defaults.clone(),
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
    let definition = exposure
        .instrument()
        .and_then(crate::instruments::json_loader::instrument_json_of)
        .ok_or_else(|| {
            Error::Internal("primitive exposure lost its runtime instrument".to_string())
        })?;
    let hash = InstrumentEnvelope::new(definition).content_hash()?;
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
