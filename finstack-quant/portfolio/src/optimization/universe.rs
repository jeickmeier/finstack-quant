//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use crate::position::PositionUnit;
use crate::types::{AttributeTest, AttributeValue, EntityId, PositionId};
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Filters for selecting which positions are included in a rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PositionFilter {
    /// All positions in the portfolio.
    All,

    /// Filter by entity ID.
    ByEntityId(EntityId),

    /// Filter by attribute test (text equality, numeric comparison, etc.).
    ByAttribute(AttributeTest),

    /// Filter by multiple position IDs.
    ByPositionIds(Vec<PositionId>),

    /// Exclude positions matching the inner filter.
    Not(Box<PositionFilter>),

    /// Conjunction: positions matching ALL inner filters.
    And(Vec<PositionFilter>),

    /// Disjunction: positions matching ANY inner filter.
    Or(Vec<PositionFilter>),
}

impl PositionFilter {
    /// Evaluate this filter against a position-like item.
    ///
    /// The caller provides the item's entity ID, position ID, and attributes.
    /// This avoids duplicating the recursive filter logic for every position-like type.
    pub fn matches(
        &self,
        entity_id: &EntityId,
        position_id: &PositionId,
        attributes: &IndexMap<String, AttributeValue>,
    ) -> bool {
        match self {
            Self::All => true,
            Self::ByEntityId(id) => entity_id == id,
            Self::ByAttribute(test) => test.evaluate(attributes),
            Self::ByPositionIds(ids) => ids.contains(position_id),
            Self::Not(inner) => !inner.matches(entity_id, position_id, attributes),
            Self::And(filters) => filters
                .iter()
                .all(|f| f.matches(entity_id, position_id, attributes)),
            Self::Or(filters) => filters
                .iter()
                .any(|f| f.matches(entity_id, position_id, attributes)),
        }
    }
}

/// A candidate instrument that could be added to the portfolio.
///
/// This represents an instrument not currently held but available for trading.
/// The optimizer can allocate weight to candidates (up to `max_weight`).
#[derive(Clone)]
pub struct CandidatePosition {
    /// Identifier that becomes `PositionId` if traded. Must be unique among
    /// candidates and absent from the existing portfolio; optimization rejects
    /// a collision.
    pub id: PositionId,

    /// Entity that would own this position.
    pub entity_id: EntityId,

    /// The instrument that could be traded.
    pub instrument: Arc<dyn Instrument>,

    /// Unit type for quantity interpretation.
    pub unit: PositionUnit,

    /// Attributes for the candidate (used in constraints like exposure limits).
    pub attributes: IndexMap<String, AttributeValue>,

    /// Maximum weight this candidate can receive (default: 1.0 = no limit).
    /// Useful for limiting exposure to any single new position.
    pub max_weight: f64,

    /// Minimum weight if included (for minimum position size constraints).
    /// Set to 0.0 to allow the optimizer to skip this candidate entirely.
    pub min_weight: f64,
}

impl CandidatePosition {
    /// Create a new candidate position.
    ///
    /// # Arguments
    ///
    /// * `id` - Identifier that will become the optimized `PositionId` if selected.
    ///   Must be unique among candidates and absent from the existing portfolio.
    /// * `entity_id` - Owning entity for the candidate.
    /// * `instrument` - Candidate instrument to trade.
    /// * `unit` - Quantity semantics for the candidate.
    ///
    /// # Returns
    ///
    /// Candidate with empty attributes, `max_weight = 1.0`, and `min_weight = 0.0`.
    pub fn new(
        id: impl Into<PositionId>,
        entity_id: impl Into<EntityId>,
        instrument: Arc<dyn Instrument>,
        unit: PositionUnit,
    ) -> Self {
        Self {
            id: id.into(),
            entity_id: entity_id.into(),
            instrument,
            unit,
            attributes: IndexMap::new(),
            max_weight: 1.0,
            min_weight: 0.0,
        }
    }

    /// Add an attribute to the candidate.
    ///
    /// Accepts any value convertible into [`AttributeValue`] — `&str`/`String`
    /// for text attributes (e.g. ratings, sectors) and `f64` for numeric
    /// attributes (e.g. credit scores, ESG scores). This single method
    /// replaces the earlier `with_text_attribute` / `with_numeric_attribute`
    /// pair.
    ///
    /// # Arguments
    ///
    /// * `key` - Attribute key.
    /// * `value` - Attribute value (text or numeric).
    ///
    /// # Returns
    ///
    /// The updated candidate for fluent chaining.
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Set maximum weight for this candidate.
    ///
    /// # Arguments
    ///
    /// * `max` - Maximum admissible weight.
    ///
    /// # Returns
    ///
    /// The updated candidate for fluent chaining.
    #[must_use]
    pub fn with_max_weight(mut self, max: f64) -> Self {
        self.max_weight = max;
        self
    }

    /// Set minimum weight (if included) for this candidate.
    ///
    /// # Arguments
    ///
    /// * `min` - Minimum admissible weight when the candidate is included.
    ///
    /// # Returns
    ///
    /// The updated candidate for fluent chaining.
    #[must_use]
    pub fn with_min_weight(mut self, min: f64) -> Self {
        self.min_weight = min;
        self
    }
}

/// Wire form of [`CandidatePosition`]: the instrument travels as its
/// canonical tagged JSON payload instead of a trait object.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidatePositionSpec {
    id: PositionId,
    entity_id: EntityId,
    instrument_spec: InstrumentJson,
    unit: PositionUnit,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    attributes: IndexMap<String, AttributeValue>,
    max_weight: f64,
    min_weight: f64,
}

impl TryFrom<CandidatePositionSpec> for CandidatePosition {
    type Error = crate::error::Error;

    fn try_from(spec: CandidatePositionSpec) -> Result<Self, Self::Error> {
        let instrument = spec.instrument_spec.into_boxed().map_err(|e| {
            crate::error::Error::invalid_input(format!(
                "Failed to convert candidate instrument JSON: {e}"
            ))
        })?;
        Ok(Self {
            id: spec.id,
            entity_id: spec.entity_id,
            instrument: Arc::from(instrument),
            unit: spec.unit,
            attributes: spec.attributes,
            max_weight: spec.max_weight,
            min_weight: spec.min_weight,
        })
    }
}

impl Serialize for CandidatePosition {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let instrument_spec = self.instrument.to_instrument_json().ok_or_else(|| {
            serde::ser::Error::custom(format!(
                "candidate '{}' holds an instrument with no JSON representation",
                self.id
            ))
        })?;
        CandidatePositionSpec {
            id: self.id.clone(),
            entity_id: self.entity_id.clone(),
            instrument_spec,
            unit: self.unit,
            attributes: self.attributes.clone(),
            max_weight: self.max_weight,
            min_weight: self.min_weight,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CandidatePosition {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let spec = CandidatePositionSpec::deserialize(deserializer)?;
        Self::try_from(spec).map_err(serde::de::Error::custom)
    }
}

/// Defines which instruments the optimizer can trade.
///
/// The trade universe consists of:
///
/// 1. **Tradeable positions**: existing portfolio positions that can be adjusted
/// 2. **Held positions**: existing positions locked at current weight and exact quantity
/// 3. **Candidate positions**: new instruments that could be added
///
/// Serializes with serde; candidate instruments travel as their canonical
/// tagged JSON payload (see [`CandidatePosition`]).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TradeUniverse {
    /// Filter for existing positions that can be traded.
    /// Positions matching this filter have their weights optimized.
    /// Default: all positions are tradeable.
    pub tradeable_filter: PositionFilter,

    /// Filter for existing positions that are held constant.
    /// Positions matching this filter keep their current weight and exact
    /// quantity under every weighting scheme, including zero-PV holdings.
    /// Takes precedence over `tradeable_filter` if both match.
    pub held_filter: Option<PositionFilter>,

    /// Candidate instruments not currently in the portfolio.
    /// These start with weight 0 and can be added by the optimizer. Their IDs
    /// must be unique among candidates and absent from existing positions.
    pub candidates: Vec<CandidatePosition>,

    /// Whether candidates can receive negative weights (short selling).
    /// Default: false (long‑only for new positions).
    pub allow_short_candidates: bool,
}

impl TradeUniverse {
    /// Create a universe where all existing positions are tradeable.
    ///
    /// # Returns
    ///
    /// Default trade universe with all held positions tradeable and no candidates.
    pub fn all_positions() -> Self {
        Self::default()
    }

    /// Create a universe with only specific positions tradeable.
    ///
    /// # Arguments
    ///
    /// * `filter` - Filter selecting which existing positions may trade.
    ///
    /// # Returns
    ///
    /// Trade universe with the supplied tradeable filter.
    pub fn filtered(filter: PositionFilter) -> Self {
        Self {
            tradeable_filter: filter,
            ..Self::default()
        }
    }

    /// Add a candidate position to the universe.
    ///
    /// # Arguments
    ///
    /// * `candidate` - Candidate instrument that may be added by the optimizer.
    ///
    /// # Returns
    ///
    /// The updated trade universe for fluent chaining.
    #[must_use]
    pub fn with_candidate(mut self, candidate: CandidatePosition) -> Self {
        self.candidates.push(candidate);
        self
    }

    /// Allow short selling of candidate positions.
    ///
    /// # Returns
    ///
    /// The updated trade universe for fluent chaining.
    pub fn allow_shorting_candidates(mut self) -> Self {
        self.allow_short_candidates = true;
        self
    }
}

impl Default for TradeUniverse {
    fn default() -> Self {
        Self {
            tradeable_filter: PositionFilter::All,
            held_filter: None,
            candidates: Vec::new(),
            allow_short_candidates: false,
        }
    }
}

impl std::fmt::Debug for CandidatePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidatePosition")
            .field("id", &self.id)
            .field("entity_id", &self.entity_id)
            .field("unit", &self.unit)
            .field("attributes", &self.attributes)
            .field("max_weight", &self.max_weight)
            .field("min_weight", &self.min_weight)
            .finish()
    }
}
