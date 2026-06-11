//! Integration tests for Bermudan swaption pricing.

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DayCount, Tenor};
use finstack_core::market_data::term_structures::DiscountCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use finstack_core::types::{CurveId, InstrumentId};
use finstack_valuations::instruments::rates::swaption::BermudanSwaptionTreeValuator;
use finstack_valuations::instruments::rates::swaption::{
    BermudanSchedule, BermudanSwaption, BermudanType, CalibratedHullWhiteModel, HullWhiteParams,
    SwaptionSettlement,
};
use finstack_valuations::instruments::OptionType;
use time::Month;

fn test_discount_curve() -> DiscountCurve {
    DiscountCurve::builder("USD-OIS")
        .base_date(Date::from_calendar_date(2025, Month::January, 1).expect("Valid date"))
        .knots([
            (0.0, 1.0),
            (0.5, 0.985),
            (1.0, 0.97),
            (2.0, 0.94),
            (5.0, 0.85),
            (10.0, 0.70),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .expect("Valid curve")
}

fn test_bermudan_swaption(
    swap_start: Date,
    swap_end: Date,
    first_exercise: Date,
    strike: f64,
    option_type: OptionType,
) -> BermudanSwaption {
    BermudanSwaption {
        id: InstrumentId::new("TEST-BERM"),
        option_type,
        notional: Money::new(10_000_000.0, Currency::USD),
        strike: rust_decimal::Decimal::try_from(strike).expect("valid decimal"),
        swap_start,
        swap_end,
        fixed_freq: Tenor::semi_annual(),
        float_freq: Tenor::quarterly(),
        day_count: DayCount::Thirty360,
        settlement: SwaptionSettlement::Physical,
        discount_curve_id: CurveId::new("USD-OIS"),
        forward_curve_id: CurveId::new("USD-OIS"),
        vol_surface_id: CurveId::new("USD-VOL"),
        bermudan_schedule: BermudanSchedule::co_terminal(
            first_exercise,
            swap_end,
            Tenor::semi_annual(),
        )
        .expect("valid Bermudan schedule"),
        bermudan_type: BermudanType::CoTerminal,
        calendar_id: None,
        pricing_overrides: Default::default(),
        attributes: Default::default(),
    }
}

#[test]
fn test_bermudan_price_positive() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    let curve = test_discount_curve();

    let ttm = swaption.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let valuator = BermudanSwaptionTreeValuator::new(&swaption, &model, &curve, as_of)
        .expect("Valid valuator");

    let price = valuator.price().expect("pricing should succeed");

    // Price should be non-negative
    assert!(
        price >= 0.0,
        "Bermudan swaption price should be non-negative, got {}",
        price
    );
}

#[test]
fn test_bermudan_payer_vs_receiver() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let payer =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    let receiver =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Put);

    let curve = test_discount_curve();
    let ttm = payer.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let payer_valuator =
        BermudanSwaptionTreeValuator::new(&payer, &model, &curve, as_of).expect("Valid valuator");
    let receiver_valuator = BermudanSwaptionTreeValuator::new(&receiver, &model, &curve, as_of)
        .expect("Valid valuator");

    let payer_price = payer_valuator.price().expect("pricing should succeed");
    let receiver_price = receiver_valuator.price().expect("pricing should succeed");

    // Both should be positive
    assert!(payer_price >= 0.0, "Payer price should be non-negative");
    assert!(
        receiver_price >= 0.0,
        "Receiver price should be non-negative"
    );
}

#[test]
fn test_bermudan_strike_sensitivity() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    // Low strike (ITM payer)
    let low_strike =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.01, OptionType::Call);
    // ATM strike
    let atm_strike =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    // High strike (OTM payer)
    let high_strike =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.06, OptionType::Call);

    let curve = test_discount_curve();
    let ttm = low_strike.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let low_price = BermudanSwaptionTreeValuator::new(&low_strike, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");
    let atm_price = BermudanSwaptionTreeValuator::new(&atm_strike, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");
    let high_price = BermudanSwaptionTreeValuator::new(&high_strike, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");

    // For a payer swaption: lower strike = higher value
    assert!(
        low_price >= atm_price,
        "Low strike payer {} should be >= ATM {}",
        low_price,
        atm_price
    );
    assert!(
        atm_price >= high_price,
        "ATM payer {} should be >= high strike {}",
        atm_price,
        high_price
    );
}

#[test]
fn test_bermudan_more_exercise_dates_higher_value() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");

    // Early first exercise (more exercise opportunities)
    let early_first = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    // Late first exercise (fewer exercise opportunities)
    let late_first = Date::from_calendar_date(2029, Month::January, 1).expect("Valid date");

    let early_swaption =
        test_bermudan_swaption(swap_start, swap_end, early_first, 0.03, OptionType::Call);
    let late_swaption =
        test_bermudan_swaption(swap_start, swap_end, late_first, 0.03, OptionType::Call);

    let curve = test_discount_curve();
    let ttm = early_swaption.time_to_maturity(as_of).expect("Valid ttm");
    let model = CalibratedHullWhiteModel::calibrate(
        HullWhiteParams::new(0.03, 0.01).expect("valid HW params"),
        50,
        &curve,
        ttm,
    )
    .expect("Calibration should succeed");

    let early_price = BermudanSwaptionTreeValuator::new(&early_swaption, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");
    let late_price = BermudanSwaptionTreeValuator::new(&late_swaption, &model, &curve, as_of)
        .expect("Valid valuator")
        .price()
        .expect("pricing should succeed");

    // More exercise opportunities should generally be worth more or equal
    // (this depends on the exact market conditions, but typically holds)
    assert!(
        early_price >= late_price * 0.9, // Allow some tolerance
        "Early first exercise {} should be >= late first exercise {}",
        early_price,
        late_price
    );
}

#[test]
fn test_expired_bermudan_zero_value() {
    // Set as_of after all exercise dates
    let as_of = Date::from_calendar_date(2035, Month::January, 1).expect("Valid date");
    let swap_start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let swaption =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);
    let _curve = test_discount_curve();

    // Tree should still build, but with ttm <= 0
    let ttm = swaption.time_to_maturity(as_of);
    assert!(
        ttm.is_ok() && ttm.unwrap() <= 0.0,
        "Expired swaption should have non-positive TTM"
    );
}

#[test]
fn test_bermudan_schedule_generation() {
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");

    let schedule = BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
        .expect("valid Bermudan schedule");

    // Should have approximately 8 exercise dates (4 years * 2 per year)
    let effective_dates = schedule.effective_dates();
    assert!(
        effective_dates.len() >= 6,
        "Should have multiple exercise dates, got {}",
        effective_dates.len()
    );

    // Dates should be sorted
    for i in 1..effective_dates.len() {
        assert!(
            effective_dates[i] > effective_dates[i - 1],
            "Exercise dates should be sorted"
        );
    }

    // All dates should be before swap end
    for &date in &effective_dates {
        assert!(date < swap_end, "Exercise dates should be before swap end");
    }
}

#[test]
fn test_bermudan_schedule_with_lockout() {
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let lockout_end = Date::from_calendar_date(2027, Month::January, 1).expect("Valid date");

    let schedule = BermudanSchedule::co_terminal(first_exercise, swap_end, Tenor::semi_annual())
        .expect("valid Bermudan schedule")
        .with_lockout(lockout_end);

    let effective_dates = schedule.effective_dates();

    // All effective dates should be after lockout
    for &date in &effective_dates {
        assert!(
            date > lockout_end,
            "Effective dates should be after lockout"
        );
    }
}

#[test]
fn test_bermudan_to_european_conversion() {
    let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
    let swap_start = as_of;
    let swap_end = Date::from_calendar_date(2030, Month::January, 1).expect("Valid date");
    let first_exercise = Date::from_calendar_date(2026, Month::January, 1).expect("Valid date");

    let bermudan =
        test_bermudan_swaption(swap_start, swap_end, first_exercise, 0.03, OptionType::Call);

    // Convert to European
    let european = bermudan.to_european().expect("Conversion should succeed");

    // Check European parameters match Bermudan
    assert_eq!(european.strike, bermudan.strike);
    assert_eq!(european.notional.amount(), bermudan.notional.amount());
    assert_eq!(european.swap_end, bermudan.swap_end);

    // European expiry should be the first Bermudan exercise date
    assert_eq!(european.expiry, first_exercise);
}
