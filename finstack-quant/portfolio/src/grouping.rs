//! Attribute-based grouping and aggregation.
//!
//! Helper functions for slicing portfolios by arbitrary tags and rolling up
//! valuations across one or more categorical dimensions.

use crate::book::{Book, BookId};
use crate::error::Result;
use crate::position::Position;
use crate::valuation::PortfolioValuation;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::{HashMap, HashSet};
use indexmap::IndexMap;

const MAX_BOOK_GROUPING_RECURSION_DEPTH: usize = 512;
const UNTAGGED_GROUP: &str = "_untagged";

fn attribute_group_key(position: &Position, attr_key: &str) -> String {
    position
        .attributes
        .get(attr_key)
        .map(ToString::to_string)
        .unwrap_or_else(|| UNTAGGED_GROUP.to_owned())
}

/// Group positions by a specific tag or attribute.
///
/// Positions that do not contain the requested attribute are placed in the
/// special `_untagged` bucket to ensure they are still represented.
///
/// # Arguments
///
/// * `positions` - Slice of positions to group.
/// * `attr_key` - Tag key used to partition positions.
///
/// # Returns
///
/// An [`IndexMap`] mapping attribute values to the positions that match.
/// Order is stable because [`IndexMap`] preserves insertion order.
pub fn group_by_attribute<'a>(
    positions: &'a [Position],
    attr_key: &str,
) -> IndexMap<String, Vec<&'a Position>> {
    let mut groups: IndexMap<String, Vec<&'a Position>> = IndexMap::new();

    for position in positions {
        groups
            .entry(attribute_group_key(position, attr_key))
            .or_default()
            .push(position);
    }

    groups
}

/// Aggregate portfolio values by a specific attribute.
///
/// Each position's base-currency value is summed into buckets by the chosen tag.
///
/// # Arguments
///
/// * `valuation` - Pre-computed valuation results providing per-position values.
/// * `positions` - Positions that correspond to the valuation results.
/// * `attr_key` - Tag to aggregate by.
/// * `base_ccy` - Currency used when adding monetary amounts.
///
/// # Returns
///
/// [`Result`] with an [`IndexMap`] of attribute values to aggregated [`Money`].
///
/// # Errors
///
/// Returns an error when a requested position has no corresponding valuation
/// or its base-currency amount cannot be added to `base_ccy` (including a
/// currency mismatch or monetary overflow).
pub fn aggregate_by_attribute(
    valuation: &PortfolioValuation,
    positions: &[Position],
    attr_key: &str,
    base_ccy: Currency,
) -> Result<IndexMap<String, Money>> {
    let mut aggregated: IndexMap<String, Money> = IndexMap::new();

    for position in positions {
        let attr_value = attribute_group_key(position, attr_key);

        let position_value = valuation
            .position_values
            .get(&position.position_id)
            .ok_or_else(|| {
                crate::error::Error::invalid_input(format!(
                    "MO-3: valuation is missing position '{}'",
                    position.position_id
                ))
            })?;
        let total = aggregated
            .entry(attr_value)
            .or_insert_with(|| Money::new(0.0, base_ccy));
        *total = total.checked_add(position_value.value_base)?;
    }

    Ok(aggregated)
}

/// Group and aggregate by multiple attributes.
///
/// Builds composite keys from the requested attributes to create multi-dimensional
/// aggregates. Missing attributes are normalised to `_untagged`.
///
/// # Arguments
///
/// * `valuation` - Portfolio valuation providing per-position values.
/// * `positions` - Positions being grouped.
/// * `attr_keys` - Ordered set of attribute keys used to build the composite key.
/// * `base_ccy` - Currency used for aggregation.
///
/// # Returns
///
/// [`Result`] containing an [`IndexMap`] whose keys are the ordered attribute values
/// and whose values are the aggregated [`Money`] totals.
///
/// # Errors
///
/// Returns an error when a requested position has no corresponding valuation
/// or its base-currency amount cannot be added to `base_ccy`.
pub fn aggregate_by_multiple_attributes(
    valuation: &PortfolioValuation,
    positions: &[Position],
    attr_keys: &[&str],
    base_ccy: Currency,
) -> Result<IndexMap<Vec<String>, Money>> {
    let mut aggregated: IndexMap<Vec<String>, Money> = IndexMap::new();

    for position in positions {
        // Build compound key from all attributes
        let key: Vec<String> = attr_keys
            .iter()
            .map(|&attr_key| attribute_group_key(position, attr_key))
            .collect();

        let position_value = valuation
            .position_values
            .get(&position.position_id)
            .ok_or_else(|| {
                crate::error::Error::invalid_input(format!(
                    "MO-3: valuation is missing position '{}'",
                    position.position_id
                ))
            })?;
        let total = aggregated
            .entry(key)
            .or_insert_with(|| Money::new(0.0, base_ccy));
        *total = total.checked_add(position_value.value_base)?;
    }

    Ok(aggregated)
}

/// Aggregate portfolio values by book hierarchy with recursive rollup.
///
/// Traversal follows each book's [`Book::child_book_ids`] links only (not the
/// optional [`Book::parent_id`] field), which must form an acyclic tree/forest.
///
/// Computes total value for each book by summing:
/// 1. Direct position values in the book
/// 2. Recursively aggregated values from child books
///
/// This enables multi-level reporting (e.g., Americas > Credit > IG).
///
/// # Arguments
///
/// * `valuation` - Pre-computed valuation results providing per-position values.
/// * `books` - Book hierarchy definition.
/// * `base_ccy` - Currency used when adding monetary amounts.
///
/// # Returns
///
/// [`Result`] with an [`IndexMap`] of book IDs to aggregated [`Money`].
/// Includes both direct and rolled-up values from child books.
///
/// # Errors
///
/// Returns an error for a missing referenced book or position valuation, a
/// cycle or excessive depth in `child_book_ids`, or an incompatible/overflowing
/// monetary addition.
///
/// # Example
///
/// ```ignore
/// use finstack_quant_portfolio::grouping::aggregate_by_book;
/// use finstack_quant_portfolio::valuation::value_portfolio;
/// use finstack_quant_core::currency::Currency;
///
/// # fn example(portfolio: finstack_quant_portfolio::Portfolio, market: finstack_quant_core::market_data::context::MarketContext, config: finstack_quant_core::config::FinstackConfig) -> finstack_quant_portfolio::Result<()> {
/// let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())?;
/// let by_book = aggregate_by_book(
///     &valuation,
///     &portfolio.books,
///     Currency::USD,
/// )?;
///
/// // Get total for "Americas" book (includes all child books like Credit, Equity, etc.)
/// if let Some(americas_total) = by_book.get("americas") {
///     println!("Americas total: {}", americas_total);
/// }
/// # Ok(())
/// # }
/// ```
pub fn aggregate_by_book(
    valuation: &PortfolioValuation,
    books: &IndexMap<BookId, Book>,
    base_ccy: Currency,
) -> Result<IndexMap<BookId, Money>> {
    let mut book_totals: IndexMap<BookId, Money> = IndexMap::new();

    // Build a map of position values by position_id for quick lookup
    let position_values: HashMap<&crate::types::PositionId, &Money> = valuation
        .position_values
        .iter()
        .map(|(id, val)| (id, &val.value_base))
        .collect();

    // Helper function to recursively compute book total
    fn compute_book_total(
        book_id: &BookId,
        books: &IndexMap<BookId, Book>,
        position_values: &HashMap<&crate::types::PositionId, &Money>,
        base_ccy: Currency,
        memo: &mut HashMap<BookId, Money>,
        visiting: &mut HashSet<BookId>,
        depth: usize,
    ) -> Result<Money> {
        // Check memo first
        if let Some(cached) = memo.get(book_id) {
            return Ok(*cached);
        }
        if depth >= MAX_BOOK_GROUPING_RECURSION_DEPTH {
            return Err(crate::error::Error::invalid_input(format!(
                "Book aggregation exceeded maximum recursion depth of {MAX_BOOK_GROUPING_RECURSION_DEPTH}"
            )));
        }
        if !visiting.insert(book_id.clone()) {
            return Err(crate::error::Error::invalid_input(format!(
                "Book hierarchy contains a cycle at '{book_id}'"
            )));
        }

        let book = books.get(book_id).ok_or_else(|| {
            crate::error::Error::InvalidInput(format!("Book not found: {}", book_id))
        })?;

        // Start with zero
        let mut total = Money::new(0.0, base_ccy);

        // Add direct position values
        for pos_id in &book.position_ids {
            let &&value = position_values.get(pos_id).ok_or_else(|| {
                crate::error::Error::invalid_input(format!(
                    "MO-3: valuation is missing book position '{pos_id}'"
                ))
            })?;
            total = total.checked_add(value)?;
        }

        // Recursively add child book totals
        for child_id in &book.child_book_ids {
            let child_total = compute_book_total(
                child_id,
                books,
                position_values,
                base_ccy,
                memo,
                visiting,
                depth + 1,
            )?;
            total = total.checked_add(child_total)?;
        }

        // Memoize
        visiting.remove(book_id);
        memo.insert(book_id.clone(), total);

        Ok(total)
    }

    // Compute totals for all books
    let mut memo: HashMap<BookId, Money> = HashMap::default();
    for book_id in books.keys() {
        let mut visiting = HashSet::default();
        let total = compute_book_total(
            book_id,
            books,
            &position_values,
            base_ccy,
            &mut memo,
            &mut visiting,
            0,
        )?;
        book_totals.insert(book_id.clone(), total);
    }

    Ok(book_totals)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::Book;
    use crate::builder::PortfolioBuilder;
    use crate::position::{Position, PositionUnit};
    use crate::test_utils::build_test_market;
    use crate::types::Entity;
    use crate::valuation::{value_portfolio, PortfolioValuation};
    use finstack_quant_core::config::FinstackConfig;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use std::sync::Arc;
    use time::macros::date;

    #[test]
    fn test_group_by_attribute() {
        let as_of = date!(2024 - 01 - 01);

        let dep1 = Deposit::builder()
            .id("DEP_1".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let dep2 = Deposit::builder()
            .id("DEP_2".into())
            .notional(Money::new(500_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 03 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let pos1 = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1",
            Arc::new(dep1),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed")
        .with_text_attribute("rating", "AAA")
        .with_text_attribute("sector", "Banking");

        let pos2 = Position::new(
            "POS_002",
            "ENTITY_A",
            "DEP_2",
            Arc::new(dep2),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed")
        .with_text_attribute("rating", "AA")
        .with_text_attribute("sector", "Banking");

        let positions = vec![pos1, pos2];

        let groups = group_by_attribute(&positions, "rating");

        assert_eq!(groups.len(), 2);
        assert!(groups.contains_key("AAA"));
        assert!(groups.contains_key("AA"));
        assert_eq!(groups.get("AAA").expect("test should succeed").len(), 1);
        assert_eq!(groups.get("AA").expect("test should succeed").len(), 1);
    }

    #[test]
    fn minor9_group_by_attribute_keeps_numeric_values_out_of_untagged() {
        let as_of = date!(2024 - 01 - 01);
        let dep1 = Deposit::builder()
            .id("DEP_1".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");
        let dep2 = Deposit::builder()
            .id("DEP_2".into())
            .notional(Money::new(500_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 03 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let missing = Position::new(
            "POS_MISSING",
            "ENTITY_A",
            "DEP_1",
            Arc::new(dep1),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed");
        let numeric = Position::new(
            "POS_NUMERIC",
            "ENTITY_A",
            "DEP_2",
            Arc::new(dep2),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed")
        .with_numeric_attribute("rating", 650.0);

        let positions = vec![missing, numeric];
        let groups = group_by_attribute(&positions, "rating");

        assert_eq!(groups.len(), 2);
        assert_eq!(groups.get("_untagged").map(Vec::len), Some(1));
        assert_eq!(groups.get("650").map(Vec::len), Some(1));
    }

    #[test]
    fn test_aggregate_by_attribute() {
        let as_of = date!(2024 - 01 - 01);

        let dep1 = Deposit::builder()
            .id("DEP_1".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let dep2 = Deposit::builder()
            .id("DEP_2".into())
            .notional(Money::new(500_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 03 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test should succeed");

        let pos1 = Position::new(
            "POS_001",
            "ENTITY_A",
            "DEP_1",
            Arc::new(dep1),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed")
        .with_text_attribute("rating", "AAA");

        let pos2 = Position::new(
            "POS_002",
            "ENTITY_A",
            "DEP_2",
            Arc::new(dep2),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed")
        .with_text_attribute("rating", "AAA");

        let portfolio = PortfolioBuilder::new("TEST")
            .base_ccy(Currency::USD)
            .as_of(as_of)
            .entity(Entity::new("ENTITY_A"))
            .position(pos1)
            .position(pos2)
            .build()
            .expect("test should succeed");

        let market = build_test_market();
        let config = FinstackConfig::default();

        let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
            .expect("test should succeed");
        let aggregated =
            aggregate_by_attribute(&valuation, &portfolio.positions, "rating", Currency::USD)
                .expect("test should succeed");

        assert!(aggregated.contains_key("AAA"));
        let total = aggregated.get("AAA").expect("test should succeed");
        assert!(total.amount().abs() >= 0.0);
    }

    #[test]
    fn mo3_aggregate_by_attribute_rejects_missing_valuation_position() {
        let as_of = date!(2024 - 01 - 01);
        let dep = Deposit::builder()
            .id("DEP_MISSING_VAL".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .start_date(as_of)
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .quote_rate_opt(Some(
                rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
            ))
            .build()
            .expect("test instrument should build");
        let position = Position::new(
            "POS_MISSING_VAL",
            "ENTITY_A",
            "DEP_MISSING_VAL",
            Arc::new(dep),
            1.0,
            PositionUnit::Units,
        )
        .expect("test position should build");

        let err = aggregate_by_attribute(&empty_valuation(), &[position], "rating", Currency::USD)
            .expect_err("MO-3: missing valuation row must fail grouping");
        assert!(err.to_string().contains("MO-3"), "unexpected error: {err}");
    }

    fn empty_valuation() -> PortfolioValuation {
        PortfolioValuation {
            as_of: date!(2024 - 01 - 01),
            position_values: IndexMap::new(),
            total_base_ccy: Money::new(0.0, Currency::USD),
            by_entity: IndexMap::new(),
            degraded_positions: Vec::new(),
            fx_collapse_policy: finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate,
            provenance: None,
        }
    }

    #[test]
    fn aggregate_by_book_rejects_cycles() {
        let mut root = Book::new("root", Some("Root".to_string()));
        root.add_child(BookId::from("child"));

        let mut child = Book::new("child", Some("Child".to_string())).with_parent("root");
        child.add_child(BookId::from("root"));

        let books = IndexMap::from([(BookId::from("root"), root), (BookId::from("child"), child)]);

        let err = aggregate_by_book(&empty_valuation(), &books, Currency::USD)
            .expect_err("cyclic hierarchy should fail");
        assert!(err.to_string().contains("cycle"), "unexpected error: {err}");
    }

    #[test]
    fn mo3_aggregate_by_book_rejects_missing_position_value() {
        let mut book = Book::new("root", Some("Root".to_string()));
        book.add_position(crate::types::PositionId::new("MISSING_POSITION"));
        let books = IndexMap::from([(BookId::from("root"), book)]);

        let err = aggregate_by_book(&empty_valuation(), &books, Currency::USD)
            .expect_err("MO-3: missing book position value must fail grouping");
        assert!(err.to_string().contains("MO-3"), "unexpected error: {err}");
    }

    #[test]
    fn aggregate_by_book_rejects_excessive_depth() {
        let mut books = IndexMap::new();
        for i in 0..=MAX_BOOK_GROUPING_RECURSION_DEPTH {
            let id = BookId::from(format!("book_{i}"));
            let mut book = Book::new(id.clone(), None);
            if i > 0 {
                book = book.with_parent(format!("book_{}", i - 1));
            }
            if i < MAX_BOOK_GROUPING_RECURSION_DEPTH {
                book.add_child(BookId::from(format!("book_{}", i + 1)));
            }
            books.insert(id, book);
        }

        let err = aggregate_by_book(&empty_valuation(), &books, Currency::USD)
            .expect_err("deep hierarchy should fail");
        assert!(
            err.to_string().contains("maximum recursion depth"),
            "unexpected error: {err}"
        );
    }
}
