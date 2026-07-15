//! Tests for NDF types and builders.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date};
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations::instruments::fx::ndf::{Ndf, NdfFixingSource, NdfQuoteConvention};
use finstack_quant_valuations::instruments::{Attributes, Instrument};
use finstack_quant_valuations::pricer::InstrumentType;
use time::Month;

#[test]
fn test_ndf_builder() {
    let ndf = Ndf::builder()
        .id(InstrumentId::new("TEST-NDF"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
        .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    assert_eq!(ndf.id.as_str(), "TEST-NDF");
    assert_eq!(ndf.base_currency, Currency::CNY);
    assert_eq!(ndf.settlement_currency, Currency::USD);
    assert_eq!(ndf.contract_rate, 7.25);
    assert!(!ndf.is_fixed());
}

#[test]
fn test_ndf_builder_with_optional_fields() {
    let ndf = Ndf::builder()
        .id(InstrumentId::new("TEST-NDF-FULL"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
        .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .fixing_rate_opt(Some(7.30))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .fixing_source_enum_opt(Some(NdfFixingSource::Cnhfix))
        .spot_rate_override_opt(Some(7.25))
        .base_calendar_id_opt(Some("CNY".to_string()))
        .quote_calendar_id_opt(Some("USD".to_string()))
        .attributes(Attributes::new().with_tag("ndf"))
        .build()
        .expect("should build");

    assert_eq!(ndf.fixing_rate, Some(7.30));
    assert_eq!(ndf.fixing_source_enum, Some(NdfFixingSource::Cnhfix));
    assert!(ndf.foreign_discount_curve_id.is_some());
    assert!(ndf.is_fixed());
    assert!(ndf.attributes.has_tag("ndf"));
}

#[test]
fn test_ndf_example() {
    let ndf = Ndf::example();

    assert_eq!(ndf.id.as_str(), "USDCNY-NDF-3M");
    assert_eq!(ndf.base_currency, Currency::CNY);
    assert_eq!(ndf.settlement_currency, Currency::USD);
    assert!(ndf.attributes.has_tag("ndf"));
    assert_eq!(ndf.attributes.get_meta("pair"), Some("USDCNY"));
}

#[test]
fn test_ndf_from_trade_date() {
    let trade_date = Date::from_calendar_date(2024, Month::January, 15).expect("valid date");

    let ndf = Ndf::from_trade_date(
        "USDCNY-3M",
        Currency::CNY,
        Currency::USD,
        trade_date,
        90, // 3 month tenor
        Money::new(10_000_000.0, Currency::CNY),
        7.25,
        "USD-OIS",
        None,
        None,
        2, // T+2 spot
        2, // T-2 fixing before maturity
        BusinessDayConvention::ModifiedFollowing,
    )
    .expect("should build");

    // Check that fixing date is before maturity
    assert!(ndf.fixing_date < ndf.maturity);

    // Check that maturity is roughly 3 months from spot
    let spot_date = trade_date + time::Duration::days(2);
    let expected_maturity = spot_date + time::Duration::days(90);
    let days_diff = (ndf.maturity - expected_maturity).whole_days().abs();
    assert!(days_diff <= 5, "Maturity should be close to expected");
}

#[test]
fn test_ndf_with_fixing_rate() {
    let ndf = Ndf::example();
    assert!(!ndf.is_fixed());

    let fixed_ndf = ndf.with_fixing_rate(7.30).expect("valid rate");
    assert!(fixed_ndf.is_fixed());
    assert_eq!(fixed_ndf.fixing_rate, Some(7.30));
}

#[test]
fn test_ndf_instrument_trait() {
    let ndf = Ndf::example();

    assert_eq!(ndf.id(), "USDCNY-NDF-3M");
    assert_eq!(ndf.key(), InstrumentType::Ndf);
    assert!(ndf.attributes().has_tag("ndf"));
}

#[test]
fn test_ndf_curve_dependencies_single() {
    let ndf = Ndf::example();
    let deps = ndf
        .market_dependencies()
        .expect("market_dependencies")
        .curves;

    // Without foreign curve, only settlement curve
    assert_eq!(deps.discount_curves.len(), 1);
    assert!(deps.discount_curves.iter().any(|c| c.as_str() == "USD-OIS"));
}

#[test]
fn test_ndf_curve_dependencies_with_foreign() {
    let ndf = Ndf::builder()
        .id(InstrumentId::new("TEST"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
        .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let deps = ndf
        .market_dependencies()
        .expect("market_dependencies")
        .curves;

    assert_eq!(deps.discount_curves.len(), 2);
}

#[test]
fn test_ndf_required_discount_curves() {
    let ndf = Ndf::example();
    let curves = ndf
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves;
    assert_eq!(curves.len(), 1);

    let ndf_with_foreign = Ndf::builder()
        .id(InstrumentId::new("TEST"))
        .base_currency(Currency::CNY)
        .settlement_currency(Currency::USD)
        .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
        .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
        .notional(Money::new(10_000_000.0, Currency::CNY))
        .contract_rate(7.25)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .foreign_discount_curve_id_opt(Some(CurveId::new("CNY-OIS")))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    let curves = ndf_with_foreign
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves;
    assert_eq!(curves.len(), 2);
}

#[test]
fn test_ndf_clone() {
    let ndf = Ndf::example();
    let cloned = ndf.clone();

    assert_eq!(ndf.id.as_str(), cloned.id.as_str());
    assert_eq!(ndf.base_currency, cloned.base_currency);
    assert_eq!(ndf.contract_rate, cloned.contract_rate);
}

#[test]
fn test_ndf_common_currencies() {
    // Test with INR (Indian Rupee)
    let ndf_inr = Ndf::builder()
        .id(InstrumentId::new("USDINR-NDF"))
        .base_currency(Currency::INR)
        .settlement_currency(Currency::USD)
        .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
        .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
        .notional(Money::new(100_000_000.0, Currency::INR))
        .contract_rate(83.0)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .fixing_source_enum_opt(Some(NdfFixingSource::Rbi))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    assert_eq!(ndf_inr.base_currency, Currency::INR);
    assert_eq!(ndf_inr.fixing_source_enum, Some(NdfFixingSource::Rbi));

    // Test with BRL (Brazilian Real)
    let ndf_brl = Ndf::builder()
        .id(InstrumentId::new("USDBRL-NDF"))
        .base_currency(Currency::BRL)
        .settlement_currency(Currency::USD)
        .fixing_date(Date::from_calendar_date(2025, Month::March, 13).expect("valid date"))
        .maturity(Date::from_calendar_date(2025, Month::March, 15).expect("valid date"))
        .notional(Money::new(10_000_000.0, Currency::BRL))
        .contract_rate(5.0)
        .domestic_discount_curve_id(CurveId::new("USD-OIS"))
        .quote_convention(NdfQuoteConvention::BasePerSettlement)
        .fixing_source_enum_opt(Some(NdfFixingSource::Ptax))
        .attributes(Attributes::new())
        .build()
        .expect("should build");

    assert_eq!(ndf_brl.base_currency, Currency::BRL);
    assert_eq!(ndf_brl.fixing_source_enum, Some(NdfFixingSource::Ptax));
}
