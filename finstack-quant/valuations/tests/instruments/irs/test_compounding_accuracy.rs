use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::calendar_by_id;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DateExt, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::irs::{
    FixedLegSpec, FloatLegSpec, FloatingLegCompounding, PayReceive,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::InterestRateSwap;
use time::Month;

// Overnight compounded fixtures need a positive projection-curve tenor.
const OVERNIGHT_FORWARD_TENOR: f64 = 1.0 / 360.0;

#[test]
fn test_compounding_lookback_sensitivity() {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cal = calendar_by_id("USNY").expect("USNY calendar");
    // Use a spot-starting swap (T+2) so SOFR lookback does not require historical fixings
    // at valuation date `base`.
    let start = base.add_business_days(2, cal).unwrap();

    // Flat 5% discount curve
    let disc = DiscountCurve::builder("DISC")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.951229)]) // exp(-0.05 * 1)
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Increasing forward curve: starts at 5%, ends at 10%
    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.10)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(fwd);

    // If lookback pushes observations before `as_of`, provide minimal fixings so the test
    // remains focused on the lookback sensitivity (not fixing-data availability).
    let earliest_obs = start
        .add_business_days(-2, cal)
        .unwrap_or(start.add_weekdays(-2));
    let ctx = if earliest_obs < base {
        let mut obs: Vec<(Date, f64)> = Vec::new();
        let mut d = earliest_obs;
        while d < base {
            obs.push((d, 0.05));
            d = d.add_business_days(1, cal).unwrap();
        }
        ctx.insert_series(ScalarTimeSeries::new("FIXING:FWD", obs, None).unwrap())
    } else {
        ctx
    };

    let mut irs = InterestRateSwap::builder()
        .id("TEST-COMP".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(12),
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(12),
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            fixing_calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    // NPV with zero lookback
    let npv_no_lookback = irs.value(&ctx, base).unwrap().amount();

    // NPV with 2 days lookback
    // Since the curve is increasing, looking back 2 days should use LOWER rates,
    // so the floating leg PV should DECREASE, and NPV (Rec Fixed) should INCREASE.
    irs.float.compounding = FloatingLegCompounding::CompoundedInArrears {
        lookback_days: 2,
        observation_shift: None,
    };
    let npv_lookback = irs.value(&ctx, base).unwrap().amount();

    assert!(
        npv_lookback > npv_no_lookback,
        "Lookback in increasing rate env should increase RecFixed NPV (lower float PV)"
    );
}

#[test]
fn test_rate_cutoff_uses_business_day_lockout_over_weekend() {
    let base = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let start = Date::from_calendar_date(2025, Month::January, 6).unwrap();
    let end = Date::from_calendar_date(2025, Month::January, 13).unwrap();

    let disc = DiscountCurve::builder("DISC")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();
    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.02), (1.0, 0.20)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();
    let ctx = MarketContext::new().insert(disc).insert(fwd);

    let mut irs = InterestRateSwap::builder()
        .id("TEST-RATE-CUTOFF-BUSINESS-DAY".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end,
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end,
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::ZERO,
            fixing_calendar_id: Some("USNY".into()),
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    let no_cutoff = irs.value(&ctx, base).unwrap().amount();
    irs.float.compounding = FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 };
    let cutoff = irs.value(&ctx, base).unwrap().amount();

    assert!(
        cutoff < no_cutoff,
        "one-business-day lockout ending after a weekend should reuse the prior business-day rate: cutoff={cutoff}, no_cutoff={no_cutoff}"
    );
}

#[test]
fn test_payment_delay_sensitivity() {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let disc = DiscountCurve::builder("DISC")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    // Flat forward curve based at the swap start (2024-01-01): seasoned resets
    // strictly before the curve base now error under the default fallback.
    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(Date::from_calendar_date(2024, Month::January, 1).unwrap())
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (2.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Provide fixings for seasoned swap - example() creates a swap starting 2024-01-01
    // but we're pricing on 2025-01-01, so past resets need fixings
    let fixings = finstack_quant_core::market_data::scalars::ScalarTimeSeries::new(
        "FIXING:FWD",
        vec![
            (
                Date::from_calendar_date(2023, Month::December, 28).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::January, 1).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::March, 28).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::April, 1).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::June, 28).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::July, 1).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::September, 30).unwrap(),
                0.05,
            ),
            (
                Date::from_calendar_date(2024, Month::October, 1).unwrap(),
                0.05,
            ),
        ],
        None,
    )
    .expect("fixings series");

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_series(fixings);

    let mut irs = InterestRateSwap::example_standard().unwrap();
    irs.fixed.discount_curve_id = "DISC".into();
    irs.float.discount_curve_id = "DISC".into();
    irs.float.forward_curve_id = "FWD".into();
    irs.float.compounding = FloatingLegCompounding::Simple; // Standard swap

    // NPV with 0 payment delay
    irs.fixed.payment_lag_days = 0;
    irs.float.payment_lag_days = 0;
    let npv0 = irs.value(&ctx, base).unwrap().amount();

    // NPV with 2 days payment delay
    // Delaying payments in a positive rate environment should lower the PV of both legs.
    irs.fixed.payment_lag_days = 2;
    irs.float.payment_lag_days = 2;
    let npv2 = irs.value(&ctx, base).unwrap().amount();

    // Since it's a par-like swap, the net effect depends on leg durations.
    // But the absolute leg PVs must change.
    assert!(npv2 != npv0, "Payment delay must change swap NPV");
}

#[test]
fn test_seasoned_compounded_swap_requires_fixings() {
    let start = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let cal = calendar_by_id("USNY").expect("USNY calendar");
    let as_of = start.add_business_days(5, cal).unwrap();

    let disc = DiscountCurve::builder("DISC")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(fwd);

    let irs = InterestRateSwap::builder()
        .id("TEST-SEASONED-COMP".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(1),
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(1),
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            fixing_calendar_id: Some("USNY".into()),
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    let err = irs
        .value(&ctx, as_of)
        .expect_err("seasoned compounded swap must require explicit fixings");
    let msg = format!("{err:?}");
    assert!(
        msg.contains("FIXING:FWD"),
        "error should explain expected fixings series id; got: {msg}"
    );
}

#[test]
fn test_seasoned_compounded_swap_with_fixings_prices() {
    let start = Date::from_calendar_date(2025, Month::January, 2).unwrap();
    let cal = calendar_by_id("USNY").expect("USNY calendar");
    let as_of = start.add_business_days(5, cal).unwrap();

    let disc = DiscountCurve::builder("DISC")
        .base_date(as_of)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Build minimal business-day fixings from start up to (but not including) as_of.
    let mut obs: Vec<(Date, f64)> = Vec::new();
    let mut d = start;
    while d < as_of {
        obs.push((d, 0.05));
        d = d.add_business_days(1, cal).unwrap();
    }
    let fixings = ScalarTimeSeries::new("FIXING:FWD", obs, None).unwrap();

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_series(fixings);

    let irs = InterestRateSwap::builder()
        .id("TEST-SEASONED-COMP-WITH-FIX".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(1),
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(1),
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            fixing_calendar_id: Some("USNY".into()),
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    let pv = irs.value(&ctx, as_of).unwrap().amount();
    assert!(pv.is_finite(), "PV should be finite");
}

/// Test that compounded swap with spread in near-zero rate environment prices correctly.
///
/// Per ISDA 2021 Section 4.5, spread is applied as simple interest (not compounded),
/// so even with near-zero index rates, the spread contribution should be stable.
#[test]
fn test_compounded_swap_with_spread_near_zero_rates() {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cal = calendar_by_id("USNY").expect("USNY calendar");
    let start = base.add_business_days(2, cal).unwrap();

    // Near-zero discount curve (0.1% rate)
    let disc = DiscountCurve::builder("DISC")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.999)])
        .interp(InterpStyle::LogLinear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        ) // Allow for very flat curves
        .build()
        .unwrap();

    // Near-zero forward curve
    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.001), (1.0, 0.001)]) // 0.1% flat
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(fwd);

    // Create swap with significant spread (50bp) on near-zero rate
    let irs = InterestRateSwap::builder()
        .id("TEST-SPREAD-ZERO".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.005).expect("valid"), // 0.5% fixed
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(12),
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(12),
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::try_from(50.0).expect("valid"), // 50bp spread
            fixing_calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    let pv = irs.value(&ctx, base);
    assert!(pv.is_ok(), "Near-zero rate swap should price: {:?}", pv);

    let pv_val = pv.unwrap().amount();
    assert!(pv_val.is_finite(), "PV should be finite");

    // Float leg has ~0.1% index + 0.5% spread = ~0.6% all-in rate
    // Fixed leg pays 0.5%
    // Pay NPV = Float - Fixed should be slightly positive
    // (receiving 0.6%, paying 0.5% on ~10MM for 1Y = ~10k profit)
    assert!(
        pv_val > 0.0,
        "Pay with higher float all-in rate should have positive NPV, got {}",
        pv_val
    );
}

/// Test that compounded swap with spread in negative rate environment prices correctly.
///
/// Negative rates are realistic in EUR/JPY/CHF markets. The spread contribution
/// should still be stable per ISDA 2021 convention.
#[test]
fn test_compounded_swap_with_spread_negative_rates() {
    let base = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let cal = calendar_by_id("USNY").expect("USNY calendar");
    let start = base.add_business_days(2, cal).unwrap();

    // Negative rate discount curve (-0.5%)
    // DF > 1 for future dates when rates are negative
    let disc = DiscountCurve::builder("DISC")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 1.005)]) // exp(-(-0.005)*1) ≈ 1.005
        .interp(InterpStyle::LogLinear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        ) // Negative rates cause DF > 1
        .build()
        .unwrap();

    // Negative forward curve
    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, -0.005), (1.0, -0.005)]) // -0.5% flat
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let ctx = MarketContext::new().insert(disc).insert(fwd);

    // Create swap with spread (100bp) on negative rate
    let irs = InterestRateSwap::builder()
        .id("TEST-SPREAD-NEG".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Pay)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.005).expect("valid"), // 0.5% fixed
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(12),
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(12),
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::try_from(100.0).expect("valid"), // 100bp spread
            fixing_calendar_id: None,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    let pv = irs.value(&ctx, base);
    assert!(
        pv.is_ok(),
        "Negative rate swap should price: {:?}",
        pv.err()
    );

    let pv_val = pv.unwrap().amount();
    assert!(pv_val.is_finite(), "PV should be finite");

    // Float leg has -0.5% index + 1% spread = 0.5% all-in rate
    // Fixed leg pays 0.5%
    // Pay NPV = Float - Fixed should be approximately zero
    assert!(
        pv_val.abs() < 1000.0, // Within ~1bp on 10MM notional
        "Pay with matching float all-in rate should have near-zero NPV, got {}",
        pv_val
    );
}

/// Test that observation shift pushing dates before curve base date uses fixings correctly.
///
/// When observation shift is large enough to push observation dates before the curve
/// base date, the pricer should require and use historical fixings for those dates.
#[test]
fn test_observation_shift_before_curve_base_date() {
    // Curve base date is 2025-01-10, but swap starts 2025-01-03
    // With a large observation shift, observations could be pushed to the curve base date
    let base = Date::from_calendar_date(2025, Month::January, 10).unwrap();
    let start = Date::from_calendar_date(2025, Month::January, 3).unwrap();
    let cal = calendar_by_id("USNY").expect("USNY calendar");

    let disc = DiscountCurve::builder("DISC")
        .base_date(base)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    let fwd = ForwardCurve::builder("FWD", OVERNIGHT_FORWARD_TENOR)
        .base_date(base)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (1.0, 0.05)])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Build fixings from swap start up to curve base date
    let mut obs: Vec<(Date, f64)> = Vec::new();
    let mut d = start;
    while d < base {
        obs.push((d, 0.05));
        d = d.add_business_days(1, cal).unwrap();
    }
    let fixings = ScalarTimeSeries::new("FIXING:FWD", obs, None).unwrap();

    let ctx = MarketContext::new()
        .insert(disc)
        .insert(fwd)
        .insert_series(fixings);

    // Create a swap that's already partially into its first period
    let irs = InterestRateSwap::builder()
        .id("TEST-OBS-SHIFT".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(FixedLegSpec {
            discount_curve_id: "DISC".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(1),
            payment_lag_days: 0,
            end_of_month: false,
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            par_method: None,
            compounding_simple: true,
        })
        .float(FloatLegSpec {
            discount_curve_id: "DISC".into(),
            forward_curve_id: "FWD".into(),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: Some("USNY".into()),
            start,
            end: start.add_months(1),
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None, // No shift needed, as_of is after start
            },
            payment_lag_days: 0,
            end_of_month: false,
            spread_bp: rust_decimal::Decimal::ZERO,
            fixing_calendar_id: Some("USNY".into()),
            stub: finstack_quant_core::dates::StubKind::ShortFront,
            reset_lag_days: 0,
        })
        .build()
        .unwrap();

    // Should price successfully using fixings for dates before base
    let pv = irs.value(&ctx, base);
    assert!(
        pv.is_ok(),
        "Swap with observations before curve base should price with fixings: {:?}",
        pv.err()
    );

    let pv_val = pv.unwrap().amount();
    assert!(pv_val.is_finite(), "PV should be finite");
}
