//! Book-hierarchy aggregation.
//!
//! Rolls portfolio valuations up the parent/child book tree defined by
//! [`Book::child_book_ids`](crate::book::Book::child_book_ids), summing each book's own positions together with
//! every descendant book's total.
//!
//! Flat, single-attribute slicing (by rating, sector, currency, …) is left to
//! the caller: Python consumers get it from `pandas.DataFrame.groupby` over the
//! position DataFrame, which is more flexible than any fixed Rust helper.

use crate::book::{Book, BookId};
use crate::error::Result;
use crate::valuation::PortfolioValuation;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::{HashMap, HashSet};
use indexmap::IndexMap;

const MAX_BOOK_GROUPING_RECURSION_DEPTH: usize = 512;

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
/// * `base_currency` - Currency used when adding monetary amounts.
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
/// ```
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
    base_currency: Currency,
) -> Result<IndexMap<BookId, Money>> {
    let mut book_totals: IndexMap<BookId, Money> = IndexMap::new();

    // Build a map of position values by position_id for quick lookup
    let position_values: HashMap<&crate::types::PositionId, &Money> = valuation
        .position_values
        .iter()
        .map(|(id, val)| (id, &val.value_base))
        .collect();

    fn compute_book_total(
        book_id: &BookId,
        books: &IndexMap<BookId, Book>,
        position_values: &HashMap<&crate::types::PositionId, &Money>,
        base_currency: Currency,
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
        let mut total = Money::from((0_i64, base_currency));

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
                base_currency,
                memo,
                visiting,
                depth + 1,
            )?;
            total = total.checked_add(child_total)?;
        }

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
            base_currency,
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
    use crate::valuation::PortfolioValuation;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;
    use time::macros::date;

    fn empty_valuation() -> PortfolioValuation {
        PortfolioValuation {
            as_of: date!(2024 - 01 - 01),
            position_values: IndexMap::new(),
            total_base_currency: Money::from((0_i64, Currency::USD)),
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
