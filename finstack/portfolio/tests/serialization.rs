//! Serialization tests for portfolio.

mod common;

use common::*;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_portfolio::book::{Book, BookId};
use finstack_portfolio::portfolio::PortfolioSpec;
use finstack_portfolio::position::{Position, PositionSpec, PositionUnit};
use finstack_portfolio::types::{AttributeValue, Entity};
use finstack_portfolio::{Portfolio, PortfolioBuilder};
use finstack_valuations::instruments::fixed_income::bond::Bond;
use finstack_valuations::instruments::rates::deposit::Deposit;
use finstack_valuations::instruments::InstrumentJson;
use indexmap::IndexMap;
use serde_json::json;
use std::sync::Arc;
use time::Duration;

#[test]
fn test_position_spec_roundtrip() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let deposit = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let position = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP_1M",
        Arc::new(deposit),
        1.5,
        PositionUnit::Units,
    )
    .unwrap()
    .with_text_attribute("rating", "AAA")
    .with_text_attribute("sector", "Banking");

    // Convert to spec
    let spec = position.to_spec();

    // Verify spec contains all position data
    assert_eq!(spec.position_id, "POS_001");
    assert_eq!(spec.entity_id, "ENTITY_A");
    assert_eq!(spec.instrument_id, "DEP_1M");
    assert_eq!(spec.quantity, 1.5);
    assert_eq!(spec.unit, PositionUnit::Units);
    assert_eq!(
        spec.attributes.get("rating"),
        Some(&AttributeValue::Text("AAA".to_string()))
    );
    assert_eq!(
        spec.attributes.get("sector"),
        Some(&AttributeValue::Text("Banking".to_string()))
    );

    // Note: instrument_spec may be None if Deposit doesn't implement to_instrument_json()
    // This is expected until we implement the conversion for all instrument types
}

#[test]
fn test_portfolio_spec_serialization() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let dep1 = Deposit::builder()
        .id("DEP_1".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let dep2 = Deposit::builder()
        .id("DEP_2".into())
        .notional(Money::new(500_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let pos1 = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP_1",
        Arc::new(dep1),
        1.0,
        PositionUnit::Units,
    )
    .unwrap()
    .with_text_attribute("rating", "AAA");

    let pos2 = Position::new(
        "POS_002",
        "ENTITY_B",
        "DEP_2",
        Arc::new(dep2),
        2.0,
        PositionUnit::Units,
    )
    .unwrap()
    .with_text_attribute("rating", "AA");

    let portfolio = PortfolioBuilder::new("TEST_PORTFOLIO")
        .name("Test Portfolio")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .entity(Entity::new("ENTITY_B"))
        .position(pos1)
        .position(pos2)
        .tag("strategy", "fixed_income")
        .build()
        .unwrap();

    // Convert to spec
    let spec = portfolio.to_spec();

    // Verify spec contains all portfolio data
    assert_eq!(spec.id, "TEST_PORTFOLIO");
    assert_eq!(spec.name, Some("Test Portfolio".to_string()));
    assert_eq!(spec.base_ccy, Currency::USD);
    assert_eq!(spec.as_of, as_of);
    assert_eq!(spec.entities.len(), 2);
    assert_eq!(spec.positions.len(), 2);
    assert_eq!(spec.tags.get("strategy"), Some(&"fixed_income".to_string()));

    // Verify positions are in spec
    assert_eq!(spec.positions[0].position_id, "POS_001");
    assert_eq!(spec.positions[1].position_id, "POS_002");
}

#[test]
fn test_portfolio_spec_json_roundtrip() {
    let as_of = base_date();
    let end_date = as_of + Duration::days(30);

    let deposit = Deposit::builder()
        .id("DEP_1M".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let position = Position::new(
        "POS_001",
        "ENTITY_A",
        "DEP_1M",
        Arc::new(deposit),
        1.0,
        PositionUnit::Units,
    )
    .unwrap();

    let portfolio = PortfolioBuilder::new("TEST")
        .base_ccy(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    // Convert to spec and serialize to JSON
    let spec = portfolio.to_spec();
    let json = serde_json::to_string(&spec).expect("Serialization should succeed");

    // Deserialize back
    let spec_roundtrip: finstack_portfolio::portfolio::PortfolioSpec =
        serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify roundtrip preserved data
    assert_eq!(spec_roundtrip.id, spec.id);
    assert_eq!(spec_roundtrip.base_ccy, spec.base_ccy);
    assert_eq!(spec_roundtrip.as_of, spec.as_of);
    assert_eq!(spec_roundtrip.positions.len(), spec.positions.len());

    // Note: Full reconstruction (from_spec) requires instrument_spec to be Some
    // This will work once we implement to_instrument_json() for all instrument types
}

#[test]
fn test_position_from_spec_preserves_book_tags_and_meta() {
    let mut attributes = IndexMap::new();
    attributes.insert(
        "rating".to_string(),
        AttributeValue::Text("AAA".to_string()),
    );
    let mut meta = IndexMap::new();
    meta.insert("desk".to_string(), json!("credit"));

    let spec = PositionSpec {
        position_id: "POS_001".into(),
        entity_id: "ENTITY_A".into(),
        instrument_id: "BOND_EXAMPLE".to_string(),
        instrument_spec: Some(InstrumentJson::Bond(
            Bond::example().expect("valid bond example"),
        )),
        quantity: 1.0,
        unit: PositionUnit::Units,
        book_id: Some(BookId::new("ig")),
        attributes,
        meta,
    };

    let reconstructed = Position::from_spec(spec).expect("position should reconstruct from spec");

    assert_eq!(reconstructed.book_id, Some(BookId::new("ig")));
    assert_eq!(
        reconstructed.attributes.get("rating"),
        Some(&AttributeValue::Text("AAA".to_string()))
    );
    assert_eq!(reconstructed.meta.get("desk"), Some(&json!("credit")));
}

#[test]
fn test_portfolio_from_spec_preserves_position_metadata() {
    let as_of = base_date();
    let mut entities = IndexMap::new();
    let entity = Entity::new("ENTITY_A");
    entities.insert(entity.id.clone(), entity);

    let mut attributes = IndexMap::new();
    attributes.insert(
        "rating".to_string(),
        AttributeValue::Text("AAA".to_string()),
    );
    let mut meta = IndexMap::new();
    meta.insert("desk".to_string(), json!("credit"));

    let spec = PortfolioSpec {
        id: "TEST".to_string(),
        name: None,
        base_ccy: Currency::USD,
        as_of,
        entities,
        positions: vec![PositionSpec {
            position_id: "POS_001".into(),
            entity_id: "ENTITY_A".into(),
            instrument_id: "BOND_EXAMPLE".to_string(),
            instrument_spec: Some(InstrumentJson::Bond(
                Bond::example().expect("valid bond example"),
            )),
            quantity: 1.0,
            unit: PositionUnit::Units,
            book_id: Some(BookId::new("ig")),
            attributes,
            meta,
        }],
        books: IndexMap::from([(
            BookId::new("ig"),
            Book::new("ig", Some("Investment Grade".to_string())),
        )]),
        tags: IndexMap::new(),
        meta: IndexMap::new(),
    };

    let reconstructed = Portfolio::from_spec(spec).expect("portfolio should reconstruct from spec");
    let restored = reconstructed
        .get_position("POS_001")
        .expect("position should exist after round-trip");

    assert_eq!(restored.book_id, Some(BookId::new("ig")));
    assert_eq!(
        restored.attributes.get("rating"),
        Some(&AttributeValue::Text("AAA".to_string()))
    );
    assert_eq!(restored.meta.get("desk"), Some(&json!("credit")));
}

#[test]
fn test_portfolio_spec_json_deserializes_multiple_instrument_specs() {
    let as_of = base_date();
    let mut entities = IndexMap::new();
    let entity = Entity::new("ENTITY_A");
    entities.insert(entity.id.clone(), entity);

    let portfolio_json = json!({
        "id": "BULK_JSON",
        "name": "bulk portfolio",
        "base_ccy": "USD",
        "as_of": as_of,
        "entities": {
            "ENTITY_A": {
                "id": "ENTITY_A",
                "name": null,
                "entity_type": "legal_entity",
                "parent_id": null,
                "meta": {}
            }
        },
        "positions": [
            {
                "position_id": "POS_BOND",
                "entity_id": "ENTITY_A",
                "instrument_id": "BOND_EXAMPLE",
                "instrument_spec": serde_json::to_value(InstrumentJson::Bond(
                    Bond::example().expect("valid bond example"),
                ))
                .expect("bond instrument json should serialize"),
                "quantity": 1.0,
                "unit": "units",
                "book_id": null,
                "tags": {},
                "meta": {}
            },
            {
                "position_id": "POS_DEPOSIT",
                "entity_id": "ENTITY_A",
                "instrument_id": "DEP_EXAMPLE",
                "instrument_spec": serde_json::to_value(InstrumentJson::Deposit(
                    Deposit::example().expect("valid deposit example"),
                ))
                .expect("deposit instrument json should serialize"),
                "quantity": 2.0,
                "unit": "units",
                "book_id": null,
                "tags": {},
                "meta": {}
            }
        ],
        "books": {},
        "tags": {},
        "meta": {}
    });

    let spec: PortfolioSpec =
        serde_json::from_value(portfolio_json).expect("portfolio json should deserialize");

    assert_eq!(spec.positions.len(), 2);
    assert!(matches!(
        spec.positions[0].instrument_spec,
        Some(InstrumentJson::Bond(_))
    ));
    assert!(matches!(
        spec.positions[1].instrument_spec,
        Some(InstrumentJson::Deposit(_))
    ));
}
