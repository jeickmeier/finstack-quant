//! Roundtrip serialization tests for various types.
//!
//! These tests verify that types serialize and deserialize correctly,
//! and that the serialized form can be used to reconstruct working objects.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{
    BusinessDayConvention, CalendarRegistry, DayCount, DayCountContextState, ScheduleBuilder,
    ScheduleSpec, StubKind, Tenor, TenorUnit,
};
use finstack_quant_core::explain::ExplainOpts;
use finstack_quant_core::{Error, InputError};
use time::{Date, Month};

#[test]
fn error_serialization_roundtrip() {
    let input = Error::Input(InputError::NotFound {
        id: "curve:USD-OIS".to_string(),
    });
    let currency_mismatch = Error::CurrencyMismatch {
        expected: Currency::USD,
        actual: Currency::EUR,
    };

    let json_input = serde_json::to_string(&input).unwrap();
    let json_currency = serde_json::to_string(&currency_mismatch).unwrap();

    let roundtrip_input: Error = serde_json::from_str(&json_input).unwrap();
    let roundtrip_currency: Error = serde_json::from_str(&json_currency).unwrap();

    assert!(matches!(
        roundtrip_input,
        Error::Input(InputError::NotFound { .. })
    ));
    if let Error::CurrencyMismatch { expected, actual } = roundtrip_currency {
        assert_eq!(expected, Currency::USD);
        assert_eq!(actual, Currency::EUR);
    } else {
        panic!("expected currency mismatch variant");
    }
}

#[test]
fn explain_opts_roundtrip() {
    let opts = ExplainOpts {
        enabled: true,
        max_entries: Some(128),
    };
    let json = serde_json::to_string(&opts).unwrap();
    let restored: ExplainOpts = serde_json::from_str(&json).unwrap();
    assert!(restored.enabled);
    assert_eq!(restored.max_entries, Some(128));
}

#[test]
fn daycount_ctx_state_roundtrip() {
    let state = DayCountContextState {
        calendar_id: Some("target2".to_string()),
        frequency: Some(Tenor::quarterly()),
        bus_basis: Some(260),
        coupon_period: None,
    };
    let registry = CalendarRegistry::global();
    let ctx = state.to_ctx(registry);
    let start = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let end = Date::from_calendar_date(2025, Month::February, 2).unwrap();
    let yf = DayCount::Bus252.year_fraction(start, end, ctx).unwrap();
    assert!(yf > 0.0);

    let roundtrip_state: DayCountContextState = ctx.into();
    assert_eq!(roundtrip_state.calendar_id.as_deref(), Some("target2"));
    assert_eq!(roundtrip_state.frequency, Some(Tenor::quarterly()));
    assert_eq!(roundtrip_state.bus_basis, Some(260));
}

#[test]
fn schedule_spec_builds_expected_dates() {
    let start = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let end = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let spec = ScheduleSpec {
        start,
        end,
        frequency: Tenor::new(1, TenorUnit::Months),
        stub: StubKind::None,
        business_day_convention: Some(BusinessDayConvention::Following),
        calendar_id: Some("target2".to_string()),
        end_of_month: false,
        imm_mode: false,
        cds_imm_mode: false,
        graceful: false,
        allow_missing_calendar: false,
    };

    let json = serde_json::to_string(&spec).unwrap();
    let restored: ScheduleSpec = serde_json::from_str(&json).unwrap();
    let schedule = restored.build().unwrap();
    assert_eq!(schedule.dates.len(), 7);

    // Cross-check with builder directly
    let builder_schedule = ScheduleBuilder::new(start, end)
        .unwrap()
        .frequency(Tenor::new(1, TenorUnit::Months))
        .adjust_with_id(BusinessDayConvention::Following, "target2")
        .build()
        .unwrap();

    assert_eq!(schedule.dates, builder_schedule.dates);
}
