//! Tests for book hierarchy functionality.

use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxProvider};
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::book::{Book, BookId};
use finstack_quant_portfolio::builder::PortfolioBuilder;
use finstack_quant_portfolio::grouping::aggregate_by_book;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::value_portfolio;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;
use time::macros::date;

fn build_test_market() -> MarketContext {
    use finstack_quant_core::dates::Date as CoreDate;

    // Simple static FX provider
    struct StaticFx {
        rate: f64,
    }
    impl FxProvider for StaticFx {
        fn rate(
            &self,
            _from: Currency,
            _to: Currency,
            _on: CoreDate,
            _policy: FxConversionPolicy,
        ) -> finstack_quant_core::Result<f64> {
            Ok(self.rate)
        }
    }

    // Build USD discount curve
    let usd_curve = DiscountCurve::builder("USD")
        .base_date(date!(2024 - 01 - 01))
        .knots(vec![(0.0, 1.0), (1.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .expect("USD curve should build");

    // Build FX matrix
    let fx = FxMatrix::new(Arc::new(StaticFx { rate: 1.10 }));

    // Build market context
    MarketContext::new().insert(usd_curve).insert_fx(fx)
}

#[test]
fn test_book_hierarchy_three_levels() {
    let as_of = date!(2024 - 01 - 01);

    // Create test instruments
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
        .expect("deposit 1 should build");

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
        .expect("deposit 2 should build");

    let dep3 = Deposit::builder()
        .id("DEP_3".into())
        .notional(Money::new(750_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(date!(2024 - 04 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .expect("deposit 3 should build");

    // Create positions
    let pos1 = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP_1",
        Arc::new(dep1),
        1.0,
        PositionUnit::Units,
    )
    .expect("position 1 should build");

    let pos2 = Position::new(
        "POS_002",
        "ENTITY_A",
        "DEP_2",
        Arc::new(dep2),
        1.0,
        PositionUnit::Units,
    )
    .expect("position 2 should build");

    let pos3 = Position::new(
        "POS_003",
        "ENTITY_A",
        "DEP_3",
        Arc::new(dep3),
        1.0,
        PositionUnit::Units,
    )
    .expect("position 3 should build");

    // Create 3-level book hierarchy: Americas > Credit > IG
    let americas = Book::new("americas", Some("Americas".to_string()));
    let credit = Book::new("credit", Some("Credit".to_string())).with_parent("americas");
    let ig = Book::new("ig", Some("Investment Grade".to_string())).with_parent("credit");

    // Build portfolio with books
    let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .book(americas)
        .book(credit)
        .book(ig)
        .position(pos1)
        .position(pos2)
        .position(pos3)
        // Assign positions to books
        .add_position_to_book("POS_001", "ig")
        .expect("should assign POS_001 to ig")
        .add_position_to_book("POS_002", "ig")
        .expect("should assign POS_002 to ig")
        .add_position_to_book("POS_003", "credit")
        .expect("should assign POS_003 to credit")
        .build()
        .expect("portfolio should build");

    // Verify book hierarchy
    assert_eq!(portfolio.books.len(), 3);
    assert!(portfolio.books.contains_key("americas"));
    assert!(portfolio.books.contains_key("credit"));
    assert!(portfolio.books.contains_key("ig"));

    // Verify parent-child relationships
    let americas_book = portfolio
        .books
        .get("americas")
        .expect("americas should exist");
    assert!(americas_book.is_root());
    assert_eq!(americas_book.child_book_ids.len(), 1);
    assert_eq!(americas_book.child_book_ids[0], BookId::new("credit"));

    let credit_book = portfolio.books.get("credit").expect("credit should exist");
    assert!(!credit_book.is_root());
    assert_eq!(credit_book.parent_id, Some(BookId::new("americas")));
    assert_eq!(credit_book.child_book_ids.len(), 1);
    assert_eq!(credit_book.child_book_ids[0], BookId::new("ig"));

    let ig_book = portfolio.books.get("ig").expect("ig should exist");
    assert!(!ig_book.is_root());
    assert_eq!(ig_book.parent_id, Some(BookId::new("credit")));
    assert_eq!(ig_book.child_book_ids.len(), 0);

    // Verify position assignments
    assert_eq!(ig_book.position_ids.len(), 2);
    assert_eq!(credit_book.position_ids.len(), 1);
    assert_eq!(americas_book.position_ids.len(), 0); // No direct positions

    // Value the portfolio
    let market = build_test_market();
    let config = FinstackConfig::default();
    let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
        .expect("valuation should succeed");

    // Aggregate by book with recursive rollup
    let by_book = aggregate_by_book(&valuation, &portfolio.books, Currency::USD)
        .expect("aggregation should succeed");

    // Verify aggregation
    assert_eq!(by_book.len(), 3);

    // IG book should have value of pos1 + pos2
    let ig_total = by_book.get("ig").expect("ig should have total");
    assert!(ig_total.amount() > 0.0);

    // Credit book should have value of pos3 + (pos1 + pos2 from IG child)
    let credit_total = by_book.get("credit").expect("credit should have total");
    assert!(credit_total.amount() > ig_total.amount());

    // Americas book should have value of all positions (rolled up from Credit and IG)
    let americas_total = by_book.get("americas").expect("americas should have total");
    assert!(americas_total.amount() >= credit_total.amount());
}

#[test]
fn test_book_hierarchy_multiple_root_books() {
    let as_of = date!(2024 - 01 - 01);

    // Create test instruments
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
        .expect("deposit should build");

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
        .expect("deposit should build");

    // Create positions
    let pos1 = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP_1",
        Arc::new(dep1),
        1.0,
        PositionUnit::Units,
    )
    .expect("position should build");

    let pos2 = Position::new(
        "POS_002",
        "ENTITY_A",
        "DEP_2",
        Arc::new(dep2),
        1.0,
        PositionUnit::Units,
    )
    .expect("position should build");

    // Create multiple root books
    let americas = Book::new("americas", Some("Americas".to_string()));
    let asia = Book::new("asia", Some("Asia".to_string()));

    // Build portfolio
    let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .book(americas)
        .book(asia)
        .position(pos1)
        .position(pos2)
        .add_position_to_book("POS_001", "americas")
        .expect("should assign to americas")
        .add_position_to_book("POS_002", "asia")
        .expect("should assign to asia")
        .build()
        .expect("portfolio should build");

    // Verify both root books
    assert_eq!(portfolio.books.len(), 2);

    let americas_book = portfolio
        .books
        .get("americas")
        .expect("americas should exist");
    assert!(americas_book.is_root());

    let asia_book = portfolio.books.get("asia").expect("asia should exist");
    assert!(asia_book.is_root());

    // Value and aggregate
    let market = build_test_market();
    let config = FinstackConfig::default();
    let valuation = value_portfolio(&portfolio, &market, &config, &Default::default())
        .expect("valuation should succeed");

    let by_book = aggregate_by_book(&valuation, &portfolio.books, Currency::USD)
        .expect("aggregation should succeed");

    // Each root book should have independent totals
    assert_eq!(by_book.len(), 2);
    assert!(by_book.contains_key("americas"));
    assert!(by_book.contains_key("asia"));
}

#[test]
fn test_position_without_book() {
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
        .expect("deposit should build");

    let pos1 = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP_1",
        Arc::new(dep1),
        1.0,
        PositionUnit::Units,
    )
    .expect("position should build");

    // Build portfolio without assigning position to any book
    let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .position(pos1)
        .build()
        .expect("portfolio should build");

    // Verify position has no book
    assert_eq!(portfolio.positions()[0].book_id, None);
    assert_eq!(portfolio.books.len(), 0);
}

#[test]
fn test_book_hierarchy_is_order_independent() {
    let as_of = date!(2024 - 01 - 01);

    let deposit = Deposit::builder()
        .id("DEP".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(date!(2024 - 02 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .expect("deposit should build");

    let position = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP",
        Arc::new(deposit),
        1.0,
        PositionUnit::Units,
    )
    .expect("position should build");

    let americas = Book::new("americas", Some("Americas".to_string()));
    let credit = Book::new("credit", Some("Credit".to_string())).with_parent("americas");
    let ig = Book::new("ig", Some("Investment Grade".to_string())).with_parent("credit");

    let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .book(ig)
        .book(credit)
        .book(americas)
        .position(position)
        .add_position_to_book("POS_001", "ig")
        .expect("should assign position to ig")
        .build()
        .expect("portfolio should build");

    let americas_book = portfolio
        .books
        .get("americas")
        .expect("americas should exist");
    let credit_book = portfolio.books.get("credit").expect("credit should exist");

    assert_eq!(
        americas_book.child_book_ids.as_slice(),
        &[BookId::new("credit")]
    );
    assert_eq!(credit_book.child_book_ids.as_slice(), &[BookId::new("ig")]);
}

#[test]
fn test_reassigning_position_between_books_removes_stale_membership() {
    let as_of = date!(2024 - 01 - 01);

    let deposit = Deposit::builder()
        .id("DEP".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(date!(2024 - 02 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .quote_rate_opt(Some(
            rust_decimal::Decimal::try_from(0.045).expect("valid literal"),
        ))
        .build()
        .expect("deposit should build");

    let position = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP",
        Arc::new(deposit),
        1.0,
        PositionUnit::Units,
    )
    .expect("position should build");

    let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .book(Book::new("book_a", Some("Book A".to_string())))
        .book(Book::new("book_b", Some("Book B".to_string())))
        .position(position)
        .add_position_to_book("POS_001", "book_a")
        .expect("should assign to book_a")
        .add_position_to_book("POS_001", "book_b")
        .expect("should reassign to book_b")
        .build()
        .expect("portfolio should build");

    let book_a = portfolio.books.get("book_a").expect("book_a should exist");
    let book_b = portfolio.books.get("book_b").expect("book_b should exist");
    let position = portfolio
        .get_position("POS_001")
        .expect("position should exist");

    assert!(book_a.position_ids.is_empty());
    assert_eq!(book_b.position_ids.len(), 1);
    assert_eq!(book_b.position_ids[0], "POS_001");
    assert_eq!(position.book_id, Some(BookId::new("book_b")));
}
