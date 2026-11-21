use super::*;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use time::macros::date;

#[test]
fn overnight_projection_preserves_index_rates_across_curve_clocks() {
    let start = date!(2025 - 01 - 02);
    let end = date!(2025 - 04 - 02);
    let calendar = finstack_quant_core::dates::calendar_by_id_strict("usny").expect("USD calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let mut factors = Vec::new();
    for (curve_day_count, _curve_rate) in [
        (DayCount::Act360, 0.04),
        (DayCount::Act365F, 0.04 * 365.0 / 360.0),
    ] {
        let mut points = vec![(start, 1.0)];
        let mut d = start;
        let mut factor = 1.0;
        while d < end {
            let next = d.add_business_days(1, calendar).expect("next bd").min(end);
            let alpha = (next - d).whole_days() as f64 / 360.0;
            factor /= 1.0 + 0.04 * alpha;
            points.push((next, factor));
            d = next;
        }
        let bumped_curve = |delta_rate: f64| {
            let mut pts = vec![(start, 1.0)];
            let mut d = start;
            let mut fac = 1.0;
            while d < end {
                let next = d.add_business_days(1, calendar).expect("next bd").min(end);
                let alpha = (next - d).whole_days() as f64 / 360.0;
                fac /= 1.0 + (0.04 + delta_rate) * alpha;
                pts.push((next, fac));
                d = next;
            }
            ForwardCurve::from_dates(
                "USD-SOFR",
                start,
                &pts,
                curve_day_count,
                finstack_quant_core::math::interp::InterpStyle::LogLinear,
                finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
            )
            .expect("forward curve")
        };
        let forward = bumped_curve(0.0);
        let project = |curve: &ForwardCurve| {
            project_overnight_coupon(OvernightCouponProjectionInput {
                curve: OvernightProjectionCurve::Forward(curve),
                fixings: None,
                fixing_id: "USD-SOFR",
                as_of: start,
                accrual_start: start,
                accrual_end: end,
                end_is_termination_date: None,
                day_count: DayCount::Act360,
                coupon_frequency: None,
                coupon_period: (start, end),
                compounding: &compounding,
                fixing_calendar: calendar,
                compounded_spread: 0.0,
                need_observation_exposures: true,
            })
            .expect("overnight projection")
        };
        let projection = project(&forward);
        for observation in &projection.observation_exposures {
            assert!((observation.projected_rate - 0.04).abs() < 1e-13);
        }
        let up = bumped_curve(1e-6);
        let down = bumped_curve(-1e-6);
        let finite_difference = (project(&up).rate - project(&down).rate) / 2e-6;
        assert!((projection.parallel_forward_sensitivity - finite_difference).abs() < 1e-8);
        factors.push(projection.compound_factor);
    }
    assert!((factors[0] - factors[1]).abs() < 1e-13);
}

#[test]
fn overnight_coupon_amount_uses_compound_factor_and_projection_yf() {
    let projection = OvernightCouponProjection {
        rate: 0.04,
        accrual_year_fraction: 0.25,
        compound_factor: 1.01,
        parallel_forward_sensitivity: 0.0,
        parallel_forward_second_sensitivity: 0.0,
        fixing_date: date!(2025 - 04 - 02),
        observation_exposures: Vec::new(),
    };
    let notional = 1_000_000.0;
    let amount = projection.coupon_amount(notional, 0.001);
    let expected = notional * (1.01 - 1.0) + notional * 0.001 * 0.25;
    assert!(
        (amount - expected).abs() < 1e-12,
        "overnight coupon amount {amount} != N*(CF-1) + N*spread*yf {expected}"
    );
}

#[test]
fn discount_projection_uses_coupon_day_count_and_telescopes() {
    let base_date = date!(2024 - 12 - 02);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.95)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("USNY calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let accrual_start = date!(2025 - 01 - 02);
    let accrual_end = date!(2025 - 04 - 02);
    let expected_accrual_year_fraction = DayCount::Act360
        .year_fraction(accrual_start, accrual_end, DayCountContext::default())
        .expect("accrual fraction");

    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Discount(&discount),
        fixings: None,
        fixing_id: "USD-SOFR",
        as_of: base_date,
        accrual_start,
        accrual_end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: None,
        coupon_period: (accrual_start, accrual_end),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("coupon projection");
    let expected = 1.0
        / discount
            .df_between_dates(accrual_start, accrual_end)
            .expect("relative discount factor");

    assert!(
        (projection.compound_factor - expected).abs() < 1.0e-12,
        "daily discount projection should telescope: {} vs {}",
        projection.compound_factor,
        expected
    );
    let expected_rate = (projection.compound_factor - 1.0) / expected_accrual_year_fraction;
    assert!(
        (projection.rate - expected_rate).abs() < 1.0e-12,
        "projector must normalize from adjusted dates: {} vs {}",
        projection.rate,
        expected_rate
    );
    assert!(
        projection.observation_exposures.is_empty(),
        "discount identity path should not emit daily observation exposures"
    );
}

fn walked_last_overnight_observation(
    start: Date,
    end: Date,
    calendar: &dyn HolidayCalendar,
) -> Date {
    let mut date = start;
    let mut last = start;
    while date < end {
        last = date;
        date = date
            .add_business_days(1, calendar)
            .expect("business day")
            .min(end);
    }
    last
}

fn assert_discount_identity_fixing_date(
    base_date: Date,
    discount: &DiscountCurve,
    calendar: &dyn HolidayCalendar,
    accrual_start: Date,
    accrual_end: Date,
) {
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let project = |need_exposures: bool| {
        project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Discount(discount),
            fixings: None,
            fixing_id: "USD-SOFR",
            as_of: base_date,
            accrual_start,
            accrual_end,
            end_is_termination_date: None,
            day_count: DayCount::Act360,
            coupon_frequency: None,
            coupon_period: (accrual_start, accrual_end),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: need_exposures,
        })
        .expect("coupon projection")
    };
    let identity = project(false);
    let walked = project(true);
    let last_observation = last_overnight_observation(accrual_start, accrual_end, calendar)
        .expect("last overnight observation");
    let business_day_before_end = accrual_end
        .add_business_days(-1, calendar)
        .expect("business day before end");
    let walked_last = walked_last_overnight_observation(accrual_start, accrual_end, calendar);

    assert!(
        (identity.compound_factor - walked.compound_factor).abs() < 1.0e-12,
        "identity CF {} vs walked CF {}",
        identity.compound_factor,
        walked.compound_factor
    );
    assert!(
        (identity.rate - walked.rate).abs() < 1.0e-12,
        "identity rate {} vs walked rate {}",
        identity.rate,
        walked.rate
    );
    assert_eq!(identity.fixing_date, walked.fixing_date);
    assert_eq!(identity.fixing_date, last_observation);
    assert_eq!(identity.fixing_date, business_day_before_end);
    assert_eq!(identity.fixing_date, walked_last);
    assert!(identity.observation_exposures.is_empty());
    assert!(!walked.observation_exposures.is_empty());
}

#[test]
fn discount_identity_matches_daily_walk_for_future_coupon() {
    let base_date = date!(2024 - 12 - 02);
    let discount = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, 0.95), (2.0, 0.90)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount curve");
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("USNY calendar");

    assert_discount_identity_fixing_date(
        base_date,
        &discount,
        calendar,
        date!(2025 - 01 - 02),
        date!(2026 - 01 - 02),
    );
    // Accrual ends on Saturday so the last observation must be the prior Friday.
    assert_discount_identity_fixing_date(
        base_date,
        &discount,
        calendar,
        date!(2025 - 01 - 03),
        date!(2025 - 01 - 11),
    );
}

#[test]
fn compounded_coupon_parallel_sensitivity_matches_finite_difference() {
    let base_date = date!(2024 - 12 - 02);
    let accrual_start = date!(2025 - 01 - 02);
    let accrual_end = date!(2025 - 04 - 02);
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test caplet")
        .expect("USNY calendar");
    let compounding = FloatingLegCompounding::CompoundedWithRateCutoff { cutoff_days: 1 };
    let build_shifted_simple_curve = |delta_rate: f64| {
        let mut pts = vec![(base_date, 1.0)];
        let mut factor = 1.0;
        let mut d = base_date;
        while d < accrual_end {
            let next = d
                .add_business_days(1, calendar)
                .expect("next bd")
                .min(accrual_end);
            let t = (d - base_date).whole_days() as f64 / 360.0;
            let r = (0.03 + 0.03 * t) + delta_rate;
            let alpha = (next - d).whole_days() as f64 / 360.0;
            factor /= 1.0 + r * alpha;
            pts.push((next, factor));
            d = next;
        }
        ForwardCurve::from_dates(
            "USD-SOFR-OIS",
            base_date,
            &pts,
            DayCount::Act360,
            finstack_quant_core::math::interp::InterpStyle::LogLinear,
            finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
        )
        .expect("forward curve")
    };
    let forward = build_shifted_simple_curve(0.0);
    let up = build_shifted_simple_curve(1e-6);
    let down = build_shifted_simple_curve(-1e-6);
    let project = |curve: &ForwardCurve| {
        project_overnight_coupon(OvernightCouponProjectionInput {
            curve: OvernightProjectionCurve::Forward(curve),
            fixings: None,
            fixing_id: "USD-SOFR-OIS",
            as_of: base_date,
            accrual_start,
            accrual_end,
            end_is_termination_date: None,
            day_count: DayCount::Act360,
            coupon_frequency: None,
            coupon_period: (accrual_start, accrual_end),
            compounding: &compounding,
            fixing_calendar: calendar,
            compounded_spread: 0.0,
            need_observation_exposures: true,
        })
        .expect("coupon projection")
    };

    let base = project(&forward);
    let finite_difference = (project(&up).rate - project(&down).rate) / (2.0e-6);
    assert!(
        (base.parallel_forward_sensitivity - finite_difference).abs() < 1.0e-8,
        "analytic product sensitivity {} should match finite difference {}",
        base.parallel_forward_sensitivity,
        finite_difference
    );
    let exposure_sum: f64 = base
        .observation_exposures
        .iter()
        .map(|exposure| exposure.coupon_forward_derivative)
        .sum();
    assert!(
        (exposure_sum - finite_difference).abs() < 1.0e-8,
        "date-specific product derivatives {exposure_sum} should sum to the parallel \
         finite difference {finite_difference}"
    );

    let second_up = project(&build_shifted_simple_curve(1e-4));
    let second_down = project(&build_shifted_simple_curve(-1e-4));
    let second_finite_difference = (second_up.rate - 2.0 * base.rate + second_down.rate) / 1.0e-8;
    assert!(
        (base.parallel_forward_second_sensitivity - second_finite_difference).abs() < 1.0e-6,
        "analytic product second sensitivity {} should match finite difference {}",
        base.parallel_forward_second_sensitivity,
        second_finite_difference
    );
}

#[test]
fn historical_observation_has_zero_stochastic_exposure() {
    let accrual_start = date!(2025 - 01 - 02);
    let as_of = date!(2025 - 01 - 03);
    let accrual_end = date!(2025 - 01 - 07);
    let forward =
        ForwardCurve::flat("USD-SOFR-OIS", as_of, 0.04, DayCount::Act360).expect("forward curve");
    let fixings = ScalarTimeSeries::new(
        "FIXING:USD-SOFR-OIS",
        vec![(accrual_start, 0.035)],
        None,
        finstack_quant_core::market_data::scalars::SeriesInterpolation::Step,
        finstack_quant_core::market_data::scalars::SeriesExtrapolation::Error,
    )
    .expect("fixings");
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("USNY calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };
    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&forward),
        fixings: Some(&fixings),
        fixing_id: "USD-SOFR-OIS",
        as_of,
        accrual_start,
        accrual_end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: None,
        coupon_period: (accrual_start, accrual_end),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: true,
    })
    .expect("projection");

    assert_eq!(
        projection.observation_exposures[0].coupon_forward_derivative,
        0.0
    );
    assert!(
        projection.observation_exposures[1].coupon_forward_derivative > 0.0,
        "same-day unpublished observation should retain stochastic exposure"
    );
}

#[test]
fn rejects_non_positive_compounding_factor() {
    let as_of = date!(2025 - 01 - 02);
    let forward = ForwardCurve::builder("BAD-OVERNIGHT")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0 / 360.0, 3.0), (1.0, 3.0)])
        .interp(finstack_quant_core::math::interp::InterpStyle::Linear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("finite pathological forward curve");
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("USNY calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 1 };

    let result = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&forward),
        fixings: None,
        fixing_id: "BAD-OVERNIGHT",
        as_of,
        accrual_start: date!(2025 - 01 - 03),
        accrual_end: date!(2025 - 01 - 06),
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: None,
        coupon_period: (date!(2025 - 01 - 03), date!(2025 - 01 - 06)),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    });

    assert!(
        result.is_err(),
        "non-positive daily compounding factor must fail closed"
    );
}

#[test]
fn rejects_non_finite_projected_rate_or_compound_product() {
    let as_of = date!(2025 - 01 - 02);
    let forward = ForwardCurve::builder("OVERFLOW-OVERNIGHT")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 1.0), (1.0 / 360.0, 1e-250)])
        .interp(finstack_quant_core::math::interp::InterpStyle::LogLinear)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("finite pathological forward curve");
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("USNY calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

    let result = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&forward),
        fixings: None,
        fixing_id: "OVERFLOW-OVERNIGHT",
        as_of,
        accrual_start: as_of,
        accrual_end: date!(2025 - 01 - 07),
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: None,
        coupon_period: (as_of, date!(2025 - 01 - 07)),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    });

    assert!(
        result.is_err(),
        "non-finite rate-derived factors or products must fail closed"
    );
}

#[test]
fn discount_projection_rejects_non_positive_or_non_finite_relative_df() {
    let base_date = date!(2025 - 01 - 02);
    let discount = DiscountCurve::builder("UNDERFLOW-DISCOUNT")
        .base_date(base_date)
        .day_count(DayCount::Act365F)
        .knots([(0.0, 1.0), (1.0, f64::MIN_POSITIVE)])
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("positive finite input discount factors");

    let end_date = date!(2027 - 01 - 04);
    let yf = DayCount::Act360
        .year_fraction(base_date, end_date, DayCountContext::default())
        .expect("yf");
    let result =
        OvernightProjectionCurve::Discount(&discount).projected_rate(base_date, end_date, yf);

    assert!(
        result.is_err(),
        "underflowed relative discount factors must fail closed"
    );
}

#[test]
fn projector_uses_fixing_calendar_for_business_252_accrual() {
    let as_of = date!(2025 - 01 - 02);
    let forward =
        ForwardCurve::flat("BRL-OVERNIGHT", as_of, 0.10, DayCount::Act360).expect("forward curve");
    let _ = DayCount::Bus252;
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&forward),
        fixings: None,
        fixing_id: "BRL-OVERNIGHT",
        as_of,
        accrual_start: as_of,
        accrual_end: date!(2025 - 01 - 10),
        end_is_termination_date: None,
        day_count: DayCount::Bus252,
        coupon_frequency: None,
        coupon_period: (as_of, date!(2025 - 01 - 10)),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("business/252 projection");
    let expected = DayCount::Bus252
        .year_fraction(
            as_of,
            date!(2025 - 01 - 10),
            DayCountContext {
                calendar: Some(calendar),
                ..DayCountContext::default()
            },
        )
        .expect("business/252 accrual");

    assert_eq!(projection.accrual_year_fraction, expected);
}

#[test]
fn projector_uses_coupon_frequency_for_act_act_isma_accrual() {
    let as_of = date!(2025 - 01 - 01);
    let forward = ForwardCurve::flat("ICMA-OVERNIGHT", as_of, 0.03, DayCount::Act365F)
        .expect("forward curve");
    let calendar = resolve_overnight_fixing_calendar(Some("usny"), Currency::USD, "test coupon")
        .expect("calendar");
    let compounding = FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 };

    let projection = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&forward),
        fixings: None,
        fixing_id: "ICMA-OVERNIGHT",
        as_of,
        accrual_start: as_of,
        accrual_end: date!(2025 - 07 - 01),
        end_is_termination_date: None,
        day_count: DayCount::ActActIsma,
        coupon_frequency: Some(Tenor::semi_annual()),
        coupon_period: (as_of, date!(2025 - 07 - 01)),
        compounding: &compounding,
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("Act/Act ISMA projection");

    assert_eq!(projection.accrual_year_fraction, 0.5);
}
