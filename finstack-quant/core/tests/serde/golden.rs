//! Golden tests for serde stability.
//!
//! These tests ensure that our wire format remains stable across versions.
//! If any of these tests fail, it indicates a breaking change to the serialization format.

use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::types::{CurveId, InstrumentId};
use time::Month;

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).unwrap()
}

#[test]
fn test_currency_json_stable() {
    let ccy = Currency::EUR;
    let json = serde_json::to_string(&ccy).unwrap();
    assert_eq!(json, r#""EUR""#);

    let deserialized: Currency = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, ccy);
}

#[test]
fn test_date_json_stable() {
    let date = test_date();
    let json = serde_json::to_string(&date).unwrap();
    assert_eq!(json, r#""2025-01-15""#);

    let deserialized: Date = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, date);
}

#[test]
fn test_tenor_json_stable() {
    let tenor = Tenor::quarterly();
    let json = serde_json::to_string(&tenor).unwrap();
    assert_eq!(json, r#"{"count":3,"unit":"months"}"#);

    let deserialized: Tenor = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, tenor);
}

#[test]
fn test_daycount_json_stable() {
    let dc = DayCount::Act360;
    let json = serde_json::to_string(&dc).unwrap();
    assert_eq!(json, r#""Act360""#);

    let deserialized: DayCount = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, dc);
}

#[test]
fn test_business_day_convention_json_stable() {
    let bdc = BusinessDayConvention::ModifiedFollowing;
    let json = serde_json::to_string(&bdc).unwrap();
    assert_eq!(json, r#""modified_following""#);

    let deserialized: BusinessDayConvention = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, bdc);
}

#[test]
fn test_stub_kind_json_stable() {
    let stub = StubKind::ShortFront;
    let json = serde_json::to_string(&stub).unwrap();
    assert_eq!(json, r#""ShortFront""#);

    let deserialized: StubKind = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, stub);
}

#[test]
fn test_cfkind_json_stable() {
    let kind = CFKind::Fixed;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, r#""Fixed""#);

    let deserialized: CFKind = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, kind);
}

#[test]
fn test_instrument_id_json_stable() {
    let id = InstrumentId::new("BOND_5Y_USD");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, r#""BOND_5Y_USD""#);

    let deserialized: InstrumentId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn test_curve_id_json_stable() {
    let id = CurveId::new("USD_SOFR_CURVE");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, r#""USD_SOFR_CURVE""#);

    let deserialized: CurveId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn test_roundtrip_all_currencies() {
    let currencies = vec![
        Currency::USD,
        Currency::EUR,
        Currency::GBP,
        Currency::JPY,
        Currency::CHF,
        Currency::AUD,
        Currency::CAD,
        Currency::CNY,
    ];

    for ccy in currencies {
        let json = serde_json::to_string(&ccy).unwrap();
        let deserialized: Currency = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ccy, "Currency {} failed roundtrip", ccy);
    }
}

#[test]
fn test_roundtrip_all_daycounts() {
    let daycounts = vec![
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::Act365L,
        DayCount::Thirty360,
        DayCount::ThirtyE360,
        DayCount::ActAct,
        DayCount::ActActIsma,
        DayCount::Bus252,
    ];

    for dc in daycounts {
        let json = serde_json::to_string(&dc).unwrap();
        let deserialized: DayCount = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, dc, "DayCount {:?} failed roundtrip", dc);
    }
}

#[test]
fn test_roundtrip_all_business_day_conventions() {
    let conventions = vec![
        BusinessDayConvention::Unadjusted,
        BusinessDayConvention::Following,
        BusinessDayConvention::ModifiedFollowing,
        BusinessDayConvention::Preceding,
        BusinessDayConvention::ModifiedPreceding,
    ];

    for bdc in conventions {
        let json = serde_json::to_string(&bdc).unwrap();
        let deserialized: BusinessDayConvention = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized, bdc,
            "BusinessDayConvention {:?} failed roundtrip",
            bdc
        );
    }
}

#[test]
fn test_roundtrip_all_cfkinds() {
    let kinds = vec![
        CFKind::Fixed,
        CFKind::FloatReset,
        CFKind::Notional,
        CFKind::PIK,
        CFKind::Amortization,
        CFKind::Fee,
        CFKind::Stub,
    ];

    for kind in kinds {
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: CFKind = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kind, "CFKind {:?} failed roundtrip", kind);
    }
}

/// Test that the JSON format is stable and documented.
/// This serves as both a regression test and documentation.
#[test]
fn test_wire_format_documentation() {
    // Primitive types use simple string representation
    assert_eq!(serde_json::to_string(&Currency::USD).unwrap(), r#""USD""#);
    assert_eq!(
        serde_json::to_string(&DayCount::Act360).unwrap(),
        r#""Act360""#
    );
    assert_eq!(serde_json::to_string(&CFKind::Fixed).unwrap(), r#""Fixed""#);

    // Date uses ISO 8601 format
    assert_eq!(
        serde_json::to_string(&test_date()).unwrap(),
        r#""2025-01-15""#
    );

    // Tenor uses struct format
    assert_eq!(
        serde_json::to_string(&Tenor::quarterly()).unwrap(),
        r#"{"count":3,"unit":"months"}"#
    );
    assert_eq!(
        serde_json::to_string(&Tenor::monthly()).unwrap(),
        r#"{"count":1,"unit":"months"}"#
    );
    assert_eq!(
        serde_json::to_string(&Tenor::annual()).unwrap(),
        r#"{"count":1,"unit":"years"}"#
    );

    // IDs use simple string representation
    assert_eq!(
        serde_json::to_string(&InstrumentId::new("BOND")).unwrap(),
        r#""BOND""#
    );
    assert_eq!(
        serde_json::to_string(&CurveId::new("CURVE")).unwrap(),
        r#""CURVE""#
    );
}
