//! Tests for FX Forward types and builders.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, Tenor};
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
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
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
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
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
        Tenor::parse("3M").expect("valid tenor"),
        Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        "USD-OIS",
        "EUR-OIS",
        None,
        None,
        2, // T+2 spot
        BusinessDayConvention::ModifiedFollowing,
        false,
    )
    .expect("should build");

    assert_eq!(
        forward.maturity,
        Date::from_calendar_date(2024, Month::April, 17).expect("valid date")
    );
}

#[test]
fn standard_forward_tenor_preserves_explicit_end_of_month_policy() {
    let forward = FxForward::from_trade_date(
        "EURUSD-1M-EOM",
        Currency::EUR,
        Currency::USD,
        Date::from_calendar_date(2024, Month::January, 29).expect("valid date"),
        Tenor::parse("1M").expect("valid tenor"),
        Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"),
        "USD-OIS",
        "EUR-OIS",
        None,
        None,
        2,
        BusinessDayConvention::Unadjusted,
        true,
    )
    .expect("should build");

    assert_eq!(
        forward.maturity,
        Date::from_calendar_date(2024, Month::February, 29).expect("valid date")
    );
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
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build")
        .with_forward_points(1.10, 0.0100)
        .expect("valid forward points"); // 100 pips

    assert_eq!(forward.spot_rate_override, None);
    assert!((forward.contract_rate.unwrap() - 1.11).abs() < 1e-10);
}

#[test]
fn test_fx_forward_with_forward_pips() {
    let forward = FxForward::builder()
        .id(InstrumentId::new("PIPS-TEST"))
        .base_currency(Currency::EUR)
        .quote_currency(Currency::USD)
        .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
        .notional(Money::new(1_000_000.0, Currency::EUR).expect("valid money fixture"))
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id(CurveId::new("EUR-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build")
        .with_forward_pips(1.10, 50.0)
        .expect("valid forward pips");

    assert_eq!(forward.spot_rate_override, None);
    assert!((forward.contract_rate.unwrap() - 1.1050).abs() < 1e-10);
}

#[test]
fn test_fx_forward_with_forward_pips_jpy_pair() {
    let forward = FxForward::builder()
        .id(InstrumentId::new("PIPS-JPY"))
        .base_currency(Currency::USD)
        .quote_currency(Currency::JPY)
        .maturity(Date::from_calendar_date(2025, Month::June, 15).expect("valid date"))
        .notional(Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"))
        .domestic_discount_curve_id(CurveId::new("JPY-OIS"))
        .foreign_discount_curve_id(CurveId::new("USD-OIS"))
        .attributes(Attributes::new())
        .build()
        .expect("should build")
        .with_forward_pips(150.00, 50.0)
        .expect("valid JPY pips");

    assert_eq!(forward.spot_rate_override, None);
    assert!((forward.contract_rate.unwrap() - 150.50).abs() < 1e-10);
}

#[test]
fn test_fx_forward_clone() {
    let forward = FxForward::example().unwrap();
    let cloned = forward.clone();

    assert_eq!(forward.id.as_str(), cloned.id.as_str());
    assert_eq!(forward.base_currency, cloned.base_currency);
    assert_eq!(forward.contract_rate, cloned.contract_rate);
}
