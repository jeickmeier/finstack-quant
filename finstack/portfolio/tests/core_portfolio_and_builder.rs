//! Core portfolio and builder tests for portfolio.

mod common;

use common::*;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_portfolio::position::{Position, PositionUnit};
use finstack_portfolio::types::{AttributeValue, Entity, DUMMY_ENTITY_ID};
use finstack_portfolio::{Error, Portfolio, PortfolioBuilder};
use finstack_valuations::instruments::rates::deposit::Deposit;
use std::sync::Arc;

#[test]
fn getters_and_tag_filters() {
    let as_of = base_date();
    let maturity = as_of + time::Duration::days(1);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let p = Position::new("P", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units)
        .unwrap()
        .with_text_attribute("sector", "Tech");

    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .position(p)
        .build()
        .unwrap();

    assert!(portfolio.get_position("P").is_some());
    assert_eq!(portfolio.positions_for_entity("E").len(), 1);
    assert_eq!(
        portfolio
            .positions_with_attribute("sector", &AttributeValue::Text("Tech".to_string()))
            .len(),
        1
    );
}

#[test]
fn validate_unknown_entity_fails() {
    let as_of = base_date();
    let maturity = as_of + time::Duration::days(1);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let p = Position::new("P", "UNKNOWN", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();

    let err = Portfolio::builder("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .position(p)
        .build()
        .unwrap_err();
    match err {
        Error::UnknownEntity { .. } => {}
        other => panic!("unexpected error: {:?}", other),
    }
}

#[test]
fn explicit_position_mutators_keep_lookup_index_in_sync() {
    let as_of = base_date();
    let maturity = as_of + time::Duration::days(1);

    let dep1 = Deposit::builder()
        .id("D1".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();
    let dep2 = Deposit::builder()
        .id("D2".into())
        .notional(Money::new(2_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let pos1 = Position::new("P1", "E", "D1", Arc::new(dep1), 1.0, PositionUnit::Units).unwrap();
    let pos2 = Position::new("P2", "E", "D2", Arc::new(dep2), 1.0, PositionUnit::Units).unwrap();

    let mut portfolio = Portfolio::builder("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .build()
        .unwrap();

    portfolio.add_position(pos1).unwrap();
    assert_eq!(portfolio.positions().len(), 1);
    assert!(portfolio.get_position("P1").is_some());

    portfolio.set_positions(vec![pos2]).unwrap();
    assert_eq!(portfolio.positions().len(), 1);
    assert!(portfolio.get_position("P1").is_none());
    assert!(portfolio.get_position("P2").is_some());
}

#[test]
fn set_positions_rejects_duplicate_position_ids() {
    let as_of = base_date();
    let maturity = as_of + time::Duration::days(1);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let pos = Position::new("P1", "E", "D", Arc::new(dep), 1.0, PositionUnit::Units).unwrap();

    let mut portfolio = Portfolio::builder("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("E"))
        .build()
        .unwrap();

    let err = portfolio
        .set_positions(vec![pos.clone(), pos])
        .expect_err("duplicate position IDs should be rejected");
    assert!(err.to_string().contains("Duplicate position ID: P1"));
    assert!(portfolio.positions().is_empty());
}

#[test]
fn builder_required_fields_and_dummy_auto_create() {
    let as_of = base_date();
    let maturity = as_of + time::Duration::days(1);

    let dep = Deposit::builder()
        .id("D".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(maturity)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let p = Position::new(
        "P",
        DUMMY_ENTITY_ID,
        "D",
        Arc::new(dep),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    // Missing base_ccy
    assert!(PortfolioBuilder::new("P").as_of(as_of).build().is_err());
    // Missing as_of
    assert!(PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .build()
        .is_err());

    // Dummy should be auto-created because position references it
    let portfolio = PortfolioBuilder::new("P")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .position(p)
        .build()
        .unwrap();
    assert!(portfolio.has_dummy_entity());
}
