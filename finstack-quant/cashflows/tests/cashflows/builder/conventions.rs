//! Convention tests: cross-currency error handling and day count validation.
//!
//! Covers:
//! - Cross-currency aggregation rejection
//! - Same-currency aggregation success
//! - Currency preservation through schedule building
//! - Bus/252 day count year fraction with calendar context

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use time::Month;

/// Helper to construct dates concisely.
fn d(year: i32, month: u8, day: u8) -> Date {
    let m = Month::try_from(month).expect("valid month");
    Date::from_calendar_date(year, m, day).expect("valid date")
}

// Cross-Currency Aggregation Tests

/// Aggregating flows in different currencies should error.
#[test]
fn test_cross_currency_aggregation_error() {
    use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;

    let flows = vec![
        (
            d(2024, 6, 15),
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
        ),
        (
            d(2024, 9, 15),
            Money::new(100.0, Currency::EUR).expect("valid money fixture"),
        ),
    ];

    let result = aggregate_cashflows_checked(&flows, Currency::USD);
    assert!(result.is_err(), "Cross-currency aggregation should fail");
}

/// Aggregating flows in the same currency should succeed.
#[test]
fn test_single_currency_aggregation() {
    use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;

    let flows = vec![
        (
            d(2024, 6, 15),
            Money::new(100.0, Currency::USD).expect("valid money fixture"),
        ),
        (
            d(2024, 9, 15),
            Money::new(200.0, Currency::USD).expect("valid money fixture"),
        ),
    ];

    let result = aggregate_cashflows_checked(&flows, Currency::USD)
        .expect("same-currency aggregation should succeed");
    assert!(
        (result.amount() - 300.0).abs() < 1e-10,
        "Aggregated amount should be 300.0, got {}",
        result.amount()
    );
}

// Currency Preservation Tests

/// Build a USD bond and verify every flow is USD-denominated.
#[test]
fn test_all_flows_preserve_currency() {
    use finstack_quant_cashflows::builder::specs::CouponType;
    use finstack_quant_cashflows::builder::specs::FixedCouponSpec;
    use finstack_quant_cashflows::builder::CashFlowSchedule;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use rust_decimal::Decimal;

    let issue = d(2024, 1, 15);
    let maturity = d(2029, 1, 15);
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let fixed = FixedCouponSpec {
        rate: Decimal::try_from(0.05).expect("valid"), // 5%
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),

            day_count: DayCount::Thirty360,

            business_day_convention: BusinessDayConvention::ModifiedFollowing,

            calendar_id: "weekends_only".into(),

            stub: StubKind::ShortFront,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(notional, issue, maturity).fixed_cf(fixed);
    let schedule = builder.build(None).expect("build should succeed");

    for flow in schedule.get_flows() {
        assert_eq!(
            flow.amount.currency(),
            Currency::USD,
            "All flows should be USD, but found {:?} on {:?} flow",
            flow.amount.currency(),
            flow.kind
        );
    }
}

// Bus/252 Day Count Validation

/// Bus/252 year fraction for a known date range using TARGET2 calendar.
#[test]
fn test_bus_252_year_fraction() {
    use finstack_quant_core::dates::calendar::TARGET2;
    use finstack_quant_core::dates::{DayCount, DayCountContext};

    let start = d(2024, 1, 2); // First business day of 2024
    let end = d(2024, 7, 1);

    let calendar = TARGET2;
    let ctx = DayCountContext {
        calendar: Some(&calendar),
        frequency: None,
        bus_basis: None,
        coupon_period: None,
        end_is_termination_date: false,
    };

    let yf = DayCount::Bus252
        .year_fraction(start, end, ctx)
        .expect("Bus/252 should work with TARGET2 calendar");

    // Business days / 252 should be in reasonable range for ~6 months
    assert!(
        yf > 0.4 && yf < 0.6,
        "6-month Bus/252 YF should be ~0.5, got {}",
        yf
    );
}

/// Bus/252 without a calendar should error.
#[test]
fn test_bus_252_requires_calendar() {
    use finstack_quant_core::dates::{DayCount, DayCountContext};

    let start = d(2024, 1, 2);
    let end = d(2024, 7, 1);

    let result = DayCount::Bus252.year_fraction(start, end, DayCountContext::default());
    assert!(
        result.is_err(),
        "Bus/252 should error without a calendar in DayCountContext"
    );
}

#[test]
fn contractual_accrual_boundaries_are_not_business_day_adjusted() {
    use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

    let periods = build_periods(BuildPeriodsParams {
        start: d(2024, 8, 31),
        end: d(2025, 8, 31),
        frequency: Tenor::annual(),
        stub: StubKind::None,
        business_day_convention: BusinessDayConvention::Following,
        calendar_id: "weekends_only",
        end_of_month: false,
        day_count: DayCount::Act365F,
        payment_lag_days: 0,
        reset_lag_days: None,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
    })
    .expect("schedule should build");

    assert_eq!(periods.len(), 1);
    let period = periods[0];
    assert_eq!(period.accrual_start, d(2024, 8, 31));
    assert_eq!(period.accrual_end, d(2025, 8, 31));
    assert_eq!(period.payment_date, d(2025, 9, 1));
    assert_eq!(
        periods
            .iter()
            .map(|period| period.payment_date)
            .collect::<Vec<_>>(),
        vec![d(2025, 9, 1)]
    );
}

/// SOFR swap preset (ISDA 2006 §4.10 / ARRC conventions): accrual boundaries
/// are business-day adjusted, so a weekend-spanning period end accrues to the
/// rolled date and the day-count fraction reflects the adjusted boundaries.
#[test]
fn sofr_swap_preset_adjusts_accrual_boundaries() {
    use finstack_quant_cashflows::builder::{
        CashFlowSchedule, CouponType, ScheduleParams, StepUpCouponSpec,
    };
    use finstack_quant_core::cashflow::CFKind;
    use rust_decimal_macros::dec;

    // 2025-03-06 (Thu) -> 2025-09-06 (Sat): the second quarterly accrual end
    // falls on a Saturday and rolls to Monday 2025-09-08 under MF/usny.
    let issue = d(2025, 3, 6);
    let maturity = d(2025, 9, 6);
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let build = |params: ScheduleParams| {
        let mut b = CashFlowSchedule::builder();
        let _ = b
            .principal(notional, issue, maturity)
            .step_up_cf(StepUpCouponSpec {
                coupon_type: CouponType::Cash,
                initial_rate: dec!(0.04),
                step_schedule: Vec::new(),
                schedule: params,
            });
        b.build(None).expect("schedule builds")
    };

    let swap = build(ScheduleParams::usd_sofr_swap());
    let mut bond_style = ScheduleParams::usd_sofr_swap();
    bond_style.adjust_accrual_dates = false;
    let bond = build(bond_style);

    let last_coupon_yf = |s: &CashFlowSchedule| {
        s.get_flows()
            .iter()
            .rfind(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
            .expect("coupon present")
            .accrual_factor
    };

    // Adjusted accrual: [2025-06-06, 2025-09-08) = 94 days on Act/360.
    let swap_yf = last_coupon_yf(&swap);
    assert!(
        (swap_yf - 94.0 / 360.0).abs() < 1e-12,
        "swap preset must accrue to the adjusted boundary: got {swap_yf}, want {}",
        94.0 / 360.0
    );

    // Unadjusted accrual: [2025-06-06, 2025-09-06) = 92 days on Act/360.
    let bond_yf = last_coupon_yf(&bond);
    assert!(
        (bond_yf - 92.0 / 360.0).abs() < 1e-12,
        "unadjusted accrual must use the raw boundary: got {bond_yf}, want {}",
        92.0 / 360.0
    );
}

#[test]
fn preceding_adjusted_february_termination_matches_coupon_emission() {
    use finstack_quant_cashflows::builder::periods::{
        build_periods, build_single_period, period_accrual, BuildPeriodsParams,
    };
    use finstack_quant_cashflows::builder::{
        CashFlowSchedule, CouponType, FixedCouponSpec, ScheduleParams,
    };
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind};

    let issue = d(2024, 9, 1);
    let maturity = d(2025, 3, 1);
    let mut schedule = ScheduleParams::semiannual_30360();
    schedule.day_count = DayCount::ThirtyE360Isda;
    schedule.stub = StubKind::None;
    schedule.business_day_convention = BusinessDayConvention::Preceding;
    schedule.adjust_accrual_dates = true;
    let params = BuildPeriodsParams::from_schedule(&schedule, issue, maturity, None);
    let periods = build_periods(params).expect("periods");
    let single = build_single_period(params).expect("single period");
    assert_eq!(periods.len(), 1);
    assert_eq!(single.accrual_start, d(2024, 8, 30));
    assert_eq!(single.accrual_end, d(2025, 2, 28));
    let expected = 178.0 / 360.0;
    assert!((single.accrual_year_fraction - expected).abs() < 1e-12);
    assert_eq!(
        periods[0].accrual_year_fraction,
        single.accrual_year_fraction
    );
    // A partial interval must not inherit the final February exception.
    let partial = period_accrual(&single, single.accrual_start, d(2025, 1, 31), &params)
        .expect("partial accrual");
    assert!((partial - 150.0 / 360.0).abs() < 1e-12);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("money"),
            issue,
            maturity,
        )
        .fixed_cf(FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: rust_decimal::Decimal::new(5, 2),
            schedule,
        });
    let built = builder.build(None).expect("cashflows");
    let coupon = built
        .get_flows()
        .iter()
        .find(|flow| flow.accrual.is_some())
        .expect("coupon");
    assert_eq!(coupon.accrual_factor, single.accrual_year_fraction);
    assert!((coupon.amount.amount() - 1_000_000.0 * 0.05 * expected).abs() < 1e-8);
}

#[test]
fn public_period_builders_reject_negative_lags_without_panicking() {
    use finstack_quant_cashflows::builder::periods::{
        build_periods, build_single_period, BuildPeriodsParams,
    };
    use finstack_quant_cashflows::builder::ScheduleParams;

    let schedule = ScheduleParams::quarterly_act360();
    let base = BuildPeriodsParams::from_schedule(&schedule, d(2025, 1, 15), d(2025, 4, 15), None);
    for lag in [-2, i32::MIN] {
        let reset = BuildPeriodsParams {
            reset_lag_days: Some(lag),
            ..base
        };
        assert!(build_periods(reset).is_err());
        assert!(build_single_period(reset).is_err());
        let payment = BuildPeriodsParams {
            payment_lag_days: lag,
            ..base
        };
        assert!(build_periods(payment).is_err());
        assert!(build_single_period(payment).is_err());
    }
    for end in [base.start, d(2024, 12, 15)] {
        let invalid = BuildPeriodsParams { end, ..base };
        assert!(build_periods(invalid).is_err());
        assert!(build_single_period(invalid).is_err());
    }
}

#[test]
fn icma_eom_requires_a_month_end_reference_anchor() {
    use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
    use finstack_quant_cashflows::builder::ScheduleParams;
    use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};

    let mut schedule = ScheduleParams::quarterly_act360();
    schedule.frequency = Tenor::monthly();
    schedule.day_count = DayCount::ActActIsma;
    schedule.business_day_convention = BusinessDayConvention::Unadjusted;
    schedule.end_of_month = true;
    schedule.stub = StubKind::ShortBack;
    let params = BuildPeriodsParams::from_schedule(&schedule, d(2025, 1, 15), d(2025, 5, 15), None);
    let error = build_periods(params).expect_err("ambiguous EOM grid");
    assert!(error.to_string().contains("regular grid anchor"));

    schedule.stub = StubKind::ShortFront;
    let params = BuildPeriodsParams::from_schedule(&schedule, d(2025, 1, 15), d(2025, 5, 31), None);
    let periods = build_periods(params).expect("month-end anchored ICMA front stub");
    assert_eq!(
        periods.first().expect("front stub").accrual_start,
        d(2025, 1, 15)
    );
    assert_eq!(
        periods.last().expect("last coupon").accrual_end,
        d(2025, 5, 31)
    );
    for period in &periods[1..] {
        assert!((period.accrual_year_fraction - 1.0 / 12.0).abs() < 1e-12);
    }
}
