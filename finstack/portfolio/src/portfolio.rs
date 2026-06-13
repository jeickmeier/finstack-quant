//! Portfolio struct and core operations.
//!
//! A portfolio represents a book of positions held by one or more entities.
//! The module provides helpers for traversing positions, filtering by tags, and
//! validating structural invariants before valuation takes place.

use crate::book::{Book, BookId};
use crate::dependencies::DependencyIndex;
use crate::error::{Error, Result};
use crate::position::Position;
use crate::types::{AttributeValue, Entity, EntityId, PositionId, DUMMY_ENTITY_ID};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::HashMap;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A portfolio of positions across multiple entities.
///
/// The portfolio holds a flat list of positions, each referencing an entity
/// and instrument. Positions can be grouped and aggregated by entity or by
/// arbitrary attributes (tags). Optional book hierarchy provides multi-level
/// organizational structure.
#[derive(Clone, Debug)]
pub struct Portfolio {
    /// Unique identifier for this portfolio
    pub id: String,

    /// Human-readable name
    pub name: Option<String>,

    /// Base currency for aggregation
    pub base_ccy: Currency,

    /// Valuation date
    pub as_of: Date,

    /// Entities that own positions
    pub entities: IndexMap<EntityId, Entity>,

    /// Flat list of positions (not serialized directly due to Instrument trait).
    ///
    /// Instruments behind `Arc` must be immutable after construction. The
    /// portfolio assumes no interior mutability: concurrent reads are safe, but
    /// mutating an instrument after adding it to a portfolio is undefined at
    /// the application level.
    pub(crate) positions: Vec<Position>,

    /// Index mapping position ID → index in `positions` for O(1) lookup.
    ///
    /// Rebuilt automatically via [`rebuild_index`] when constructing a portfolio
    /// through the builder or `from_spec`.
    pub(crate) position_index: HashMap<PositionId, usize>,

    /// Inverted index mapping market factor keys to affected position indices.
    ///
    /// Rebuilt together with `position_index` via [`rebuild_index`].
    /// Enables selective repricing when only a subset of market data changes.
    pub(crate) dependency_index: DependencyIndex,

    /// Index mapping entity ID → owned position indices, so `positions_for_entity`
    /// avoids a full O(N) scan. Rebuilt via [`rebuild_index`].
    pub(crate) entity_index: HashMap<EntityId, Vec<usize>>,

    /// Index mapping attribute key → position indices carrying that key. Narrows
    /// `positions_with_attribute` to the positions that actually have the key;
    /// the value comparison still runs over that reduced set. Rebuilt via
    /// [`rebuild_index`].
    pub(crate) attribute_key_index: HashMap<String, Vec<usize>>,

    /// Optional hierarchical book organization
    pub books: IndexMap<BookId, Book>,

    /// Portfolio-level tags
    pub tags: IndexMap<String, String>,

    /// Additional metadata
    pub meta: IndexMap<String, serde_json::Value>,
}

impl Serialize for Portfolio {
    fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Err(serde::ser::Error::custom(
            "direct Portfolio serialization is unsupported because positions contain runtime instruments; use Portfolio::to_spec() or PortfolioSpec",
        ))
    }
}

impl<'de> Deserialize<'de> for Portfolio {
    fn deserialize<D>(_deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(serde::de::Error::custom(
            "direct Portfolio deserialization is unsupported because positions require runtime instruments; use Portfolio::from_spec(PortfolioSpec)",
        ))
    }
}

/// Serializable portfolio specification.
///
/// This struct allows portfolios to be serialized and deserialized by storing
/// positions as `PositionSpec` rather than `Position` (which contains non-serializable
/// `Arc<dyn Instrument>`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioSpec {
    /// Portfolio identifier
    pub id: String,
    /// Human-readable name
    pub name: Option<String>,
    /// Base currency for aggregation
    pub base_ccy: Currency,
    /// Valuation date
    pub as_of: Date,
    /// Entities that own positions
    pub entities: IndexMap<EntityId, Entity>,
    /// Positions as serializable specs
    pub positions: Vec<crate::position::PositionSpec>,
    /// Optional hierarchical book organization
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub books: IndexMap<BookId, Book>,
    /// Portfolio-level tags
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tags: IndexMap<String, String>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

impl Portfolio {
    /// Create a [`crate::builder::PortfolioBuilder`] for constructing a portfolio.
    ///
    /// This is the single canonical entry point for portfolio construction.
    /// The builder validates invariants on [`crate::builder::PortfolioBuilder::build`].
    #[must_use]
    pub fn builder(id: impl Into<String>) -> crate::builder::PortfolioBuilder {
        crate::builder::PortfolioBuilder::new(id)
    }

    /// Rebuild both derived caches: the position-ID lookup and the
    /// market-factor dependency index.
    ///
    /// Call this after mutating `positions` directly to keep the O(1) lookup
    /// and the selective-repricing index valid.
    pub fn rebuild_index(&mut self) {
        self.position_index = self
            .positions
            .iter()
            .enumerate()
            .map(|(i, p)| (p.position_id.clone(), i))
            .collect();
        self.dependency_index = DependencyIndex::build(&self.positions);

        let mut entity_index: HashMap<EntityId, Vec<usize>> = HashMap::default();
        let mut attribute_key_index: HashMap<String, Vec<usize>> = HashMap::default();
        for (i, position) in self.positions.iter().enumerate() {
            entity_index
                .entry(position.entity_id.clone())
                .or_default()
                .push(i);
            for key in position.attributes.keys() {
                attribute_key_index.entry(key.clone()).or_default().push(i);
            }
        }
        self.entity_index = entity_index;
        self.attribute_key_index = attribute_key_index;
    }

    /// Borrow all positions as an immutable slice.
    ///
    /// # Returns
    ///
    /// All stored positions in insertion order.
    pub fn positions(&self) -> &[Position] {
        &self.positions
    }

    /// Append a position and incrementally update derived indices.
    ///
    /// This is O(k) where k is the number of market dependencies for the
    /// position, rather than O(N) for a full rebuild.
    ///
    /// # Arguments
    ///
    /// * `position` - Position to append to the portfolio.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if a position with the same
    /// `position_id` is already present. Silently appending would leave a
    /// duplicate row in the internal `positions` vector that all
    /// iteration-based aggregators would double-count, while the
    /// `position_index` map and [`Self::get_position`] would only see the
    /// new row.
    pub fn add_position(&mut self, position: Position) -> Result<()> {
        if self.position_index.contains_key(&position.position_id) {
            return Err(Error::validation(format!(
                "Duplicate position ID: {}",
                position.position_id
            )));
        }
        let idx = self.positions.len();
        self.position_index
            .insert(position.position_id.clone(), idx);
        self.dependency_index.add_position(idx, &position);
        self.entity_index
            .entry(position.entity_id.clone())
            .or_default()
            .push(idx);
        for key in position.attributes.keys() {
            self.attribute_key_index
                .entry(key.clone())
                .or_default()
                .push(idx);
        }
        self.positions.push(position);
        Ok(())
    }

    /// Replace all positions and refresh derived indices.
    ///
    /// # Arguments
    ///
    /// * `positions` - Complete replacement position vector.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_core::Error::Validation`] if the replacement vector contains
    /// duplicate position IDs. The portfolio is left unchanged on error.
    pub fn set_positions(&mut self, positions: Vec<Position>) -> Result<()> {
        use finstack_core::HashSet;

        let mut seen_ids = HashSet::default();
        for position in &positions {
            if !seen_ids.insert(&position.position_id) {
                return Err(Error::validation(format!(
                    "Duplicate position ID: {}",
                    position.position_id
                )));
            }
        }

        self.positions = positions;
        self.rebuild_index();
        Ok(())
    }

    /// Get a position by identifier (O(1) via index).
    ///
    /// # Arguments
    ///
    /// * `position_id` - Identifier of the position to locate.
    ///
    /// # Returns
    ///
    /// The matching position, if present.
    pub fn get_position(&self, position_id: &str) -> Option<&Position> {
        self.position_index
            .get(position_id)
            .and_then(|&idx| self.positions.get(idx))
    }

    /// Get all positions for a given entity.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - Entity identifier used for filtering (accepts &str or &EntityId).
    ///
    /// # Returns
    ///
    /// All positions owned by the requested entity.
    pub fn positions_for_entity(&self, entity_id: &str) -> Vec<&Position> {
        self.entity_index
            .get(entity_id)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.positions.get(i))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Filter positions whose attribute matches the supplied key/value.
    ///
    /// All positions whose attribute matches the supplied key/value pair.
    ///
    /// # Arguments
    ///
    /// * `key` - Attribute key to inspect.
    /// * `value` - Desired attribute value.
    ///
    /// # Returns
    ///
    /// All positions whose attribute matches the supplied key/value pair.
    pub fn positions_with_attribute(&self, key: &str, value: &AttributeValue) -> Vec<&Position> {
        self.attribute_key_index
            .get(key)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| self.positions.get(i))
                    .filter(|p| p.attributes.get(key) == Some(value))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Read-only access to the dependency index for inspection and testing.
    ///
    /// # Returns
    ///
    /// Borrowed dependency index rebuilt alongside the position lookup cache.
    pub fn dependency_index(&self) -> &DependencyIndex {
        &self.dependency_index
    }

    /// Validate the portfolio structure.
    ///
    /// Checks that:
    /// - All position IDs are unique
    /// - All positions reference valid entities
    /// - Dummy entity exists if needed
    /// - Position book references point to existing books
    /// - Book position references point to existing positions
    /// - Book hierarchy contains no cycles
    ///
    /// # Returns
    ///
    /// `Ok(())` when all portfolio invariants hold.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ValidationFailed`] when duplicate position IDs are found,
    /// [`Error::UnknownEntity`] when a position references an entity
    /// that is not present in [`Portfolio::entities`], or when a cycle is detected
    /// in the book hierarchy.
    pub fn validate(&self) -> Result<()> {
        use finstack_core::HashSet;

        let mut seen_ids: finstack_core::HashSet<_> = HashSet::default();
        for position in &self.positions {
            // Check for duplicate position IDs
            if !seen_ids.insert(&position.position_id) {
                return Err(Error::validation(format!(
                    "Duplicate position ID: {}",
                    position.position_id
                )));
            }

            // Check entity exists
            if !self.entities.contains_key(&position.entity_id) {
                return Err(Error::UnknownEntity {
                    position_id: position.position_id.clone(),
                    entity_id: position.entity_id.clone(),
                });
            }
        }

        // Validate book references before hierarchy traversal so missing books
        // or positions fail directly instead of being treated as empty groups.
        for position in &self.positions {
            if let Some(book_id) = &position.book_id {
                if !self.books.contains_key(book_id) {
                    return Err(Error::validation(format!(
                        "Position '{}' references non-existent book '{}'",
                        position.position_id, book_id
                    )));
                }
            }
        }
        for (book_id, book) in &self.books {
            for position_id in &book.position_ids {
                if !seen_ids.contains(position_id) {
                    return Err(Error::validation(format!(
                        "Book '{}' references non-existent position '{}'",
                        book_id, position_id
                    )));
                }
            }
        }

        // Validate book hierarchy: check for cycles via parent_id chains
        self.validate_book_hierarchy()?;

        Ok(())
    }

    /// Validate the book hierarchy for cycles and referential integrity.
    ///
    /// Checks performed:
    /// 1. No cycles in parent_id chains (iterative walk with visited set).
    /// 2. Every `child_book_ids` entry refers to an existing book.
    /// 3. Parent/child consistency: if B is in A's `child_book_ids`,
    ///    then B's `parent_id` must be A.
    fn validate_book_hierarchy(&self) -> Result<()> {
        use finstack_core::HashSet;

        // (1) Detect cycles via parent_id chains
        for (book_id, book) in &self.books {
            let mut visited = HashSet::default();
            visited.insert(book_id.clone());

            let mut current = book.parent_id.clone();
            while let Some(ref pid) = current {
                if !visited.insert(pid.clone()) {
                    return Err(Error::validation(format!(
                        "Cycle detected in book hierarchy at book '{}'",
                        pid
                    )));
                }
                current = self.books.get(pid).and_then(|b| b.parent_id.clone());
            }
        }

        // (2)+(3) Validate child_book_ids referential integrity
        for (parent_id, parent_book) in &self.books {
            for child_id in &parent_book.child_book_ids {
                let child = self.books.get(child_id).ok_or_else(|| {
                    Error::validation(format!(
                        "Book '{}' references non-existent child book '{}'",
                        parent_id, child_id
                    ))
                })?;

                if child.parent_id.as_ref() != Some(parent_id) {
                    return Err(Error::validation(format!(
                        "Book '{}' lists '{}' as child, but child's parent_id is {:?}",
                        parent_id, child_id, child.parent_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if the portfolio uses the dummy entity.
    ///
    /// # Returns
    ///
    /// `true` when the canonical standalone-positions entity is present.
    pub fn has_dummy_entity(&self) -> bool {
        self.entities.contains_key(DUMMY_ENTITY_ID)
    }

    /// Convert this portfolio to a serializable specification.
    ///
    /// Converts all positions to `PositionSpec` by extracting their instrument
    /// JSON representations. Positions whose instruments don't support serialization
    /// will have `instrument_spec: None` and cannot be fully reconstructed.
    ///
    /// # Returns
    ///
    /// A `PortfolioSpec` that can be serialized to JSON.
    pub fn to_spec(&self) -> PortfolioSpec {
        PortfolioSpec {
            id: self.id.clone(),
            name: self.name.clone(),
            base_ccy: self.base_ccy,
            as_of: self.as_of,
            entities: self.entities.clone(),
            positions: self.positions.iter().map(|p| p.to_spec()).collect(),
            books: self.books.clone(),
            tags: self.tags.clone(),
            meta: self.meta.clone(),
        }
    }

    /// Reconstruct a Portfolio from a specification.
    ///
    /// # Arguments
    ///
    /// * `spec` - The portfolio specification to reconstruct
    ///
    /// # Returns
    ///
    /// Rebuilt runtime portfolio with caches refreshed and validation applied.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if:
    /// - Any position specification cannot be converted to a position
    /// - Portfolio validation fails (duplicate IDs, unknown entities)
    pub fn from_spec(spec: PortfolioSpec) -> Result<Self> {
        let positions: Result<Vec<_>> = spec
            .positions
            .into_iter()
            .map(crate::position::Position::from_spec)
            .collect();

        let positions = positions?;

        let mut portfolio = Self {
            id: spec.id,
            name: spec.name,
            base_ccy: spec.base_ccy,
            as_of: spec.as_of,
            entities: spec.entities,
            positions,
            position_index: HashMap::default(),
            dependency_index: DependencyIndex::default(),
            entity_index: HashMap::default(),
            attribute_key_index: HashMap::default(),
            books: spec.books,
            tags: spec.tags,
            meta: spec.meta,
        };

        portfolio.rebuild_index();

        // Validate the reconstructed portfolio
        portfolio.validate()?;

        Ok(portfolio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::Book;
    use crate::position::{Position, PositionUnit};
    use crate::types::{Entity, PositionId};
    use finstack_core::money::Money;
    use finstack_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    fn test_position_with_book(book_id: &str) -> Position {
        let as_of = date!(2024 - 01 - 01);
        let dep = Deposit::builder()
            .id("DEP_BOOK_TEST".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test instrument should build");
        Position::new(
            "POS_BOOK_TEST",
            "ACME",
            "DEP_BOOK_TEST",
            Arc::new(dep),
            1.0,
            PositionUnit::Units,
        )
        .expect("test position should build")
        .with_book(book_id)
    }

    #[test]
    fn test_portfolio_creation() {
        let portfolio = Portfolio::builder("FUND_A")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .build()
            .expect("test should succeed");

        assert_eq!(portfolio.id, "FUND_A");
        assert_eq!(portfolio.base_ccy, Currency::USD);
        assert_eq!(portfolio.as_of, date!(2024 - 01 - 01));
        assert!(portfolio.entities.is_empty());
        assert!(portfolio.positions.is_empty());
    }

    #[test]
    fn test_portfolio_validation() {
        let portfolio = Portfolio::builder("FUND_A")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .entity(Entity::new("ACME"))
            .build()
            .expect("test should succeed");

        // Valid portfolio
        assert!(portfolio.validate().is_ok());
    }

    #[test]
    fn minor2_direct_portfolio_serde_fails_fast() {
        let portfolio = Portfolio::builder("FUND_A")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .build()
            .expect("test portfolio should build");

        let err = serde_json::to_string(&portfolio)
            .expect_err("minor 2: direct Portfolio serialization must fail");
        assert!(
            err.to_string().contains("PortfolioSpec"),
            "minor 2: error should point callers at PortfolioSpec, got {err}"
        );

        let err = serde_json::from_str::<Portfolio>(r#"{"id":"FUND_A"}"#)
            .expect_err("minor 2: direct Portfolio deserialization must fail");
        assert!(
            err.to_string().contains("PortfolioSpec"),
            "minor 2: error should point callers at PortfolioSpec, got {err}"
        );
    }

    #[test]
    fn minor3_validate_rejects_position_book_reference_to_missing_book() {
        let portfolio = Portfolio::builder("FUND_A")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .entity(Entity::new("ACME"))
            .position(test_position_with_book("MISSING_BOOK"))
            .build()
            .expect_err("minor 3: missing position book reference must fail validation");

        assert!(
            portfolio.to_string().contains("non-existent book"),
            "minor 3: unexpected validation error: {portfolio}"
        );
    }

    #[test]
    fn minor3_validate_rejects_book_reference_to_missing_position() {
        let mut book = Book::new("BOOK_A", Some("Book A".to_string()));
        book.add_position(PositionId::new("MISSING_POSITION"));
        let portfolio = Portfolio::builder("FUND_A")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .book(book)
            .build()
            .expect_err("minor 3: missing book position reference must fail validation");

        assert!(
            portfolio.to_string().contains("non-existent position"),
            "minor 3: unexpected validation error: {portfolio}"
        );
    }
}
