//! Tests for FX Forward types and builders.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::fx_forward::FxForward;
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::InstrumentType;
use time::Month;

#[test]
fn test_fx_forward_builder() {
    let forward = FxForward::builder()
        .id(InstrumentId::new("TEST-FWD"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    assert_eq!(forward.id.as_str(), "TEST-FWD");
    assert_eq!(forward.base_currency, Currency::EUR);
    assert_eq!(forward.quote_currency, Currency::USD);
    assert_eq!(forward.notional.amount(), 1_000_000.0);
    assert!(forward.contract_rate.is_none());
}

#[test]
fn test_fx_forward_builder_with_optional_fields() {
    let forward = FxForward::builder()
        .id(InstrumentId::new("TEST-FWD-FULL"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .contract_rate_opt(Some(1.12))
        .spot_rate_override_opt(Some(1.10))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .base_calendar_id_opt(Some("EUR".to_string()))
        .quote_calendar_id_opt(Some("USD".to_string()))
        .attributes(Attributes::new().with_tag("test"))
        .build()
        .expect("should build");

    assert_eq!(forward.contract_rate, Some(1.12));
    assert_eq!(forward.spot_rate_override, Some(1.10));
    assert_eq!(forward.base_calendar_id, Some("EUR".to_string()));
    assert_eq!(forward.quote_calendar_id, Some("USD".to_string()));
    assert!(forward.attributes.has_tag("test"));
}

#[test]
fn test_fx_forward_example() {
    let forward = FxForward::example().unwrap();

    assert_eq!(forward.id.as_str(), "EURUSD-FWD-6M");
    assert_eq!(forward.base_currency, Currency::EUR);
    assert_eq!(forward.quote_currency, Currency::USD);
    assert!(forward.attributes.has_tag("fx"));
    assert_eq!(forward.attributes.get_meta("pair"), Some("EURUSD"));
}

#[test]
fn test_fx_forward_from_trade_date() {
    let trade_date = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");

    let forward = FxForward::from_trade_date(
        "EURUSD-3M",
        Currency::EUR,
        Currency::USD,
        trade_date,
        90, // 3 month tenor
        Money::new(1_000_000.0, Currency::EUR),
        "USD-OIS",
        "EUR-OIS",
        None,
        None,
        2, // T+2 spot
        BusinessDayConvention::ModifiedFollowing,
    )
    .expect("should build");

    // Check that maturity is roughly 3 months from spot
    let spot_date = trade_date + time::Duration::days(2);
    let expected_maturity = spot_date + time::Duration::days(90);

    assert!(forward.maturity >= spot_date);
    // Allow some business day adjustment tolerance
    let days_diff = (forward.maturity - expected_maturity).whole_days().abs();
    assert!(days_diff <= 5, "Maturity should be close to expected");
}

#[test]
fn test_fx_forward_instrument_trait() {
    let forward = FxForward::example().unwrap();

    assert_eq!(forward.id(), "EURUSD-FWD-6M");
    assert_eq!(forward.key(), InstrumentType::FxForward);
    assert!(forward.attributes().has_tag("fx"));
}

#[test]
fn test_fx_forward_market_dependencies() {
    let forward = FxForward::example().unwrap();
    let deps = forward
        .market_dependencies()
        .expect("market_dependencies")
        .curves;

    assert_eq!(deps.discount_curves.len(), 2);
    assert!(deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"));
    assert!(deps.discount_curves.iter().any(|c| c.as_str() == "EUR-OIS"));
}

#[test]
fn test_fx_forward_required_discount_curves() {
    let forward = FxForward::example().unwrap();
    let curves = forward
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves;

    assert_eq!(curves.len(), 2);
}

#[test]
fn test_fx_forward_with_forward_points_builder() {
    let forward = FxForward::builder()
        .id(InstrumentId::new("POINTS-TEST"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
        .notional(Money::new(1_000_000.0, Currency::EUR))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build")
        .with_forward_points(1.10, 0.0100)
        .expect("valid forward points"); // 100 pips

    assert_eq!(forward.spot_rate_override, Some(1.10));
    assert!((forward.contract_rate.unwrap() - 1.11).abs() < 1e-10);
}

#[test]
fn test_fx_forward_clone() {
    let forward = FxForward::example().unwrap();
    let cloned = forward.clone();

    assert_eq!(forward.id.as_str(), cloned.id.as_str());
    assert_eq!(forward.base_currency, cloned.base_currency);
    assert_eq!(forward.contract_rate, cloned.contract_rate);
}
