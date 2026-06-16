//! Core types for portfolio management.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::fmt;

define_string_id! {
    /// Entity identifier (company, fund, etc.)
    pub struct EntityId;
}

define_string_id! {
    /// Position identifier.
    pub struct PositionId;
}

/// Constant for the dummy entity used for standalone instruments.
///
/// Standalone instruments (IRS, Deposits, FX, etc.) that don't belong
/// to a specific entity can reference this dummy entity ID.
pub const DUMMY_ENTITY_ID: &str = "_standalone";

/// An entity that can hold positions.
///
/// Entities represent companies, funds, or other legal entities that
/// own instruments. For standalone instruments (derivatives, FX), use
/// the dummy entity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier for the entity
    pub id: EntityId,

    /// Human-readable name
    pub name: Option<String>,

    /// Entity-level tags for grouping and filtering
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tags: IndexMap<String, String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

impl Entity {
    /// Create a new entity with the given ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique entity identifier.
    ///
    /// # Returns
    ///
    /// A new entity with empty tags and metadata.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_portfolio::types::Entity;
    ///
    /// let entity = Entity::new("ACME");
    /// assert_eq!(entity.id.as_str(), "ACME");
    /// assert!(entity.name.is_none());
    /// ```
    pub fn new(id: impl Into<EntityId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            tags: IndexMap::new(),
            meta: IndexMap::new(),
        }
    }

    /// Set the entity name.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name.
    ///
    /// # Returns
    ///
    /// The updated entity for fluent chaining.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Add a tag.
    ///
    /// # Arguments
    ///
    /// * `key` - Tag key.
    /// * `value` - Tag value.
    ///
    /// # Returns
    ///
    /// The updated entity for fluent chaining.
    pub fn with_tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add multiple tags at once.
    ///
    /// # Arguments
    ///
    /// * `tags` - Iterator of (key, value) pairs.
    ///
    /// # Returns
    ///
    /// The updated entity for fluent chaining.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_portfolio::types::Entity;
    ///
    /// let entity = Entity::new("ACME")
    ///     .with_tags([("sector", "Technology"), ("region", "US")]);
    ///
    /// assert_eq!(entity.tags.get("sector"), Some(&"Technology".to_string()));
    /// assert_eq!(entity.tags.get("region"), Some(&"US".to_string()));
    /// ```
    pub fn with_tags<K, V, I>(mut self, tags: I) -> Self
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in tags {
            self.tags.insert(k.into(), v.into());
        }
        self
    }

    /// Create the dummy entity for standalone instruments.
    ///
    /// # Returns
    ///
    /// The canonical dummy entity used for positions that are not associated
    /// with a real legal entity.
    pub fn dummy() -> Self {
        Self {
            id: EntityId::new(DUMMY_ENTITY_ID),
            name: Some("Standalone Instruments".to_string()),
            tags: IndexMap::new(),
            meta: IndexMap::new(),
        }
    }
}

/// Value stored in a position or candidate attribute.
///
/// Positions carry key/value attributes for grouping, filtering, and
/// optimization constraints.  Text values represent categorical data
/// (rating, sector), while numeric values represent continuous data
/// (credit score, ESG score) usable in metric expressions.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    /// Categorical / string attribute (e.g., rating = "CCC", sector = "Energy").
    Text(String),
    /// Numeric attribute (e.g., credit_score = 650.0, esg_score = 72.5).
    Number(f64),
}

impl AttributeValue {
    /// Return the text value if this is a `Text` variant.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Number(_) => None,
        }
    }

    /// Return the numeric value if this is a `Number` variant.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Text(_) => None,
        }
    }
}

impl fmt::Display for AttributeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) => write!(f, "{s}"),
            Self::Number(n) => write!(f, "{n}"),
        }
    }
}

impl From<&str> for AttributeValue {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<String> for AttributeValue {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<f64> for AttributeValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
    }
}

/// Comparison operator for attribute-based filtering.
///
/// For [`AttributeValue::Text`] attributes, only [`ComparisonOp::Eq`] and
/// [`ComparisonOp::Ne`] are meaningful; ordering comparisons on text return
/// `false`.  For [`AttributeValue::Number`] attributes, all six operators
/// apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComparisonOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than (numeric only).
    Lt,
    /// Less than or equal (numeric only).
    Le,
    /// Greater than (numeric only).
    Gt,
    /// Greater than or equal (numeric only).
    Ge,
}

/// Predicate that tests a single position attribute against a value.
///
/// Reusable building block for [`crate::optimization::PositionFilter::ByAttribute`]
/// and [`crate::optimization::PerPositionMetric::AttributeIndicator`].
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AttributeTest {
    /// Attribute key to test.
    pub key: String,
    /// Comparison operator.
    pub op: ComparisonOp,
    /// Value to compare against.
    pub value: AttributeValue,
}

impl AttributeTest {
    /// Create a new attribute test.
    pub fn new(key: impl Into<String>, op: ComparisonOp, value: impl Into<AttributeValue>) -> Self {
        Self {
            key: key.into(),
            op,
            value: value.into(),
        }
    }

    /// Convenience: text equality test.
    pub fn text_eq(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self::new(key, ComparisonOp::Eq, AttributeValue::Text(value.into()))
    }

    /// Convenience: numeric comparison test.
    pub fn numeric(key: impl Into<String>, op: ComparisonOp, value: f64) -> Self {
        Self::new(key, op, AttributeValue::Number(value))
    }

    /// Evaluate this test against a set of attributes.
    ///
    /// Returns `false` if the key is absent or types are incompatible
    /// (e.g., ordering comparison on text).
    ///
    /// Numeric equality and inequality use an absolute/relative tolerance
    /// (`1e-12` / `1e-9`) to avoid float-representation noise. Numeric
    /// ordering comparisons (`<`, `<=`, `>`, `>=`) are exact so tier and
    /// constraint boundaries do not shift because of tolerance.
    pub fn evaluate(&self, attributes: &IndexMap<String, AttributeValue>) -> bool {
        let Some(attr) = attributes.get(&self.key) else {
            return false;
        };
        match (&self.op, attr, &self.value) {
            (ComparisonOp::Eq, AttributeValue::Text(a), AttributeValue::Text(b)) => a == b,
            (ComparisonOp::Ne, AttributeValue::Text(a), AttributeValue::Text(b)) => a != b,
            (ComparisonOp::Eq, AttributeValue::Number(a), AttributeValue::Number(b)) => {
                let abs_tol = 1e-12;
                let rel_tol = 1e-9;
                (a - b).abs() <= f64::max(abs_tol, rel_tol * f64::max(a.abs(), b.abs()))
            }
            (ComparisonOp::Ne, AttributeValue::Number(a), AttributeValue::Number(b)) => {
                let abs_tol = 1e-12;
                let rel_tol = 1e-9;
                (a - b).abs() > f64::max(abs_tol, rel_tol * f64::max(a.abs(), b.abs()))
            }
            (ComparisonOp::Lt, AttributeValue::Number(a), AttributeValue::Number(b)) => a < b,
            (ComparisonOp::Le, AttributeValue::Number(a), AttributeValue::Number(b)) => a <= b,
            (ComparisonOp::Gt, AttributeValue::Number(a), AttributeValue::Number(b)) => a > b,
            (ComparisonOp::Ge, AttributeValue::Number(a), AttributeValue::Number(b)) => a >= b,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn test_entity_creation() {
        let entity = Entity::new("ACME_CORP")
            .with_name("Acme Corporation")
            .with_tag("sector", "Technology");

        assert_eq!(entity.id.as_str(), "ACME_CORP");
        assert_eq!(entity.name, Some("Acme Corporation".to_string()));
        assert_eq!(entity.tags.get("sector"), Some(&"Technology".to_string()));
    }

    #[test]
    fn test_dummy_entity() {
        let dummy = Entity::dummy();
        assert_eq!(dummy.id.as_str(), DUMMY_ENTITY_ID);
        assert!(dummy.name.is_some());
    }

    #[test]
    fn test_entity_id_newtype() {
        let id1 = EntityId::new("ENTITY_1");
        let id2: EntityId = "ENTITY_2".into();
        let id3: EntityId = String::from("ENTITY_3").into();

        assert_eq!(id1.as_str(), "ENTITY_1");
        assert_eq!(format!("{}", id2), "ENTITY_2");
        assert_eq!(id3.to_string(), "ENTITY_3");

        // Test equality
        let id1_clone = EntityId::new("ENTITY_1");
        assert_eq!(id1, id1_clone);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_position_id_newtype() {
        let id1 = PositionId::new("POS_001");
        let id2: PositionId = "POS_002".into();

        assert_eq!(id1.as_str(), "POS_001");
        assert_eq!(format!("{}", id2), "POS_002");
    }

    #[test]
    fn test_with_tags() {
        let entity = Entity::new("ACME").with_tags([("sector", "Tech"), ("region", "NA")]);

        assert_eq!(entity.tags.len(), 2);
        assert_eq!(entity.tags.get("sector"), Some(&"Tech".to_string()));
        assert_eq!(entity.tags.get("region"), Some(&"NA".to_string()));
    }

    #[test]
    fn test_attribute_value_text() {
        let v = AttributeValue::Text("CCC".to_string());
        assert_eq!(v.as_text(), Some("CCC"));
        assert_eq!(v.as_number(), None);
        assert_eq!(format!("{v}"), "CCC");
    }

    #[test]
    fn test_attribute_value_number() {
        let v = AttributeValue::Number(72.5);
        assert_eq!(v.as_text(), None);
        assert_eq!(v.as_number(), Some(72.5));
        assert_eq!(format!("{v}"), "72.5");
    }

    #[test]
    fn test_attribute_value_from() {
        let t: AttributeValue = "hello".into();
        assert!(matches!(t, AttributeValue::Text(_)));
        let n: AttributeValue = 42.0_f64.into();
        assert!(matches!(n, AttributeValue::Number(_)));
    }

    #[test]
    fn test_attribute_value_serde_text() {
        let v = AttributeValue::Text("CCC".to_string());
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "\"CCC\"");
        let round: AttributeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(round, v);
    }

    #[test]
    fn test_attribute_value_serde_number() {
        let v = AttributeValue::Number(72.5);
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "72.5");
        let round: AttributeValue = serde_json::from_str(&json).unwrap();
        assert_eq!(round, v);
    }

    #[test]
    fn test_attribute_test_text_eq() {
        let attrs = IndexMap::from([(
            "rating".to_string(),
            AttributeValue::Text("CCC".to_string()),
        )]);
        assert!(AttributeTest::text_eq("rating", "CCC").evaluate(&attrs));
        assert!(!AttributeTest::text_eq("rating", "BB").evaluate(&attrs));
        assert!(!AttributeTest::text_eq("missing", "CCC").evaluate(&attrs));
    }

    #[test]
    fn test_attribute_test_numeric_comparisons() {
        let attrs = IndexMap::from([("score".to_string(), AttributeValue::Number(650.0))]);
        assert!(AttributeTest::numeric("score", ComparisonOp::Ge, 600.0).evaluate(&attrs));
        assert!(!AttributeTest::numeric("score", ComparisonOp::Lt, 600.0).evaluate(&attrs));
        assert!(AttributeTest::numeric("score", ComparisonOp::Le, 650.0).evaluate(&attrs));
        assert!(AttributeTest::numeric("score", ComparisonOp::Eq, 650.0).evaluate(&attrs));
        assert!(!AttributeTest::numeric("score", ComparisonOp::Ne, 650.0).evaluate(&attrs));
        assert!(AttributeTest::numeric("score", ComparisonOp::Gt, 600.0).evaluate(&attrs));
    }

    #[test]
    fn numeric_eq_uses_tolerance_but_ordering_is_exact() {
        let attrs = IndexMap::from([("score".to_string(), AttributeValue::Number(600.0 + 1e-10))]);

        assert!(AttributeTest::numeric("score", ComparisonOp::Eq, 600.0).evaluate(&attrs));
        assert!(!AttributeTest::numeric("score", ComparisonOp::Ne, 600.0).evaluate(&attrs));
        assert!(AttributeTest::numeric("score", ComparisonOp::Gt, 600.0).evaluate(&attrs));
        assert!(!AttributeTest::numeric("score", ComparisonOp::Le, 600.0).evaluate(&attrs));
    }

    #[test]
    fn test_attribute_test_type_mismatch() {
        let attrs = IndexMap::from([(
            "rating".to_string(),
            AttributeValue::Text("CCC".to_string()),
        )]);
        assert!(!AttributeTest::numeric("rating", ComparisonOp::Gt, 5.0).evaluate(&attrs));
    }
}
