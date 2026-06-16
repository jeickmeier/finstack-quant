//! Serialization tests for bond future types.
//!
//! Tests JSON serialization round-trips and wire format stability for:
//! - `BondFuture` (all fields)
//! - `DeliverableBond`
//! - `BondFutureSpecs`
//!
//! Ensures `deny_unknown_fields` is enforced and deserialization defaults.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::bond_future::{
    BondFuture, BondFutureSpecs, DeliverableBond, Position,
};
use finstack_quant_valuations::instruments::Attributes;
use time::Month;

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test deliverable bond.
fn create_test_deliverable_bond() -> DeliverableBond {
    DeliverableBond {
        bond_id: InstrumentId::new("US912828XG33"),
        conversion_factor: 0.8234,
    }
}

/// Create a minimal valid bond future for testing.
fn create_test_bond_future() -> BondFuture {
    BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .expect("Valid bond future")
}

/// Create a bond future with multiple deliverable bonds.
fn create_bond_future_with_basket() -> BondFuture {
    let basket = vec![
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        },
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XH15"),
            conversion_factor: 0.7891,
        },
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XJ71"),
            conversion_factor: 0.8567,
        },
    ];

    BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(10_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Short)
        .contract_specs(BondFutureSpecs::ust_10y())
        .deliverable_basket(basket)
        .ctd_bond_id(InstrumentId::new("US912828XH15"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new().with_meta("exchange", "CBOT"))
        .build()
        .expect("Valid bond future")
}

// ============================================================================
// DeliverableBond Serialization Tests
// ============================================================================

#[test]
fn test_deliverable_bond_serde_roundtrip() {
    let deliverable = create_test_deliverable_bond();

    let json = serde_json::to_string(&deliverable).expect("Serialization failed");
    let deserialized: DeliverableBond =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deliverable.bond_id, deserialized.bond_id);
    assert_eq!(
        deliverable.conversion_factor,
        deserialized.conversion_factor
    );
}

#[test]
fn test_deliverable_bond_json_structure() {
    let deliverable = create_test_deliverable_bond();
    let json = serde_json::to_string_pretty(&deliverable).expect("Serialization failed");

    // Verify JSON contains expected fields
    assert!(json.contains("bond_id"));
    assert!(json.contains("conversion_factor"));
    assert!(json.contains("US912828XG33"));
    assert!(json.contains("0.8234"));
}

#[test]
fn test_deliverable_bond_array_roundtrip() {
    let basket = vec![
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XG33"),
            conversion_factor: 0.8234,
        },
        DeliverableBond {
            bond_id: InstrumentId::new("US912828XH15"),
            conversion_factor: 0.7891,
        },
    ];

    let json = serde_json::to_string(&basket).expect("Serialization failed");
    let deserialized: Vec<DeliverableBond> =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(basket.len(), deserialized.len());
    for (original, parsed) in basket.iter().zip(deserialized.iter()) {
        assert_eq!(original.bond_id, parsed.bond_id);
        assert_eq!(original.conversion_factor, parsed.conversion_factor);
    }
}

// ============================================================================
// BondFutureSpecs Serialization Tests
// ============================================================================

#[test]
fn test_bond_future_specs_default_roundtrip() {
    let specs = BondFutureSpecs::default();

    let json = serde_json::to_string(&specs).expect("Serialization failed");
    let deserialized: BondFutureSpecs =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(specs.contract_size, deserialized.contract_size);
    assert_eq!(specs.tick_size, deserialized.tick_size);
    assert_eq!(specs.tick_value, deserialized.tick_value);
    assert_eq!(specs.standard_coupon, deserialized.standard_coupon);
    assert_eq!(
        specs.standard_maturity_years,
        deserialized.standard_maturity_years
    );
    assert_eq!(specs.settlement_days, deserialized.settlement_days);
}

#[test]
fn test_bond_future_specs_ust_10y_roundtrip() {
    let specs = BondFutureSpecs::ust_10y();
    let json = serde_json::to_string(&specs).expect("Serialization failed");
    let _deserialized: BondFutureSpecs =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(specs.contract_size, 100_000.0);
    assert_eq!(specs.standard_coupon, 0.06);
}

#[test]
fn test_bond_future_specs_ust_5y_roundtrip() {
    let specs = BondFutureSpecs::ust_5y();
    let json = serde_json::to_string(&specs).expect("Serialization failed");
    let _deserialized: BondFutureSpecs =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(specs.tick_size, 1.0 / 128.0);
    assert_eq!(specs.standard_maturity_years, 5.0);
}

#[test]
fn test_bond_future_specs_ust_2y_roundtrip() {
    let specs = BondFutureSpecs::ust_2y();
    let json = serde_json::to_string(&specs).expect("Serialization failed");
    let _deserialized: BondFutureSpecs =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(specs.standard_maturity_years, 2.0);
}

#[test]
fn test_bond_future_specs_bund_roundtrip() {
    let specs = BondFutureSpecs::bund();
    let json = serde_json::to_string(&specs).expect("Serialization failed");
    let _deserialized: BondFutureSpecs =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(specs.tick_size, 0.01);
    assert_eq!(specs.standard_coupon, 0.06);
}

#[test]
fn test_bond_future_specs_gilt_roundtrip() {
    let specs = BondFutureSpecs::gilt();
    let json = serde_json::to_string(&specs).expect("Serialization failed");
    let _deserialized: BondFutureSpecs =
        serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(specs.standard_coupon, 0.04); // Gilt uses 4%, not 6%
}

#[test]
fn test_bond_future_specs_json_structure() {
    let specs = BondFutureSpecs::ust_10y();
    let json = serde_json::to_string_pretty(&specs).expect("Serialization failed");

    // Verify all required fields are present
    assert!(json.contains("contract_size"));
    assert!(json.contains("tick_size"));
    assert!(json.contains("tick_value"));
    assert!(json.contains("standard_coupon"));
    assert!(json.contains("standard_maturity_years"));
    assert!(json.contains("settlement_days"));
}

// ============================================================================
// BondFuture Serialization Tests
// ============================================================================

#[test]
fn test_bond_future_minimal_roundtrip() {
    let future = create_test_bond_future();

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(future.id, deserialized.id);
    assert_eq!(future.notional.amount(), deserialized.notional.amount());
    assert_eq!(future.notional.currency(), deserialized.notional.currency());
    assert_eq!(future.expiry, deserialized.expiry);
    assert_eq!(future.delivery_start, deserialized.delivery_start);
    assert_eq!(future.delivery_end, deserialized.delivery_end);
    assert_eq!(future.quoted_price, deserialized.quoted_price);
    assert_eq!(
        format!("{:?}", future.position),
        format!("{:?}", deserialized.position)
    );
    assert_eq!(future.ctd_bond_id, deserialized.ctd_bond_id);
    assert_eq!(future.discount_curve_id, deserialized.discount_curve_id);
}

#[test]
fn test_bond_future_with_basket_roundtrip() {
    let future = create_bond_future_with_basket();

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    // Verify basket was preserved
    assert_eq!(
        future.deliverable_basket.len(),
        deserialized.deliverable_basket.len()
    );
    for (original, parsed) in future
        .deliverable_basket
        .iter()
        .zip(deserialized.deliverable_basket.iter())
    {
        assert_eq!(original.bond_id, parsed.bond_id);
        assert_eq!(original.conversion_factor, parsed.conversion_factor);
    }

    // Verify position
    assert_eq!(
        format!("{:?}", future.position),
        format!("{:?}", deserialized.position)
    );
}

#[test]
fn test_bond_future_long_position_roundtrip() {
    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .expect("Valid bond future");

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(
        format!("{:?}", Position::Long),
        format!("{:?}", deserialized.position)
    );
}

#[test]
fn test_bond_future_short_position_roundtrip() {
    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Short)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .expect("Valid bond future");

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(
        format!("{:?}", Position::Short),
        format!("{:?}", deserialized.position)
    );
}

#[test]
fn test_bond_future_with_attributes_roundtrip() {
    let attrs = Attributes::new()
        .with_meta("exchange", "CBOT")
        .with_meta("contract_code", "TY")
        .with_meta("month", "H")
        .with_meta("year", "2025");

    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(attrs)
        .build()
        .expect("Valid bond future");

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(
        future.attributes.get_meta("exchange"),
        deserialized.attributes.get_meta("exchange")
    );
    assert_eq!(
        future.attributes.get_meta("contract_code"),
        deserialized.attributes.get_meta("contract_code")
    );
}

#[test]
fn test_bond_future_json_structure() {
    let future = create_test_bond_future();
    let json = serde_json::to_string_pretty(&future).expect("Serialization failed");

    // Verify all required fields are present
    assert!(json.contains("id"));
    assert!(json.contains("notional"));
    assert!(json.contains("expiry"));
    assert!(json.contains("delivery_start"));
    assert!(json.contains("delivery_end"));
    assert!(json.contains("quoted_price"));
    assert!(json.contains("position"));
    assert!(json.contains("contract_specs"));
    assert!(json.contains("deliverable_basket"));
    assert!(json.contains("ctd_bond_id"));
    assert!(json.contains("discount_curve_id"));
    assert!(json.contains("attributes"));
}

#[test]
fn test_bond_future_different_currencies() {
    let currencies = vec![
        (Currency::USD, "USD-TREASURY"),
        (Currency::EUR, "EUR-BUND"),
        (Currency::GBP, "GBP-GILT"),
    ];

    for (currency, curve_id) in currencies {
        let future = BondFuture::builder()
            .id(InstrumentId::new("TEST"))
            .notional(Money::new(1_000_000.0, currency))
            .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
            .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
            .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![create_test_deliverable_bond()])
            .ctd_bond_id(InstrumentId::new("US912828XG33"))
            .discount_curve_id(CurveId::new(curve_id))
            .attributes(Attributes::new())
            .build()
            .expect("Valid bond future");

        let json = serde_json::to_string(&future).expect("Serialization failed");
        let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

        assert_eq!(currency, deserialized.notional.currency());
        assert_eq!(future.discount_curve_id, deserialized.discount_curve_id);
    }
}

// ============================================================================
// deny_unknown_fields Tests
// ============================================================================

#[test]
fn test_bond_future_deny_unknown_fields() {
    // JSON with an unknown field "unknown_field"
    let json = r#"{
        "id": "TYH5",
        "notional": {
            "amount": 1000000.0,
            "currency": "USD"
        },
        "expiry": "2025-03-20",
        "delivery_start": "2025-03-21",
        "delivery_end": "2025-03-31",
        "quoted_price": 125.50,
        "position": "long",
        "contract_specs": {
            "contract_size": 100000.0,
            "tick_size": 0.015625,
            "tick_value": 15.625,
            "standard_coupon": 0.06,
            "standard_maturity_years": 10.0,
            "settlement_days": 2
        },
        "deliverable_basket": [
            {
                "bond_id": "US912828XG33",
                "conversion_factor": 0.8234
            }
        ],
        "ctd_bond_id": "US912828XG33",
        "discount_curve_id": "USD-TREASURY",
        "attributes": {
            "tags": [],
            "meta": {}
        },
        "unknown_field": true
    }"#;

    let result: Result<BondFuture, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Deserialization should fail with unknown field"
    );

    // Verify the error message mentions the unknown field
    let error = result.unwrap_err();
    let error_msg = error.to_string().to_lowercase();
    assert!(
        error_msg.contains("unknown") || error_msg.contains("field"),
        "Error should mention unknown field, got: {}",
        error
    );
}

#[test]
fn test_bond_future_specs_unknown_field_allowed() {
    // BondFutureSpecs does NOT have deny_unknown_fields,
    // so unknown fields should be silently ignored
    let json = r#"{
        "contract_size": 100000.0,
        "tick_size": 0.015625,
        "tick_value": 15.625,
        "standard_coupon": 0.06,
        "standard_maturity_years": 10.0,
        "settlement_days": 2,
        "extra_field": "ignored"
    }"#;

    let result: Result<BondFutureSpecs, _> = serde_json::from_str(json);
    // Should succeed (unknown fields ignored for BondFutureSpecs)
    assert!(
        result.is_ok(),
        "BondFutureSpecs should allow unknown fields"
    );
}

// ============================================================================
// Deserialization Default Tests
// ============================================================================

#[test]
fn test_bond_future_minimal_json() {
    // Test that BondFuture can be deserialized from minimal JSON
    // (this ensures defaults work for optional fields if any are added in the future)
    let json = r#"{
        "id": "TYH5",
        "notional": {
            "amount": 1000000.0,
            "currency": "USD"
        },
        "expiry": "2025-03-20",
        "delivery_start": "2025-03-21",
        "delivery_end": "2025-03-31",
        "quoted_price": 125.50,
        "position": "long",
        "contract_specs": {
            "contract_size": 100000.0,
            "tick_size": 0.015625,
            "tick_value": 15.625,
            "standard_coupon": 0.06,
            "standard_maturity_years": 10.0,
            "settlement_days": 2
        },
        "deliverable_basket": [
            {
                "bond_id": "US912828XG33",
                "conversion_factor": 0.8234
            }
        ],
        "ctd_bond_id": "US912828XG33",
        "discount_curve_id": "USD-TREASURY",
        "attributes": {
            "tags": [],
            "meta": {}
        }
    }"#;

    let result: Result<BondFuture, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Minimal JSON should deserialize correctly. Error: {:?}",
        result.as_ref().err()
    );

    let future = result.unwrap();
    assert_eq!(future.id.as_str(), "TYH5");
    assert_eq!(future.quoted_price, 125.50);
}

#[test]
fn test_bond_future_pretty_json() {
    // Test that pretty-printed JSON round-trips correctly
    let future = create_test_bond_future();

    let json = serde_json::to_string_pretty(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(future.id, deserialized.id);
    assert_eq!(future.quoted_price, deserialized.quoted_price);
}

#[test]
fn test_bond_future_compact_json() {
    // Test that compact JSON (no whitespace) works
    let future = create_test_bond_future();

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(future.id, deserialized.id);
    assert_eq!(future.quoted_price, deserialized.quoted_price);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_bond_future_large_notional() {
    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000_000.0, Currency::USD)) // $1 billion
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .expect("Valid bond future");

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(
        future.notional.amount(),
        deserialized.notional.amount(),
        "Large notional should round-trip correctly"
    );
}

#[test]
fn test_bond_future_fractional_price() {
    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.515625) // 125-16.5/32
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .expect("Valid bond future");

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(
        future.quoted_price, deserialized.quoted_price,
        "Fractional price should round-trip correctly"
    );
}

#[test]
fn test_bond_future_empty_attributes() {
    let future = BondFuture::builder()
        .id(InstrumentId::new("TYH5"))
        .notional(Money::new(1_000_000.0, Currency::USD))
        .expiry(Date::from_calendar_date(2025, Month::March, 20).unwrap())
        .delivery_start(Date::from_calendar_date(2025, Month::March, 21).unwrap())
        .delivery_end(Date::from_calendar_date(2025, Month::March, 31).unwrap())
        .quoted_price(125.50)
        .position(Position::Long)
        .contract_specs(BondFutureSpecs::default())
        .deliverable_basket(vec![create_test_deliverable_bond()])
        .ctd_bond_id(InstrumentId::new("US912828XG33"))
        .discount_curve_id(CurveId::new("USD-TREASURY"))
        .attributes(Attributes::new())
        .build()
        .expect("Valid bond future");

    let json = serde_json::to_string(&future).expect("Serialization failed");
    let deserialized: BondFuture = serde_json::from_str(&json).expect("Deserialization failed");

    assert!(
        deserialized.attributes.get_meta("any_key").is_none(),
        "Empty attributes should round-trip correctly"
    );
}
