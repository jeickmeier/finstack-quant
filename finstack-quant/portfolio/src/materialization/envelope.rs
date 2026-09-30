//! Strict wire types for portfolio materialization bundles.

use crate::book::{Book, BookId};
use crate::position::PositionUnit;
use crate::types::{AttributeValue, Entity, EntityId, PositionId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::wire::PercentageQuantityWire;
use finstack_quant_valuations::instruments::{InstrumentEnvelope, MarketDependencies};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Sole supported portfolio-materialization contract marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum PortfolioMaterializationSchema {
    /// Canonical v1 materialization contract.
    #[serde(rename = "finstack_quant.portfolio_materialization/1")]
    Materialization,
}

impl PortfolioMaterializationSchema {
    /// The exact marker required by every persisted materialization envelope.
    pub const CURRENT: Self = Self::Materialization;
}

/// Strict, versioned portfolio materialization bundle.
///
/// Unlike [`crate::portfolio::PortfolioSpec`], this database-oriented format
/// stores each instrument artifact once and lets ordered lightweight positions
/// reference the artifact by ID.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioMaterializationEnvelope {
    /// Exact materialization contract marker.
    pub schema: PortfolioMaterializationSchema,
    /// Portfolio fields that do not contain runtime instrument trait objects.
    pub portfolio: PortfolioHeader,
    /// Unique strict instrument envelopes referenced by positions.
    pub instruments: Vec<InstrumentArtifact>,
    /// Positions in the order required by the reconstructed portfolio.
    pub positions: Vec<MaterializedPosition>,
    /// Optional producer and compiler version provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materializer: Option<MaterializerInfo>,
}

/// Portfolio fields shared by every materialized position.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct PortfolioHeader {
    /// Stable portfolio identifier.
    pub id: String,
    /// Optional human-readable portfolio name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Reporting currency used for portfolio aggregation.
    pub base_currency: Currency,
    /// Valuation date for the materialized portfolio.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub as_of: Date,
    /// Entities keyed by their stable IDs in deterministic order.
    pub entities: IndexMap<EntityId, Entity>,
    /// Optional book hierarchy keyed by stable book IDs.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub books: IndexMap<BookId, Book>,
    /// Portfolio-level grouping and classification tags.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tags: IndexMap<String, String>,
    /// Extension metadata retained as part of the persisted bundle.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

/// One unique, content-addressed instrument artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct InstrumentArtifact {
    /// Immutable producer revision ID or content-addressed artifact ID.
    pub artifact_id: String,
    /// Optional claimed canonical digest, verified before decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Full typed, strict instrument envelope.
    pub envelope: InstrumentEnvelope,
    /// Optional producer dependency claim, checked against runtime extraction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<MarketDependencies>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
enum MaterializedPositionWire {
    Percentage(PercentageMaterializedPositionWire),
    NonPercentage(NonPercentageMaterializedPositionWire),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct PercentageMaterializedPositionWire {
    /// Position identifier, unique within the materialized portfolio.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    id: PositionId,
    /// Legal entity or book the position belongs to, used for rollups.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    entity_id: EntityId,
    /// Identifier of the instrument this position holds.
    instrument_id: String,
    /// Identifier of the compiled artifact the instrument was resolved from.
    artifact_id: String,
    /// Holding size expressed as a percentage of the referenced notional.
    quantity: PercentageQuantityWire,
    /// Unit discriminator; always `percentage` for this variant.
    unit: PercentagePositionUnitWire,
    /// Typed classification tags used for grouping and scenario selection.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    attributes: IndexMap<String, AttributeValue>,
    /// Free-form metadata carried through unchanged; not interpreted here.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    meta: IndexMap<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
struct NonPercentageMaterializedPositionWire {
    /// Position identifier, unique within the materialized portfolio.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    id: PositionId,
    /// Legal entity or book the position belongs to, used for rollups.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    entity_id: EntityId,
    /// Identifier of the instrument this position holds.
    instrument_id: String,
    /// Identifier of the compiled artifact the instrument was resolved from.
    artifact_id: String,
    /// Holding size in the unit named by `unit`. Negative values are short
    /// positions.
    quantity: f64,
    /// Unit `quantity` is expressed in, such as notional, shares, or contracts.
    unit: NonPercentagePositionUnitWire,
    /// Typed classification tags used for grouping and scenario selection.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    attributes: IndexMap<String, AttributeValue>,
    /// Free-form metadata carried through unchanged; not interpreted here.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    meta: IndexMap<String, serde_json::Value>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum PercentagePositionUnitWire {
    Percentage,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged, deny_unknown_fields)]
enum NonPercentagePositionUnitWire {
    Named(NonPercentagePositionUnitName),
    Notional {
        #[cfg_attr(feature = "json-schema", schemars(required))]
        notional: NullableCurrencyWire,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(untagged)]
enum NullableCurrencyWire {
    Currency(Currency),
    Null(()),
}

impl From<Option<Currency>> for NullableCurrencyWire {
    fn from(currency: Option<Currency>) -> Self {
        match currency {
            Some(currency) => Self::Currency(currency),
            None => Self::Null(()),
        }
    }
}

impl From<NullableCurrencyWire> for Option<Currency> {
    fn from(currency: NullableCurrencyWire) -> Self {
        match currency {
            NullableCurrencyWire::Currency(currency) => Some(currency),
            NullableCurrencyWire::Null(()) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
enum NonPercentagePositionUnitName {
    Units,
    FaceValue,
}

/// Lightweight position referencing a unique instrument artifact.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(from = "MaterializedPositionWire")]
#[cfg_attr(feature = "json-schema", schemars(with = "MaterializedPositionWire"))]
pub struct MaterializedPosition {
    /// Stable position identifier.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub id: PositionId,
    /// Stable ID of the entity that owns the position.
    #[cfg_attr(feature = "json-schema", schemars(with = "String"))]
    pub entity_id: EntityId,
    /// Instrument identifier exposed by portfolio lookup and reports.
    pub instrument_id: String,
    /// Artifact ID resolved against
    /// [`PortfolioMaterializationEnvelope::instruments`].
    pub artifact_id: String,
    /// Signed holding quantity interpreted according to [`PositionUnit`].
    pub quantity: f64,
    /// Scaling convention applied to `quantity`.
    pub unit: PositionUnit,
    /// Position attributes used for grouping, filtering, and constraints.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub attributes: IndexMap<String, AttributeValue>,
    /// Extension metadata retained with the position.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

impl From<MaterializedPositionWire> for MaterializedPosition {
    fn from(value: MaterializedPositionWire) -> Self {
        match value {
            MaterializedPositionWire::Percentage(position) => Self {
                id: position.id,
                entity_id: position.entity_id,
                instrument_id: position.instrument_id,
                artifact_id: position.artifact_id,
                quantity: position.quantity.into_inner(),
                unit: PositionUnit::Percentage,
                attributes: position.attributes,
                meta: position.meta,
            },
            MaterializedPositionWire::NonPercentage(position) => Self {
                id: position.id,
                entity_id: position.entity_id,
                instrument_id: position.instrument_id,
                artifact_id: position.artifact_id,
                quantity: position.quantity,
                unit: match position.unit {
                    NonPercentagePositionUnitWire::Named(NonPercentagePositionUnitName::Units) => {
                        PositionUnit::Units
                    }
                    NonPercentagePositionUnitWire::Named(
                        NonPercentagePositionUnitName::FaceValue,
                    ) => PositionUnit::FaceValue,
                    NonPercentagePositionUnitWire::Notional { notional } => {
                        PositionUnit::Notional(notional.into())
                    }
                },
                attributes: position.attributes,
                meta: position.meta,
            },
        }
    }
}

impl TryFrom<&MaterializedPosition> for MaterializedPositionWire {
    type Error = finstack_quant_core::Error;

    fn try_from(position: &MaterializedPosition) -> Result<Self, Self::Error> {
        let common = || {
            (
                position.id.clone(),
                position.entity_id.clone(),
                position.instrument_id.clone(),
                position.artifact_id.clone(),
                position.attributes.clone(),
                position.meta.clone(),
            )
        };

        match position.unit {
            PositionUnit::Percentage => {
                let (id, entity_id, instrument_id, artifact_id, attributes, meta) = common();
                Ok(Self::Percentage(PercentageMaterializedPositionWire {
                    id,
                    entity_id,
                    instrument_id,
                    artifact_id,
                    quantity: PercentageQuantityWire::try_from(position.quantity)?,
                    unit: PercentagePositionUnitWire::Percentage,
                    attributes,
                    meta,
                }))
            }
            unit => {
                if !position.quantity.is_finite() {
                    return Err(finstack_quant_core::Error::Validation(format!(
                        "materialized position quantity must be finite, got {}",
                        position.quantity
                    )));
                }
                let unit = match unit {
                    PositionUnit::Units => {
                        NonPercentagePositionUnitWire::Named(NonPercentagePositionUnitName::Units)
                    }
                    PositionUnit::FaceValue => NonPercentagePositionUnitWire::Named(
                        NonPercentagePositionUnitName::FaceValue,
                    ),
                    PositionUnit::Notional(notional) => NonPercentagePositionUnitWire::Notional {
                        notional: notional.into(),
                    },
                    PositionUnit::Percentage => {
                        return Err(finstack_quant_core::Error::Internal(
                            "percentage position reached non-percentage serialization branch"
                                .to_string(),
                        ));
                    }
                };
                let (id, entity_id, instrument_id, artifact_id, attributes, meta) = common();
                Ok(Self::NonPercentage(NonPercentageMaterializedPositionWire {
                    id,
                    entity_id,
                    instrument_id,
                    artifact_id,
                    quantity: position.quantity,
                    unit,
                    attributes,
                    meta,
                }))
            }
        }
    }
}

impl Serialize for MaterializedPosition {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        MaterializedPositionWire::try_from(self)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

/// Producer and compiler version stamps for reproducibility.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct MaterializerInfo {
    /// Stable producer implementation name.
    pub producer: String,
    /// Version of the producer that assembled the bundle.
    pub producer_version: String,
    /// Optional version of the artifact compiler used by the producer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiler_version: Option<String>,
    /// Finstack version targeted by the compiled artifacts.
    pub finstack_version: String,
}
