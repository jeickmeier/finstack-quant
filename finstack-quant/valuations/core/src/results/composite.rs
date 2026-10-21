//! Composite valuation details: resolved legs, leg valuations and the
//! primitive exposure report.
//!
//! These are the payload of
//! [`ValuationDetails::Composite`](crate::results::ValuationDetails). A
//! composite instrument fills them while pricing its legs; a path's runtime
//! instrument rides along for live consumers but is never serialized.

use std::sync::Arc;

use crate::instruments::pricing_overrides::MetricPricingOverrides;
use crate::instruments::Instrument;
use crate::metrics::MetricId;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::InstrumentId;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Immutable resolved quantity for one top-level composite leg.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ResolvedCompositeLeg {
    /// Identifier of the corresponding leg specification.
    pub instrument_id: InstrumentId,
    /// Signed quantity held from the state's effective date until the next rebalance.
    pub quantity: f64,
}

/// Path-level primitive exposure in a resolved composite tree.
#[derive(Clone, Serialize, Deserialize)]
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
    /// Runtime instrument of this path, retained for recursive consumers of
    /// a live decomposition and never serialized.
    #[serde(skip)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    pub instrument: Option<Arc<dyn Instrument>>,
    /// Metric overrides inherited from the enclosing composites (innermost wins),
    /// applied to the primitive as defaults when it is priced; never serialized.
    #[serde(skip)]
    #[cfg_attr(feature = "json-schema", schemars(skip))]
    pub metric_defaults: MetricPricingOverrides,
}

impl PrimitiveExposure {
    /// The runtime instrument of this path, when the report came from a live
    /// composite decomposition.
    ///
    /// Runtime instruments are not serialized with the report; a deserialized
    /// report has none.
    #[must_use]
    pub fn instrument(&self) -> Option<&dyn Instrument> {
        self.instrument.as_deref()
    }
}

impl std::fmt::Debug for PrimitiveExposure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrimitiveExposure")
            .field("path", &self.path)
            .field("instrument_id", &self.instrument_id)
            .field("instrument_type", &self.instrument_type)
            .field("quantity", &self.quantity)
            .field("value", &self.value)
            .field("measures", &self.measures)
            .field(
                "instrument",
                &self.instrument.as_ref().map(|i| i.id().to_string()),
            )
            .field("metric_defaults", &self.metric_defaults)
            .finish()
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
    pub exposures: CompositeExposureReport,
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
