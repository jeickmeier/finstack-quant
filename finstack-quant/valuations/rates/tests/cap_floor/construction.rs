//! Construction tests for interest rate options.
//!
//! Validates instrument creation, parameter handling, and builder patterns.

use finstack_quant_cashflows::builder::FloatingLegCompounding;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::money::Money;
use finstack_quant_models::types::ExerciseStyle;
use finstack_quant_valuations_core::instruments::{Instrument, SettlementType};
use finstack_quant_valuations_rates::cap_floor::{
    CapFloor, OvernightCouponConvention, OvernightSpreadCompounding, RateOptionType,
};
use rust_decimal::Decimal;
use time::Month;

#[test]
fn test_cap_creation_basic() {
    let notional = Money::new(10_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let cap = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "USD_CAP_3%".into(),
        rate_option_type: RateOptionType::Cap,
        notional,
        strike: 0.03,
        start_date: start,
        maturity: end,
        frequency: Some(Tenor::quarterly()),
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-LIBOR-3M".into(),
        vol_surface_id: "USD-CAP-VOL".into(),
    })
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
    let notional = Money::new(5_000_000.0, Currency::EUR).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let end = Date::from_calendar_date(2028, Month::March, 15).unwrap();

    let floor = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "EUR_FLOOR_1%".into(),
        rate_option_type: RateOptionType::Floor,
        notional,
        strike: 0.01,
        start_date: start,
        maturity: end,
        frequency: Some(Tenor::semi_annual()),
        day_count: DayCount::Thirty360,
        discount_curve_id: "EUR-OIS".into(),
        forward_curve_id: "EUR-EURIBOR-6M".into(),
        vol_surface_id: "EUR-CAP-VOL".into(),
    })
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
    let notional = Money::new(1_000_000.0, Currency::GBP).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::June, 1).unwrap();
    let end = Date::from_calendar_date(2027, Month::June, 1).unwrap();

    let cap = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "GBP_CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional,
        strike: 0.04,
        start_date: start,
        maturity: end,
        frequency: Some(Tenor::quarterly()),
        day_count: DayCount::Act365F,
        discount_curve_id: "GBP-OIS".into(),
        forward_curve_id: "GBP-LIBOR-3M".into(),
        vol_surface_id: "GBP-CAP-VOL".into(),
    })
    .expect("valid strike");

    assert_eq!(cap.rate_option_type, RateOptionType::Cap);
    assert_eq!(cap.strike, Decimal::try_from(0.04).expect("valid decimal"));
    assert_eq!(cap.notional.currency(), Currency::GBP);
}

#[test]
fn test_floor_new_floor_helper() {
    let notional = Money::new(2_000_000.0, Currency::JPY).expect("valid money fixture");
    let start = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2031, Month::January, 1).unwrap();

    let floor = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "JPY_FLOOR".into(),
        rate_option_type: RateOptionType::Floor,
        notional,
        strike: 0.005,
        start_date: start,
        maturity: end,
        frequency: Some(Tenor::quarterly()),
        day_count: DayCount::Act360,
        discount_curve_id: "JPY-OIS".into(),
        forward_curve_id: "JPY-LIBOR-3M".into(),
        vol_surface_id: "JPY-CAP-VOL".into(),
    })
    .expect("valid strike");

    assert_eq!(floor.rate_option_type, RateOptionType::Floor);
    assert_eq!(
        floor.strike,
        Decimal::try_from(0.005).expect("valid decimal")
    );
}

#[test]
fn test_caplet_creation() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
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
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    assert_eq!(caplet.rate_option_type, RateOptionType::Caplet);
    assert!(end > start);
}

#[test]
fn test_floorlet_creation() {
    let notional = Money::new(500_000.0, Currency::EUR).expect("valid money fixture");
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
        business_day_convention: BusinessDayConvention::Following,
        calendar_id: None,
        exercise_style: ExerciseStyle::European,
        settlement: SettlementType::Cash,
        discount_curve_id: "EUR_OIS".into(),
        forward_curve_id: "EUR_EURIBOR_6M".into(),
        vol_surface_id: "EUR_CAP_VOL".into(),
        vol_type: Default::default(),
        vol_shift: 0.0,
        overnight_coupon: None,
        premium: None,
        spread_bp: Decimal::ZERO,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Default::default(),
    };

    assert_eq!(floorlet.rate_option_type, RateOptionType::Floorlet);
}

#[test]
fn test_custom_calendar() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let mut cap = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "CAP_WITH_CALENDAR".into(),
        rate_option_type: RateOptionType::Cap,
        notional,
        strike: 0.03,
        start_date: start,
        maturity: end,
        frequency: Some(Tenor::quarterly()),
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-LIBOR-3M".into(),
        vol_surface_id: "USD-CAP-VOL".into(),
    })
    .expect("valid strike");
    cap.calendar_id = Some("US_NERC".into());

    assert_eq!(cap.calendar_id.as_deref(), Some("US_NERC"));
}

#[test]
fn test_different_day_counts() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let day_counts = vec![
        DayCount::Act360,
        DayCount::Act365F,
        DayCount::Thirty360,
        DayCount::ActActIsma,
    ];

    for day_count in day_counts {
        let cap = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
            id: "CAP_DC_TEST".into(),
            rate_option_type: RateOptionType::Cap,
            notional,
            strike: 0.03,
            start_date: start,
            maturity: end,
            frequency: Some(Tenor::quarterly()),
            day_count,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-LIBOR-3M".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
        })
        .expect("valid strike");

        assert_eq!(cap.day_count, day_count);
    }
}

#[test]
fn test_different_frequencies() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2030, Month::January, 1).unwrap();

    let frequencies = vec![
        Tenor::monthly(),
        Tenor::quarterly(),
        Tenor::semi_annual(),
        Tenor::annual(),
    ];

    for frequency in frequencies {
        let cap = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
            id: "CAP_FREQ_TEST".into(),
            rate_option_type: RateOptionType::Cap,
            notional,
            strike: 0.03,
            start_date: start,
            maturity: end,
            frequency: Some(frequency),
            day_count: DayCount::Act360,
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-LIBOR-3M".into(),
            vol_surface_id: "USD-CAP-VOL".into(),
        })
        .expect("valid strike");

        assert_eq!(cap.frequency, frequency);
    }
}

#[test]
fn test_new_caplet_rejects_nan_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "CAPLET-NAN".into(),
        rate_option_type: RateOptionType::Caplet,
        notional,
        strike: f64::NAN,
        start_date: start,
        maturity: end,
        frequency: None,
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
    });
    assert!(result.is_err(), "CapFloor::new should reject NaN strike");
}

#[test]
fn test_new_caplet_rejects_infinite_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "CAPLET-INF".into(),
        rate_option_type: RateOptionType::Caplet,
        notional,
        strike: f64::INFINITY,
        start_date: start,
        maturity: end,
        frequency: None,
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
    });
    assert!(
        result.is_err(),
        "CapFloor::new should reject infinite strike"
    );
}

#[test]
fn test_new_floorlet_rejects_nan_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "FLOORLET-NAN".into(),
        rate_option_type: RateOptionType::Floorlet,
        notional,
        strike: f64::NAN,
        start_date: start,
        maturity: end,
        frequency: None,
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
    });
    assert!(result.is_err(), "CapFloor::new should reject NaN strike");
}

#[test]
fn test_new_caplet_accepts_valid_strike() {
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let start = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let end = Date::from_calendar_date(2026, Month::January, 1).unwrap();

    let result = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "CAPLET-OK".into(),
        rate_option_type: RateOptionType::Caplet,
        notional,
        strike: 0.05,
        start_date: start,
        maturity: end,
        frequency: None,
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        vol_surface_id: "USD_CAP_VOL".into(),
    });
    assert!(
        result.is_ok(),
        "CapFloor::new should accept a valid finite strike"
    );
}

#[test]
fn term_index_rejects_overnight_coupon_settings() {
    let mut caplet = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "TERM-WITH-OVERNIGHT-SETTINGS".into(),
        rate_option_type: RateOptionType::Caplet,
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: 0.05,
        start_date: Date::from_calendar_date(2025, Month::January, 2).unwrap(),
        maturity: Date::from_calendar_date(2025, Month::April, 2).unwrap(),
        frequency: None,
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-LIBOR-3M".into(),
        vol_surface_id: "USD-CAP-VOL".into(),
    })
    .expect("valid caplet");
    caplet.overnight_coupon = Some(OvernightCouponConvention {
        compounding: FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 },
        payment_lag_days: 2,
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
    let cap = CapFloor::new(finstack_quant_valuations_rates::cap_floor::CapFloorParams {
        id: "LEGACY-CAP".into(),
        rate_option_type: RateOptionType::Cap,
        notional: Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        strike: 0.05,
        start_date: Date::from_calendar_date(2025, Month::January, 2).unwrap(),
        maturity: Date::from_calendar_date(2026, Month::January, 2).unwrap(),
        frequency: Some(Tenor::quarterly()),
        day_count: DayCount::Act360,
        discount_curve_id: "USD-OIS".into(),
        forward_curve_id: "USD-LIBOR-3M".into(),
        vol_surface_id: "USD-CAP-VOL".into(),
    })
    .expect("valid cap");
    let mut value = serde_json::to_value(cap).expect("serialize cap");
    assert!(
        value.get("overnight_coupon").is_none(),
        "default overnight terms should be omitted for backward-compatible JSON"
    );
    assert!(
        value.get("spread_bp").is_none(),
        "zero spread_bp should be omitted for backward-compatible JSON"
    );
    value
        .as_object_mut()
        .expect("cap JSON object")
        .remove("spread_bp");

    let decoded: CapFloor = serde_json::from_value(value).expect("deserialize legacy cap JSON");
    assert!(decoded.overnight_coupon.is_none());
    assert_eq!(decoded.spread_bp, Decimal::ZERO);
}
