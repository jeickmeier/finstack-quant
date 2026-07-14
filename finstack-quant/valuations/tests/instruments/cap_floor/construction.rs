//! Construction tests for interest rate options.
//!
//! Validates instrument creation, parameter handling, and builder patterns.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::cap_floor::{
    CapFloor, OvernightCouponConvention, OvernightSpreadCompounding, RateOptionType,
};
use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_valuations::instruments::{ExerciseStyle, Instrument, SettlementType};
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_cap_creation_basic() {
    let notional = Money::new(10_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let cap = CapFloor::new_cap(
        "USD_CAP_3%",
        notional,
        0.03,
        start,
        end,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
        "USD-LIBOR-3M",
        "USD-CAP-VOL",
    )
    .expect("valid strike");

    assert_eq!(cap.id, "USD_CAP_3%");
    assert_eq!(cap.rate_option_type, RateOptionType::Cap);
    assert_eq!(cap.notional.amount(), 10_000_000.0);
    assert_eq!(cap.notional.currency(), Currency::USD);
    assert_eq!(cap.strike, Decimal::try_from(0.03).expect("valid decimal"));
    assert_eq!(cap.frequency, Tenor::quarterly());
    assert_eq!(cap.day_count, DayCount::Act360);
    assert_eq!(cap.start_date, start);
    assert_eq!(cap.maturity, end);
}

#[test]
fn test_floor_creation_basic() {
    let notional = Money::new(5_000_000.0, Currency::EUR);
    let start = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let end = Date::from_calendar_date(2028, Month::March, 15).unwrap();

    let floor = CapFloor::new_floor(
        "EUR_FLOOR_1%",
        notional,
        0.01,
        start,
        end,
        Tenor::semi_annual(),
        DayCount::Thirty360,
        "EUR-OIS",
        "EUR-EURIBOR-6M",
        "EUR-CAP-VOL",
    )
    .expect("valid strike");

    assert_eq!(floor.id, "EUR_FLOOR_1%");
    assert_eq!(floor.rate_option_type, RateOptionType::Floor);
    assert_eq!(floor.notional.amount(), 5_000_000.0);
    assert_eq!(floor.notional.currency(), Currency::EUR);
    assert_eq!(
        floor.strike,
        Decimal::try_from(0.01).expect("valid decimal")
    );
    assert_eq!(floor.frequency, Tenor::semi_annual());
}

#[test]
fn test_cap_new_cap_helper() {
    let notional = Money::new(1_000_000.0, Currency::GBP);
    let start = Date::from_calendar_date(2025, Month::June, 1).unwrap();
    let end = Date::from_calendar_date(2027, Month::June, 1).unwrap();

    let cap = CapFloor::new_cap(
        "GBP_CAP",
        notional,
        0.04,
        start,
        end,
        Tenor::quarterly(),
        DayCount::Act365F,
        "GBP-OIS",
        "GBP-LIBOR-3M",
        "GBP-CAP-VOL",
    )
    .expect("valid strike");

    assert_eq!(cap.rate_option_type, RateOptionType::Cap);
    assert_eq!(cap.strike, Decimal::try_from(0.04).expect("valid decimal"));
    assert_eq!(cap.notional.currency(), Currency::GBP);
}

#[test]
fn test_floor_new_floor_helper() {
    let notional = Money::new(2_000_000.0, Currency::JPY);
    let start = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2031, Month::January, 1).unwrap();

    let floor = CapFloor::new_floor(
        "JPY_FLOOR",
        notional,
        0.005,
        start,
        end,
        Tenor::quarterly(),
        DayCount::Act360,
        "JPY-OIS",
        "JPY-LIBOR-3M",
        "JPY-CAP-VOL",
    )
    .expect("valid strike");

    assert_eq!(floor.rate_option_type, RateOptionType::Floor);
    assert_eq!(
        floor.strike,
        Decimal::try_from(0.005).expect("valid decimal")
    );
}

#[test]
fn test_caplet_creation() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2025, Month::April, 1).unwrap();

    let caplet = CapFloor {
        id: "CAPLET_TEST".into(),
        rate_option_type: RateOptionType::Caplet,
        notional,
        strike: Decimal::try_from(0.05).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    assert_eq!(caplet.rate_option_type, RateOptionType::Caplet);
    assert!(end > start);
}

#[test]
fn test_floorlet_creation() {
    let notional = Money::new(500_000.0, Currency::EUR);
    let start = Date::from_calendar_date(2025, Month::March, 1).unwrap();
    let end = Date::from_calendar_date(2025, Month::September, 1).unwrap();

    let floorlet = CapFloor {
        id: "FLOORLET_TEST".into(),
        rate_option_type: RateOptionType::Floorlet,
        notional,
        strike: Decimal::try_from(0.02).expect("valid decimal"),
        start_date: start,
        maturity: end,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        stub: StubKind::None,
        bdc: BusinessDayConvention::Following,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "EUR_OIS".into(),
        forward_curve_id: "EUR_EURIBOR_6M".into(),
        vol_surface_id: "EUR_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        spread: Decimal::ZERO,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    assert_eq!(floorlet.rate_option_type, RateOptionType::Floorlet);
}

#[test]
fn test_custom_calendar() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let mut cap = CapFloor::new_cap(
        "CAP_WITH_CALENDAR",
        notional,
        0.03,
        start,
        end,
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
        "USD-LIBOR-3M",
        "USD-CAP-VOL",
    )
    .expect("valid strike");
    cap.calendar_id = Some("US_NERC".into());

    assert_eq!(cap.calendar_id.as_deref(), Some("US_NERC"));
}

#[test]
fn test_different_day_counts() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let day_counts = vec![
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::Thirty360,
        DayCount::ActActIsma,
    ];

    for dc in day_counts {
        let cap = CapFloor::new_cap(
            "CAP_DC_TEST",
            notional,
            0.03,
            start,
            end,
            Tenor::quarterly(),
            dc,
            "USD-OIS",
            "USD-LIBOR-3M",
            "USD-CAP-VOL",
        )
        .expect("valid strike");

        assert_eq!(cap.day_count, dc);
    }
}

#[test]
fn test_different_frequencies() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let frequencies = vec![
        Tenor::monthly(),
        Tenor::quarterly(),
        Tenor::semi_annual(),
        Tenor::annual(),
    ];

    for freq in frequencies {
        let cap = CapFloor::new_cap(
            "CAP_FREQ_TEST",
            notional,
            0.03,
            start,
            end,
            freq,
            DayCount::Act360,
            "USD-OIS",
            "USD-LIBOR-3M",
            "USD-CAP-VOL",
        )
        .expect("valid strike");

        assert_eq!(cap.frequency, freq);
    }
}

#[test]
fn test_new_caplet_rejects_nan_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new_caplet(
        "CAPLET-NAN",
        notional,
        f64::NAN,
        start,
        end,
        DayCount::Act360,
        "USD-OIS",
        "USD-SOFR-3M",
        "USD_CAP_VOL",
    );
    assert!(result.is_err(), "new_caplet should reject NaN strike");
}

#[test]
fn test_new_caplet_rejects_infinite_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new_caplet(
        "CAPLET-INF",
        notional,
        f64::INFINITY,
        start,
        end,
        DayCount::Act360,
        "USD-OIS",
        "USD-SOFR-3M",
        "USD_CAP_VOL",
    );
    assert!(result.is_err(), "new_caplet should reject infinite strike");
}

#[test]
fn test_new_floorlet_rejects_nan_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new_floorlet(
        "FLOORLET-NAN",
        notional,
        f64::NAN,
        start,
        end,
        DayCount::Act360,
        "USD-OIS",
        "USD-SOFR-3M",
        "USD_CAP_VOL",
    );
    assert!(result.is_err(), "new_floorlet should reject NaN strike");
}

#[test]
fn test_new_caplet_accepts_valid_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new_caplet(
        "CAPLET-OK",
        notional,
        0.05,
        start,
        end,
        DayCount::Act360,
        "USD-OIS",
        "USD-SOFR-3M",
        "USD_CAP_VOL",
    );
    assert!(
        result.is_ok(),
        "new_caplet should accept a valid finite strike"
    );
}

#[test]
fn term_index_rejects_overnight_coupon_settings() {
    let mut caplet = CapFloor::new_caplet(
        "TERM-WITH-OVERNIGHT-SETTINGS",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2025, Month::January, 2).unwrap(),
        Date::from_calendar_date(2025, Month::April, 2).unwrap(),
        DayCount::Act360,
        "USD-OIS",
        "USD-LIBOR-3M",
        "USD-CAP-VOL",
    )
    .expect("valid caplet");
    caplet.overnight_coupon = Some(OvernightCouponConvention {
        compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
        payment_delay_days: 2,
        fixing_calendar_id: Some("usny".into()),
        payment_calendar_id: Some("usny".into()),
        spread_compounding: OvernightSpreadCompounding::Exclude,
    });

    let error = caplet
        .validate_for_pricing()
        .expect_err("term index must reject overnight-only coupon settings");
    assert!(
        error.to_string().contains("overnight"),
        "validation should identify the incompatible overnight settings: {error}"
    );
}

#[test]
fn legacy_cap_json_defaults_overnight_coupon_to_none() {
    let cap = CapFloor::new_cap(
        "LEGACY-CAP",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        Date::from_calendar_date(2025, Month::January, 2).unwrap(),
        Date::from_calendar_date(2026, Month::January, 2).unwrap(),
        Tenor::quarterly(),
        DayCount::Act360,
        "USD-OIS",
        "USD-LIBOR-3M",
        "USD-CAP-VOL",
    )
    .expect("valid cap");
    let mut value = serde_json::to_value(cap).expect("serialize cap");
    assert!(
        value.get("overnight_coupon").is_none(),
        "default overnight terms should be omitted for backward-compatible JSON"
    );
    assert!(
        value.get("spread").is_none(),
        "zero spread should be omitted for backward-compatible JSON"
    );
    value
        .as_object_mut()
        .expect("cap JSON object")
        .remove("spread");

    let decoded: CapFloor = serde_json::from_value(value).expect("deserialize legacy cap JSON");
    assert!(decoded.overnight_coupon.is_none());
    assert_eq!(decoded.spread, Decimal::ZERO);
}
