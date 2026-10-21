//! Serialization tests for portfolio.

use crate::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::book::{Book, BookId};
use finstack_quant_portfolio::position::{Position, PositionSpec, PositionUnit};
use finstack_quant_portfolio::types::{AttributeValue, Entity};
use finstack_quant_portfolio::Portfolio;
use finstack_quant_portfolio::PortfolioSpec;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::InstrumentJson;
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
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
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
    .with_attribute("rating", "AAA")
    .with_attribute("sector", "Banking");

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
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD".into())
        .build()
        .unwrap();

    let dep2 = Deposit::builder()
        .id("DEP_2".into())
        .notional(Money::new(500_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
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
    .with_attribute("rating", "AAA");

    let pos2 = Position::new(
        "POS_002",
        "ENTITY_B",
        "DEP_2",
        Arc::new(dep2),
        2.0,
        PositionUnit::Units,
    )
    .unwrap()
    .with_attribute("rating", "AA");

    let portfolio = Portfolio::builder("TEST_PORTFOLIO")
        .name("Test Portfolio")
        .base_currency(Currency::USD)
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
    assert_eq!(spec.base_currency, Currency::USD);
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
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .start_date(as_of)
        .maturity(end_date)
        .day_count(finstack_quant_core::dates::DayCount::Act360)
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

    let portfolio = Portfolio::builder("TEST")
        .base_currency(Currency::USD)
        .as_of(as_of)
        .entity(Entity::new("ENTITY_A"))
        .position(position)
        .build()
        .unwrap();

    // Convert to spec and serialize to JSON
    let spec = portfolio.to_spec();
    let json = serde_json::to_string(&spec).expect("Serialization should succeed");

    // Deserialize back
    let spec_roundtrip: finstack_quant_portfolio::PortfolioSpec =
        serde_json::from_str(&json).expect("Deserialization should succeed");

    // Verify roundtrip preserved data
    assert_eq!(spec_roundtrip.id, spec.id);
    assert_eq!(spec_roundtrip.base_currency, spec.base_currency);
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

    let mut book = Book::new("ig", Some("Investment Grade".to_string()));
    book.add_position("POS_001".into());
    let spec = PortfolioSpec {
        id: "TEST".to_string(),
        name: None,
        base_currency: Currency::USD,
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
        books: IndexMap::from([(BookId::new("ig"), book)]),
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
        "base_currency": "USD",
        "as_of": as_of,
        "entities": {
            "ENTITY_A": {
                "id": "ENTITY_A",
                "name": null,
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

/// Serializes `value`, adds an unknown top-level field and asserts that
/// deserializing it back is rejected, while the clean payload still parses.
fn assert_rejects_unknown_field<T>(value: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let type_name = std::any::type_name::<T>();
    let mut payload = serde_json::to_value(value).expect("serialize inbound spec");
    serde_json::from_value::<T>(payload.clone())
        .unwrap_or_else(|error| panic!("{type_name}: clean payload must parse: {error}"));
    payload
        .as_object_mut()
        .expect("inbound spec serializes as an object")
        .insert("unexpected_field".to_string(), json!(1));
    let error = serde_json::from_value::<T>(payload)
        .err()
        .unwrap_or_else(|| panic!("{type_name}: unknown field must be rejected"));
    assert!(
        error.to_string().contains("unexpected_field"),
        "{type_name}: error must name the field, got: {error}"
    );
}

#[test]
fn inbound_specs_reject_unknown_fields() {
    use finstack_quant_portfolio::brinson::{BrinsonPeriodResult, SectorEffect, SectorPeriod};
    use finstack_quant_portfolio::cashflows::CashflowAggregationOptions;
    use finstack_quant_portfolio::optimization::{
        MetricExpr, MissingMetricPolicy, Objective, PerPositionMetric, PortfolioOptimizationSpec,
        WeightingScheme,
    };
    use finstack_quant_portfolio::performance::DatedCashflow;
    use finstack_quant_portfolio::valuation::PortfolioValuationOptions;
    use finstack_quant_valuations::metrics::MetricId;

    let as_of = time::macros::date!(2025 - 01 - 01);
    let position = PositionSpec {
        position_id: "POS-1".into(),
        entity_id: "FUND".into(),
        instrument_id: "DEP-1".to_string(),
        instrument_spec: None,
        quantity: 1.0,
        unit: PositionUnit::Units,
        book_id: None,
        attributes: IndexMap::new(),
        meta: IndexMap::new(),
    };
    let portfolio = PortfolioSpec {
        id: "BOOK".to_string(),
        name: None,
        base_currency: Currency::USD,
        as_of,
        entities: IndexMap::new(),
        positions: vec![position.clone()],
        books: IndexMap::new(),
        tags: IndexMap::new(),
        meta: IndexMap::new(),
    };
    let sector_effect = SectorEffect {
        sector: "rates".to_string(),
        allocation: 0.01,
        selection: 0.02,
        interaction: 0.0,
        total: 0.03,
    };

    assert_rejects_unknown_field(&portfolio);
    assert_rejects_unknown_field(&position);
    assert_rejects_unknown_field(&PortfolioValuationOptions::default());
    assert_rejects_unknown_field(&CashflowAggregationOptions::default());
    assert_rejects_unknown_field(&PortfolioOptimizationSpec {
        portfolio,
        objective: Objective::Maximize(MetricExpr::ValueWeightedAverage {
            metric: PerPositionMetric::Metric(MetricId::Ytm),
            filter: None,
        }),
        constraints: Vec::new(),
        weighting: WeightingScheme::ValueWeight,
        missing_metric_policy: MissingMetricPolicy::default(),
        label: None,
        trade_universe: None,
    });
    assert_rejects_unknown_field(&SectorPeriod {
        sector: "rates".to_string(),
        portfolio_weight: 0.6,
        benchmark_weight: 0.5,
        portfolio_return: 0.02,
        benchmark_return: 0.01,
    });
    assert_rejects_unknown_field(&sector_effect);
    assert_rejects_unknown_field(&BrinsonPeriodResult {
        sectors: vec![sector_effect],
        total_allocation: 0.01,
        total_selection: 0.02,
        total_interaction: 0.0,
        portfolio_return: 0.04,
        benchmark_return: 0.01,
        total_excess_return: 0.03,
    });
    assert_rejects_unknown_field(&DatedCashflow {
        date: as_of,
        amount: 100.0,
    });
}
