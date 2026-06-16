//! Builder pattern for assembling [`Portfolio`] instances.
//!
//! This module provides a fluent interface for constructing validated portfolios.
//! The builder keeps track of entities, positions, metadata and validates the resulting
//! [`Portfolio`] to ensure it is internally consistent.
//! Typical usage starts by creating a builder with [`Portfolio::builder`](crate::portfolio::Portfolio::builder), chaining
//! configuration methods, and finalizing with [`PortfolioBuilder::build`].
//!
//! # Error Handling Best Practices
//!
//! All builder methods should return `Result<T, Error>` instead of using `unwrap()`.
//! This allows callers to handle errors appropriately:
//!
//! ```rust
//! use finstack_quant_portfolio::Portfolio;
//! use finstack_quant_core::currency::Currency;
//! use finstack_quant_core::dates::Date;
//! use time::Month;
//!
//! let valuation_date = Date::from_calendar_date(2024, Month::January, 1).expect("test should succeed");
//!
//! let result = Portfolio::builder("test_portfolio")
//!     .base_ccy(Currency::USD)
//!     .as_of(valuation_date)
//!     .build();
//!
//! match result {
//!     Ok(portfolio) => {
//!         // Use portfolio
//!         assert_eq!(portfolio.base_ccy, Currency::USD);
//!     }
//!     Err(e) => {
//!         // Handle error
//!         eprintln!("Failed to build portfolio: {}", e);
//!     }
//! }
//! ```

use crate::book::{Book, BookId};
use crate::error::Result;
use crate::portfolio::Portfolio;
use crate::position::Position;
use crate::types::{Entity, EntityId, PositionId, DUMMY_ENTITY_ID};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use indexmap::IndexMap;

/// Builder for constructing a [`Portfolio`] with validation.
///
/// The builder stores all intermediate values needed to construct a portfolio and checks
/// invariants such as base currency, valuation date, and entity references before the
/// final portfolio is produced.
#[derive(Debug)]
pub struct PortfolioBuilder {
    id: String,
    name: Option<String>,
    base_ccy: Option<Currency>,
    as_of: Option<Date>,
    entities: IndexMap<EntityId, Entity>,
    positions: Vec<Position>,
    books: IndexMap<BookId, Book>,
    tags: IndexMap<String, String>,
    meta: IndexMap<String, serde_json::Value>,
}

impl PortfolioBuilder {
    /// Create a new portfolio builder with the given identifier.
    ///
    /// # Arguments
    ///
    /// * `id` - Unique identifier for the portfolio; converted into an owned `String`.
    ///
    /// # Returns
    ///
    /// A builder with no configured currency, valuation date, entities, or positions.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            base_ccy: None,
            as_of: None,
            entities: IndexMap::new(),
            positions: Vec::new(),
            books: IndexMap::new(),
            tags: IndexMap::new(),
            meta: IndexMap::new(),
        }
    }

    /// Set the portfolio's human-readable name.
    ///
    /// # Arguments
    ///
    /// * `name` - Display name stored alongside the portfolio identifier.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Declare the portfolio's reporting currency.
    ///
    /// # Arguments
    ///
    /// * `ccy` - Currency to use when consolidating values and metrics.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn base_ccy(mut self, ccy: Currency) -> Self {
        self.base_ccy = Some(ccy);
        self
    }

    /// Assign the valuation date used for pricing and analytics.
    ///
    /// # Arguments
    ///
    /// * `date` - The as-of date for valuation and risk calculation.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn as_of(mut self, date: Date) -> Self {
        self.as_of = Some(date);
        self
    }

    /// Register a single [`Entity`] with the builder.
    ///
    /// # Arguments
    ///
    /// * `entity` - Entity definition including identifier, tags and metadata.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn entity(mut self, entity: Entity) -> Self {
        self.entities.insert(entity.id.clone(), entity);
        self
    }

    /// Register multiple entities in a single call.
    ///
    /// # Arguments
    ///
    /// * `entities` - Iterator yielding entities to insert into the portfolio.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn entities(mut self, entities: impl IntoIterator<Item = Entity>) -> Self {
        for entity in entities {
            self.entities.insert(entity.id.clone(), entity);
        }
        self
    }

    /// Add a single position to the portfolio.
    ///
    /// # Arguments
    ///
    /// * `position` - Fully constructed [`Position`].
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn position(mut self, position: Position) -> Self {
        self.positions.push(position);
        self
    }

    /// Add multiple positions from any iterator.
    ///
    /// # Arguments
    ///
    /// * `positions` - Iterator yielding positions to append to the builder.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn positions(mut self, positions: impl IntoIterator<Item = Position>) -> Self {
        self.positions.extend(positions);
        self
    }

    /// Apply a portfolio-level tag.
    ///
    /// Tags allow categorisation and filtering of portfolios.
    ///
    /// # Arguments
    ///
    /// * `key` - Tag identifier.
    /// * `value` - Tag value stored as a string.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Attach an arbitrary JSON metadata entry.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key.
    /// * `value` - JSON payload stored alongside the portfolio.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.meta.insert(key.into(), value);
        self
    }

    /// Add a book to the portfolio hierarchy.
    ///
    /// # Arguments
    ///
    /// * `book` - Book definition with optional parent reference.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn book(mut self, book: Book) -> Self {
        let book_id = book.id.clone();
        let parent_id = book.parent_id.clone();
        self.books.insert(book_id.clone(), book);

        if let Some(parent_id) = parent_id {
            if let Some(parent) = self.books.get_mut(&parent_id) {
                parent.add_child(book_id.clone());
            }
        }

        let existing_children: Vec<_> = self
            .books
            .values()
            .filter(|candidate| candidate.parent_id.as_ref() == Some(&book_id))
            .map(|candidate| candidate.id.clone())
            .collect();

        if let Some(inserted_book) = self.books.get_mut(&book_id) {
            for child_id in existing_children {
                inserted_book.add_child(child_id);
            }
        }

        self
    }

    /// Add multiple books in a single call.
    ///
    /// # Arguments
    ///
    /// * `books` - Iterator yielding books to insert.
    ///
    /// # Returns
    ///
    /// The updated builder for fluent chaining.
    pub fn books(mut self, books: impl IntoIterator<Item = Book>) -> Self {
        for book in books {
            self = self.book(book);
        }
        self
    }

    /// Assign a position to a book.
    ///
    /// This method updates both the position's book_id and adds the position
    /// to the book's position_ids list.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position identifier.
    /// * `book_id` - Book identifier.
    ///
    /// # Returns
    ///
    /// Updated builder with the position assigned to the requested book.
    ///
    /// # Errors
    ///
    /// Returns error if the position or book doesn't exist.
    pub fn add_position_to_book(
        mut self,
        position_id: impl Into<PositionId>,
        book_id: impl Into<BookId>,
    ) -> Result<Self> {
        let pos_id = position_id.into();
        let bk_id = book_id.into();

        // Find the position
        let position = self
            .positions
            .iter_mut()
            .find(|p| p.position_id == pos_id)
            .ok_or_else(|| {
                crate::error::Error::InvalidInput(format!("Position not found: {}", pos_id))
            })?;

        let previous_book_id = position.book_id.clone();

        if let Some(previous_book_id) = previous_book_id {
            if previous_book_id != bk_id {
                if let Some(previous_book) = self.books.get_mut(&previous_book_id) {
                    previous_book.remove_position(&pos_id);
                }
            }
        }

        // Update position's book_id
        position.book_id = Some(bk_id.clone());

        // Add position to book
        let book = self.books.get_mut(&bk_id).ok_or_else(|| {
            crate::error::Error::InvalidInput(format!("Book not found: {}", bk_id))
        })?;

        book.add_position(pos_id);

        Ok(self)
    }

    /// Build the portfolio with validation and entity reconciliation.
    ///
    /// This method performs several checks before returning a portfolio:
    /// 1. Ensures the base currency and valuation date are configured.
    /// 2. Injects a dummy entity when positions reference [`DUMMY_ENTITY_ID`].
    /// 3. Delegates to [`Portfolio::validate`](crate::portfolio::Portfolio::validate) to confirm
    ///    entity references and portfolio structure.
    ///
    /// # Returns
    ///
    /// A validated [`Portfolio`] with derived indices rebuilt.
    ///
    /// # Errors
    ///
    /// Returns [`Error`](crate::error::Error) when the configuration is incomplete
    /// or validation fails.
    pub fn build(mut self) -> Result<Portfolio> {
        let base_ccy = self.base_ccy.ok_or_else(|| {
            crate::error::Error::ValidationFailed("Base currency must be set".to_string())
        })?;

        let as_of = self.as_of.ok_or_else(|| {
            crate::error::Error::ValidationFailed("Valuation date (as_of) must be set".to_string())
        })?;

        // Auto-create dummy entity if needed
        let needs_dummy = self
            .positions
            .iter()
            .any(|p| p.entity_id == DUMMY_ENTITY_ID);

        if needs_dummy && !self.entities.contains_key(DUMMY_ENTITY_ID) {
            tracing::debug!("Auto-creating dummy entity for standalone instruments");
            let dummy = Entity::dummy();
            self.entities.insert(dummy.id.clone(), dummy);
        }

        let mut portfolio = Portfolio {
            id: self.id,
            name: self.name,
            base_ccy,
            as_of,
            entities: self.entities,
            positions: self.positions,
            position_index: finstack_quant_core::HashMap::default(),
            dependency_index: crate::dependencies::DependencyIndex::default(),
            entity_index: finstack_quant_core::HashMap::default(),
            attribute_key_index: finstack_quant_core::HashMap::default(),
            books: self.books,
            tags: self.tags,
            meta: self.meta,
        };

        portfolio.rebuild_index();

        // Validate the portfolio
        portfolio.validate()?;

        Ok(portfolio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position::{Position, PositionUnit};
    use crate::types::Entity;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_builder_basic() {
        let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
            .name("Test Portfolio")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .entity(Entity::new("ACME"))
            .tag("strategy", "fixed_income")
            .build()
            .expect("test should succeed");

        assert_eq!(portfolio.id, "TEST_PORTFOLIO");
        assert_eq!(portfolio.name, Some("Test Portfolio".to_string()));
        assert_eq!(portfolio.base_ccy, Currency::USD);
        assert!(portfolio.entities.contains_key("ACME"));
    }

    #[test]
    fn test_builder_dummy_entity_auto_creation() {
        let deposit = Deposit::builder()
            .id("DEP_1".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(date!(2024 - 01 - 01))
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            DUMMY_ENTITY_ID,
            "DEP_1",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(date!(2024 - 01 - 01))
            .position(position)
            .build()
            .expect("test should succeed");

        // Dummy entity should be auto-created
        assert!(portfolio.has_dummy_entity());
        assert_eq!(portfolio.positions.len(), 1);
    }

    #[test]
    fn test_builder_validation_fails_without_base_ccy() {
        let result = PortfolioBuilder::new("TEST")
            .as_of(date!(2024 - 01 - 01))
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_builder_validation_fails_without_as_of() {
        let result = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .build();

        assert!(result.is_err());
    }
}
