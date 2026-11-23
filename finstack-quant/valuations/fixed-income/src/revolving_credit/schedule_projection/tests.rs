use super::super::types::RevolvingCredit;
use super::schedule::*;
use finstack_quant_cashflows::builder::{FloatingLegCompounding, RateSpec};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::Attributes;
use time::Month;

fn create_test_facility(
    start: Date,
    end: Date,
    payment_frequency: Tenor,
    rate: RateSpec,
    calendar_id: Option<&str>,
) -> RevolvingCredit {
    use finstack_quant_core::dates::StubKind;

    RevolvingCredit {
        id: "TEST-RC".into(),
        commitment: Money::from((10_000_000_i64, Currency::USD)),
        drawn: Money::from((5_000_000_i64, Currency::USD)),
        issue_date: start,
        maturity: end,
        rate,
        day_count: DayCount::Act360,
        yield_basis: finstack_quant_valuations_core::metrics::returns::YieldBasis::Act360,
        frequency: payment_frequency,
        quote_compounding: finstack_quant_core::math::Compounding::QUARTERLY,
        commitment_steps: Vec::new(),
        margin_steps: Vec::new(),
        lc: None,
        scheduled_fees: Vec::new(),
        oid_eir: None,
        fees: crate::revolving_credit::types::RevolvingCreditFees::default(),
        draw_repay_spec: crate::revolving_credit::types::DrawRepaySpec::Deterministic(vec![]),
        discount_curve_id: "USD-OIS".into(),
        credit_curve_id: None,
        recovery_rate: 0.0,
        leq: 0.0,
        stub: StubKind::ShortFront,
        business_day_convention:
            finstack_quant_core::dates::BusinessDayConvention::ModifiedFollowing,
        calendar_id: calendar_id.map(str::to_string).map(Into::into),
        payment_lag_days: 0,
        settlement_days: 0,
        instrument_pricing_overrides: Default::default(),
        metric_pricing_overrides: Default::default(),
        scenario_pricing_overrides: Default::default(),
        attributes: Attributes::new(),
    }
}

#[test]
fn test_build_payment_dates_ends_on_or_before_maturity() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let facility = create_test_facility(
        start,
        end,
        Tenor::quarterly(),
        RateSpec::Fixed { rate: 0.05 },
        None,
    );

    let dates =
        build_payment_dates(&facility).expect("Payment dates building should succeed in test");
    assert!(dates.len() >= 2);
    // Verify no sentinel: last date should be at or before maturity
    assert!(*dates.last().expect("Dates should not be empty") <= end);
}

#[test]
fn payment_adjustment_does_not_move_accrual_boundaries() {
    let start = Date::from_calendar_date(2026, Month::January, 3).expect("date");
    let maturity = Date::from_calendar_date(2027, Month::January, 3).expect("date");
    let facility = create_test_facility(
        start,
        maturity,
        Tenor::annual(),
        RateSpec::Fixed { rate: 0.05 },
        None,
    );

    let periods = build_payment_periods(&facility).expect("periods");
    let final_period = periods.last().expect("period");
    assert_eq!(final_period.accrual_end, maturity);
    assert_eq!(
        final_period.payment_date,
        Date::from_calendar_date(2027, Month::January, 4).expect("date")
    );

    let observation_dates = build_accrual_boundary_dates(&facility).expect("boundaries");
    assert_eq!(observation_dates.last(), Some(&maturity));
}

#[test]
fn test_build_reset_dates_fixed_returns_none() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let facility = create_test_facility(
        start,
        end,
        Tenor::quarterly(),
        RateSpec::Fixed { rate: 0.05 },
        None,
    );

    let reset_dates =
        build_reset_dates(&facility).expect("Reset dates building should succeed in test");
    assert!(reset_dates.is_none());
}

#[test]
fn test_build_reset_dates_floating_returns_some() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");
    let facility = create_test_facility(
        start,
        end,
        Tenor::quarterly(),
        RateSpec::Floating(finstack_quant_cashflows::builder::FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(200.0).expect("valid"),
            gearing: rust_decimal::Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: Default::default(),
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            fixing_calendar_id: None,
            compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
        }),
        None,
    );

    let reset_dates =
        build_reset_dates(&facility).expect("Reset dates building should succeed in test");
    assert!(reset_dates.is_some());
    let dates = reset_dates.expect("Reset dates should exist for floating rate");
    assert!(dates.len() >= 2);
}

#[test]
fn floating_projection_applies_full_coupon_economics() {
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use rust_decimal::Decimal;

    let reset = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let spec = finstack_quant_cashflows::builder::FloatingRateSpec {
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: "USD-SOFR-3M".into(),
        spread_bp: Decimal::from(100),
        gearing: Decimal::new(5, 1),
        gearing_includes_spread: false,
        index_floor_bp: None,
        index_cap_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: Some(Decimal::from(200)),
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: None,
        compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    };
    let forward = ForwardCurve::flat(
        "USD-SOFR-3M",
        reset,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("forward curve");

    let rate = finstack_quant_cashflows::builder::project_floating_rate(
        reset,
        &forward,
        &finstack_quant_cashflows::builder::FloatingRateParams::try_from(&spec)
            .expect("rate params"),
        reset,
        Date::from_calendar_date(2025, Month::April, 2).expect("date"),
        90.0 / 360.0,
    )
    .expect("projected coupon");
    assert!((rate - 0.02).abs() < 1e-12, "all-in cap must bind: {rate}");
}

#[test]
fn overnight_index_uses_shared_compounding_engine() {
    use finstack_quant_core::dates::calendar_by_id_strict;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use finstack_quant_valuations_core::instruments::pricing::overnight::{
        OvernightCouponProjectionInput, OvernightProjectionCurve, project_overnight_coupon,
    };
    use rust_decimal::Decimal;

    let start = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let end = Date::from_calendar_date(2025, Month::April, 2).expect("date");
    let spec = finstack_quant_cashflows::builder::FloatingRateSpec {
        index_id: "USD-SOFR-OIS".into(),
        forward_curve_id: "USD-SOFR-OIS".into(),
        spread_bp: Decimal::ZERO,
        gearing: Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: None,
        index_cap_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::quarterly(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: Some("usny".into()),
        compounding: None,
        overnight_basis: None,
        fallback: Default::default(),
    };
    let forward = ForwardCurve::flat(
        "USD-SOFR-OIS",
        start,
        0.04,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("forward curve");
    let revolver_rate = project_revolver_floating_rate(
        RevolverFloatingProjection {
            accrual_start: start,
            accrual_end: end,
            as_of: start,
            spec: &spec,
            fwd: &forward,
            day_count: DayCount::Act360,
            coupon_frequency: Tenor::quarterly(),
            coupon_period: (start, end),
            currency: Currency::USD,
            calendar_id: None,
            margin_delta_bp: 0.0,
            fixings: None,
        },
        None,
    )
    .expect("overnight revolver coupon");

    let compounding = resolved_overnight_compounding(&spec)
        .expect("resolve")
        .expect("overnight");
    let calendar = calendar_by_id_strict("usny").expect("usny");
    let expected = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&forward),
        fixings: None,
        fixing_id: "USD-SOFR-OIS",
        as_of: start,
        accrual_start: start,
        accrual_end: end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: Some(Tenor::quarterly()),
        coupon_period: (start, end),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("shared overnight coupon");
    assert!(
        (revolver_rate - expected.rate).abs() < 1e-12,
        "revolver overnight rate {revolver_rate} != shared engine {}",
        expected.rate
    );
}

#[test]
fn act365l_overnight_slices_keep_the_actual_annual_coupon_denominator() {
    use finstack_quant_core::dates::calendar_by_id_strict;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use rust_decimal::Decimal;
    use time::macros::date;

    let coupon_start = date!(2023 - 03 - 01);
    let coupon_end = date!(2024 - 03 - 01);
    let split = date!(2023 - 06 - 01);
    let spec = finstack_quant_cashflows::builder::FloatingRateSpec {
        index_id: "USD-SOFR-OIS".into(),
        forward_curve_id: "USD-SOFR-OIS".into(),
        spread_bp: Decimal::ZERO,
        gearing: Decimal::ONE,
        gearing_includes_spread: true,
        index_floor_bp: None,
        index_cap_bp: None,
        all_in_floor_bp: None,
        all_in_cap_bp: None,
        overnight_index_constraints: Default::default(),
        reset_frequency: Tenor::annual(),
        index_tenor: None,
        reset_lag_days: 0,
        fixing_calendar_id: Some("weekends_only".into()),
        compounding: Some(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }),
        overnight_basis: None,
        fallback: Default::default(),
    };
    let forward = ForwardCurve::flat("USD-SOFR-OIS", coupon_start, 0.05, DayCount::Act365F)
        .expect("flat overnight curve");
    let calendar = calendar_by_id_strict("weekends_only").expect("weekend calendar");
    for (start, end, margin_delta_bp) in [(coupon_start, split, 0.0), (split, coupon_end, 100.0)] {
        let projected = project_revolver_floating_rate(
            RevolverFloatingProjection {
                accrual_start: start,
                accrual_end: end,
                as_of: coupon_start,
                spec: &spec,
                fwd: &forward,
                day_count: DayCount::Act365L,
                coupon_frequency: Tenor::annual(),
                coupon_period: (coupon_start, coupon_end),
                currency: Currency::USD,
                calendar_id: Some("weekends_only"),
                margin_delta_bp,
                fixings: None,
            },
            None,
        )
        .expect("overnight coupon slice");
        let mut factor = 1.0;
        let mut date = start;
        while date < end {
            let next = date
                .add_business_days(1, calendar)
                .expect("next weekday")
                .min(end);
            // Projection growth uses the curve's continuous ACT/365F clock; the
            // resulting coupon is annualized on contractual ACT/365L.
            factor *= (0.05 * (next - date).whole_days() as f64 / 365.0).exp();
            date = next;
        }
        let expected =
            (factor - 1.0) / ((end - start).whole_days() as f64 / 366.0) + margin_delta_bp * 1e-4;
        assert!((projected - expected).abs() < 1e-13, "{start} -> {end}");
    }
}

/// A 0% SOFR index floor applied daily floors every negative fixing
/// before compounding; applied per period it floors only the compounded
/// rate. Fixings are -1% before 2025-02-14 and +2% from it, all realized.
/// The expected rates are compounded by hand over the usny business days.
#[test]
fn overnight_index_floor_applies_daily_or_per_period() {
    use finstack_quant_cashflows::builder::{
        FloatingLegCompounding, OvernightIndexConstraintApplication,
    };
    use finstack_quant_core::dates::{DateExt, calendar_by_id_strict};
    use finstack_quant_core::market_data::scalars::ScalarTimeSeries;
    use finstack_quant_core::market_data::term_structures::ForwardCurve;
    use rust_decimal::Decimal;

    let start = Date::from_calendar_date(2025, Month::January, 2).expect("date");
    let end = Date::from_calendar_date(2025, Month::April, 2).expect("date");
    let switch = Date::from_calendar_date(2025, Month::February, 14).expect("date");
    let fixing = |date: Date| if date < switch { -0.01 } else { 0.02 };
    let calendar = calendar_by_id_strict("usny").expect("usny");
    let mut observations = Vec::new();
    let mut date = start;
    while date < end {
        observations.push((date, fixing(date)));
        date = date
            .add_business_days(1, calendar)
            .expect("next business day");
    }
    let series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-OIS",
        observations.clone(),
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings");

    // Hand calculation: ∏(1 + r_i · d_i / 360) over each business day,
    // d_i the calendar days to the next business day.
    let compound = |floor: bool| -> f64 {
        let mut product = 1.0;
        for (i, (date, rate)) in observations.iter().enumerate() {
            let next = observations.get(i + 1).map_or(end, |(next, _)| *next);
            let days = f64::from(i32::try_from((next - *date).whole_days()).expect("days"));
            let rate = if floor { rate.max(0.0) } else { *rate };
            product *= 1.0 + rate * days / 360.0;
        }
        let period_days = f64::from(i32::try_from((end - start).whole_days()).expect("days"));
        (product - 1.0) * 360.0 / period_days
    };
    let daily_expected = compound(true);
    let period_expected = compound(false).max(0.0);
    assert!(
        daily_expected > period_expected + 1e-4,
        "fixings straddle zero"
    );

    let forward = ForwardCurve::flat(
        "USD-SOFR-OIS",
        start,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("forward curve");
    let rate = |application: OvernightIndexConstraintApplication| -> f64 {
        let spec = finstack_quant_cashflows::builder::FloatingRateSpec {
            index_id: "USD-SOFR-OIS".into(),
            forward_curve_id: "USD-SOFR-OIS".into(),
            spread_bp: Decimal::ZERO,
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: Some(Decimal::ZERO),
            index_cap_bp: None,
            all_in_floor_bp: None,
            all_in_cap_bp: None,
            overnight_index_constraints: application,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: Some("usny".into()),
            compounding: Some(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 }),
            overnight_basis: Some(DayCount::Act360),
            fallback: Default::default(),
        };
        project_revolver_floating_rate(
            RevolverFloatingProjection {
                accrual_start: start,
                accrual_end: end,
                as_of: end,
                spec: &spec,
                fwd: &forward,
                day_count: DayCount::Act360,
                coupon_frequency: Tenor::quarterly(),
                coupon_period: (start, end),
                currency: Currency::USD,
                calendar_id: None,
                margin_delta_bp: 0.0,
                fixings: Some(&series),
            },
            None,
        )
        .expect("overnight coupon")
    };
    let daily = rate(OvernightIndexConstraintApplication::Daily);
    let period = rate(OvernightIndexConstraintApplication::Period);
    assert!(
        (daily - daily_expected).abs() < 1e-12,
        "daily floor {daily} vs hand {daily_expected}"
    );
    assert!(
        (period - period_expected).abs() < 1e-12,
        "period floor {period} vs hand {period_expected}"
    );
}

#[test]
fn test_calendar_adjustment_applied() {
    let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid test date");
    let end = Date::from_calendar_date(2026, Month::January, 1).expect("Valid test date");

    // Without calendar
    let facility_no_cal = create_test_facility(
        start,
        end,
        Tenor::quarterly(),
        RateSpec::Fixed { rate: 0.05 },
        None,
    );
    let dates_no_cal = build_payment_dates(&facility_no_cal)
        .expect("Payment dates building should succeed in test");

    // With NYSE calendar
    let facility_with_cal = create_test_facility(
        start,
        end,
        Tenor::quarterly(),
        RateSpec::Fixed { rate: 0.05 },
        Some("nyse"),
    );
    let dates_with_cal = build_payment_dates(&facility_with_cal)
        .expect("Payment dates building should succeed in test");

    // Both should have same length (quarterly over 1 year)
    assert_eq!(dates_no_cal.len(), dates_with_cal.len());
}

#[test]
fn test_apply_draw_repay_event_draw() {
    use crate::revolving_credit::types::DrawRepayEvent;

    let balance = Money::from((5_000_000_i64, Currency::USD));
    let commitment = Money::from((10_000_000_i64, Currency::USD));
    let draw_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

    let event = DrawRepayEvent {
        date: draw_date,
        amount: Money::from((2_000_000_i64, Currency::USD)),
        is_draw: true,
    };

    let new_balance = apply_draw_repay_event(balance, &event, commitment)
        .expect("Draw/repay event application should succeed");
    assert_eq!(new_balance.amount(), 7_000_000.0);
}

#[test]
fn test_apply_draw_repay_event_repay() {
    use crate::revolving_credit::types::DrawRepayEvent;

    let balance = Money::from((5_000_000_i64, Currency::USD));
    let commitment = Money::from((10_000_000_i64, Currency::USD));
    let repay_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

    let event = DrawRepayEvent {
        date: repay_date,
        amount: Money::from((1_000_000_i64, Currency::USD)),
        is_draw: false,
    };

    let new_balance = apply_draw_repay_event(balance, &event, commitment)
        .expect("Draw/repay event application should succeed");
    assert_eq!(new_balance.amount(), 4_000_000.0);
}

#[test]
fn test_apply_draw_repay_event_exceeds_commitment() {
    use crate::revolving_credit::types::DrawRepayEvent;

    let balance = Money::from((8_000_000_i64, Currency::USD));
    let commitment = Money::from((10_000_000_i64, Currency::USD));
    let draw_date = Date::from_calendar_date(2025, Month::March, 1).expect("Valid test date");

    let event = DrawRepayEvent {
        date: draw_date,
        amount: Money::from((3_000_000_i64, Currency::USD)), // Would exceed commitment
        is_draw: true,
    };

    let result = apply_draw_repay_event(balance, &event, commitment);
    assert!(result.is_err());
    assert!(
        result
            .expect_err("should fail")
            .to_string()
            .contains("exceed commitment")
    );
}

#[test]
fn resolve_fixings_propagates_invalid_index_id_and_allows_missing_valid_series() {
    use crate::revolving_credit::cashflow_engine::CashflowEngine;
    use finstack_quant_core::ErrorKind;
    use finstack_quant_core::market_data::scalars::{
        ScalarTimeSeries, SeriesExtrapolation, SeriesInterpolation,
    };
    use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
    use rust_decimal::Decimal;
    use time::macros::date;

    let start = date!(2025 - 01 - 02);
    let end = date!(2026 - 01 - 02);
    let make_floating = |index_id: &str| {
        create_test_facility(
            start,
            end,
            Tenor::quarterly(),
            RateSpec::Floating(finstack_quant_cashflows::builder::FloatingRateSpec {
                index_id: index_id.into(),
                forward_curve_id: "USD-SOFR-3M".into(),
                spread_bp: Decimal::from(200),
                gearing: Decimal::ONE,
                gearing_includes_spread: true,
                index_floor_bp: None,
                index_cap_bp: None,
                all_in_floor_bp: None,
                all_in_cap_bp: None,
                overnight_index_constraints: Default::default(),
                reset_frequency: Tenor::quarterly(),
                index_tenor: None,
                reset_lag_days: 0,
                fixing_calendar_id: None,
                compounding: None,
                overnight_basis: None,
                fallback: Default::default(),
            }),
            None,
        )
    };

    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (2.0, 0.92)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let fwd =
        ForwardCurve::flat("USD-SOFR-3M", start, 0.04, DayCount::Act365F).expect("forward curve");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("insert discount curve")
        .try_insert(fwd)
        .expect("insert forward curve");

    // Valid missing fixing series on an unseasoned facility resolves cleanly to Ok(None).
    let valid_facility = make_floating("USD-SOFR-3M");
    assert!(
        resolve_fixings(&valid_facility, &market)
            .expect("missing valid fixing series should return Ok(None)")
            .is_none()
    );
    assert!(CashflowEngine::deterministic(&valid_facility, &market, start, &[]).is_ok());

    // Invalid index_id must propagate a Validation error rather than silently returning None.
    for invalid_id in ["FIXING:USD-SOFR-3M", "", "  ", " USD-SOFR-3M"] {
        let invalid_facility = make_floating(invalid_id);
        let err = resolve_fixings(&invalid_facility, &market)
            .expect_err("invalid fixing index_id must fail validation");
        assert_eq!(
            err.kind(),
            ErrorKind::Validation,
            "expected Validation error for '{invalid_id}', got: {err}"
        );

        let engine_err = CashflowEngine::deterministic(&invalid_facility, &market, start, &[])
            .expect_err("unseasoned deterministic schedule must propagate invalid index_id");
        assert_eq!(engine_err.kind(), ErrorKind::Validation);
    }

    // Fixed-rate facility returns Ok(None).
    let fixed_facility = create_test_facility(
        start,
        end,
        Tenor::quarterly(),
        RateSpec::Fixed { rate: 0.05 },
        None,
    );
    assert!(
        resolve_fixings(&fixed_facility, &market)
            .expect("fixed facility should return Ok(None)")
            .is_none()
    );

    // Present valid fixing series is returned.
    let series = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-3M",
        vec![(start, 0.045)],
        None,
        SeriesInterpolation::Step,
        SeriesExtrapolation::Error,
    )
    .expect("series");
    let market_with_fixings = market.insert_series(series);
    let resolved = resolve_fixings(&valid_facility, &market_with_fixings)
        .expect("present fixing series should succeed")
        .expect("present fixing series should be Some");
    assert_eq!(resolved.id().as_str(), "FIXING:USD-SOFR-3M");
}

#[test]
fn resolved_overnight_compounding_prefers_explicit_then_index_id() {
    let explicit = FloatingLegCompounding::CompoundedInArrears { lookback_days: 5 };
    assert_eq!(
        resolved_overnight_compounding_for_index("USD-SOFR-3M", Some(&explicit)).expect("explicit"),
        Some(explicit)
    );
    assert_eq!(
        resolved_overnight_compounding_for_index(
            "USD-SOFR-OIS",
            Some(&FloatingLegCompounding::Simple)
        )
        .expect("explicit simple overrides index"),
        None
    );
    assert!(
        resolved_overnight_compounding_for_index(
            "USD-SOFR-OIS",
            Some(&FloatingLegCompounding::SimpleAverage),
        )
        .is_err()
    );
    assert_eq!(
        resolved_overnight_compounding_for_index("USD-SOFR-OIS", None)
            .expect("registered overnight index"),
        Some(FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 })
    );
    assert_eq!(
        resolved_overnight_compounding_for_index("USD-SOFR-3M", None)
            .expect("registered term index"),
        None
    );
}
