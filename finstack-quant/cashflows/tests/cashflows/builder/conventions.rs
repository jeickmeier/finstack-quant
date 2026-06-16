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

// =============================================================================
// Cross-Currency Aggregation Tests
// =============================================================================

/// Aggregating flows in different currencies should error.
#[test]
fn test_cross_currency_aggregation_error() {
    use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;

    let flows = vec![
        (d(2024, 6, 15), Money::new(100.0, Currency::USD)),
        (d(2024, 9, 15), Money::new(100.0, Currency::EUR)),
    ];

    let result = aggregate_cashflows_checked(&flows, Currency::USD);
    assert!(result.is_err(), "Cross-currency aggregation should fail");
}

/// Aggregating flows in the same currency should succeed.
#[test]
fn test_single_currency_aggregation() {
    use finstack_quant_cashflows::aggregation::aggregate_cashflows_checked;

    let flows = vec![
        (d(2024, 6, 15), Money::new(100.0, Currency::USD)),
        (d(2024, 9, 15), Money::new(200.0, Currency::USD)),
    ];

    let result = aggregate_cashflows_checked(&flows, Currency::USD)
        .expect("same-currency aggregation should succeed");
    assert!(
        (result.amount() - 300.0).abs() < 1e-10,
        "Aggregated amount should be 300.0, got {}",
        result.amount()
    );
}

// =============================================================================
// Currency Preservation Tests
// =============================================================================

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
    let notional = Money::new(1_000_000.0, Currency::USD);

    let fixed = FixedCouponSpec {
        rate: Decimal::try_from(0.05).expect("valid"), // 5%
        coupon_type: CouponType::Cash,
        freq: Tenor::semi_annual(),
        dc: DayCount::Thirty360,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::ShortFront,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(notional, issue, maturity).fixed_cf(fixed);
    let schedule = builder
        .build_with_curves(None)
        .expect("build should succeed");

    for flow in &schedule.flows {
        assert_eq!(
            flow.amount.currency(),
            Currency::USD,
            "All flows should be USD, but found {:?} on {:?} flow",
            flow.amount.currency(),
            flow.kind
        );
    }
}

// =============================================================================
// Bus/252 Day Count Validation
// =============================================================================

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
    use finstack_quant_cashflows::builder::date_generation::build_dates;
    use finstack_quant_core::dates::{BusinessDayConvention, StubKind, Tenor};

    let schedule = build_dates(
        d(2024, 8, 31),
        d(2025, 8, 31),
        Tenor::annual(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )
    .expect("schedule should build");

    assert_eq!(schedule.periods.len(), 1);
    let period = schedule.periods[0];
    assert_eq!(period.accrual_start, d(2024, 8, 31));
    assert_eq!(period.accrual_end, d(2025, 8, 31));
    assert_eq!(period.payment_date, d(2025, 9, 1));
    assert_eq!(schedule.dates, vec![d(2025, 9, 1)]);
}

/// SOFR swap preset (ISDA 2006 §4.10 / ARRC conventions): accrual boundaries
/// are business-day adjusted, so a weekend-spanning period end accrues to the
/// rolled date and the day-count fraction reflects the adjusted boundaries.
#[test]
fn sofr_swap_preset_adjusts_accrual_boundaries() {
    use finstack_quant_cashflows::builder::{CashFlowSchedule, CouponType, ScheduleParams};
    use finstack_quant_core::cashflow::CFKind;
    use rust_decimal_macros::dec;

    // 2025-03-06 (Thu) -> 2025-09-06 (Sat): the second quarterly accrual end
    // falls on a Saturday and rolls to Monday 2025-09-08 under MF/usny.
    let issue = d(2025, 3, 6);
    let maturity = d(2025, 9, 6);
    let notional = Money::new(1_000_000.0, Currency::USD);

    let build = |params: ScheduleParams| {
        let mut b = CashFlowSchedule::builder();
        let _ = b.principal(notional, issue, maturity).fixed_stepup_decimal(
            &[(maturity, dec!(0.04))],
            params,
            CouponType::Cash,
        );
        b.build_with_curves(None).expect("schedule builds")
    };

    let swap = build(ScheduleParams::usd_sofr_swap());
    let mut bond_style = ScheduleParams::usd_sofr_swap();
    bond_style.adjust_accrual_dates = false;
    let bond = build(bond_style);

    let last_coupon_yf = |s: &CashFlowSchedule| {
        s.flows
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
