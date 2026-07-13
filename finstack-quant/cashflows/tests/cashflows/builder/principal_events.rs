//! Tests for principal event validation in cashflow schedules.
//!
//! These tests verify boundary conditions and validation rules for principal events,
//! including date constraints relative to issue and maturity dates.
//!
//! # Coverage
//!
//! - Date boundary validation (before issue, at issue, at maturity, after maturity)
//! - Currency mismatch detection
//! - Multiple events on same date
//! - Draw vs repay semantics
//! - Outstanding balance constraints

use finstack_quant_cashflows::builder::{
    AmortizationSpec, CashFlowBuilder, CashFlowSchedule, CouponType, FeeSpec, FixedCouponSpec,
    FixedWindow, FloatingCouponSpec, FloatingRateFallback, FloatingRateSpec,
    OvernightIndexConstraintApplication, PrincipalEvent, ScheduleParams, StepUpCouponSpec,
};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use time::Month;

use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};

// =============================================================================
// Principal Event Date Validation
// =============================================================================

#[test]
fn principal_events_after_maturity_rejected() {
    // Principal events after maturity should be rejected to prevent
    // post-maturity flows after outstanding has been zeroed out.

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let post_maturity = Date::from_calendar_date(2026, Month::February, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Event after maturity should cause build to fail
    let event = PrincipalEvent {
        date: post_maturity,
        delta: Money::new(100_000.0, Currency::USD), // Draw
        cash: Money::new(100_000.0, Currency::USD),
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_err(),
        "Build should fail when principal event is after maturity"
    );

    // Error should indicate date is out of range
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("outside") || err_msg.contains("range"),
        "Error message should mention date is outside allowed range: {}",
        err_msg
    );
}

#[test]
fn principal_events_at_maturity_accepted() {
    // Principal events exactly at maturity should be allowed
    // (e.g., final draw for a bullet redemption structure)

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Event exactly at maturity should be allowed
    let event = PrincipalEvent {
        date: maturity,
        delta: Money::new(500_000.0, Currency::USD), // Partial repay at maturity
        cash: Money::new(500_000.0, Currency::USD),
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_ok(),
        "Build should succeed when principal event is exactly at maturity"
    );
}

// =============================================================================
// Principal Event Before Issue Date
// =============================================================================

#[test]
fn principal_events_before_issue_included_and_adjusts_outstanding() {
    // Principal events before issue date are included and should adjust
    // the initial outstanding balance (e.g., delayed funding structures).

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let pre_issue = Date::from_calendar_date(2024, Month::December, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Draw 100k before issue (positive delta increases outstanding)
    let event = PrincipalEvent {
        date: pre_issue,
        delta: Money::new(100_000.0, Currency::USD),
        cash: Money::new(100_000.0, Currency::USD),
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let schedule = builder.build_with_curves(None).unwrap();

    // Pre-issue event should appear in flows
    assert!(
        schedule.flows.iter().any(|cf| cf.date == pre_issue),
        "Pre-issue event should appear in flows"
    );

    // Outstanding at issue should include the pre-issue draw
    let outstanding = schedule.outstanding_by_date().unwrap();
    let issue_outstanding = outstanding
        .iter()
        .find(|(d, _)| *d == issue)
        .map(|(_, m)| m.amount())
        .unwrap();
    assert!(
        (issue_outstanding - (init.amount() + 100_000.0)).abs() < 0.01,
        "Outstanding at issue should include pre-issue draw: expected {}, got {}",
        init.amount() + 100_000.0,
        issue_outstanding
    );
}

#[test]
fn principal_events_at_issue_accepted() {
    // Principal events exactly at issue date should be allowed
    // (e.g., partial funding at closing)

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Additional draw at issue (delayed draw term loan pattern)
    let event = PrincipalEvent {
        date: issue,
        delta: Money::new(500_000.0, Currency::USD), // Additional draw
        cash: Money::new(500_000.0, Currency::USD),
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_ok(),
        "Build should succeed when principal event is at issue date"
    );
}

// =============================================================================
// Currency Mismatch Validation
// =============================================================================

#[test]
fn principal_events_currency_mismatch_rejected() {
    // Principal events with different currency than notional should be rejected
    // to avoid cross-currency outstanding tracking.

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Event in EUR when notional is USD
    let event = PrincipalEvent {
        date: mid_date,
        delta: Money::new(100_000.0, Currency::EUR), // Wrong currency
        cash: Money::new(100_000.0, Currency::EUR),
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_err(),
        "Build should fail when principal event currency differs from notional"
    );
}

#[test]
fn principal_event_delta_cash_currency_mismatch_rejected() {
    // Delta/cash currency mismatch should be rejected at the builder layer.
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let event = PrincipalEvent {
        date: mid_date,
        delta: Money::new(100_000.0, Currency::USD),
        cash: Money::new(100_000.0, Currency::EUR), // Mismatch
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_err(),
        "Build should fail when principal event delta/cash currencies differ"
    );
}

// =============================================================================
// Multiple Events on Same Date
// =============================================================================

#[test]
fn multiple_principal_events_same_date_accepted() {
    // Multiple principal events on the same date should be processed
    // (e.g., draw and partial repay on same day for restructuring)

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Two events on same date
    let events = [
        PrincipalEvent {
            date: mid_date,
            delta: Money::new(200_000.0, Currency::USD), // Draw
            cash: Money::new(200_000.0, Currency::USD),
            kind: CFKind::Notional,
        },
        PrincipalEvent {
            date: mid_date,
            delta: Money::new(-100_000.0, Currency::USD), // Repay
            cash: Money::new(100_000.0, Currency::USD),
            kind: CFKind::Amortization,
        },
    ];

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            events[0].date,
            events[0].delta,
            Some(events[0].cash),
            events[0].kind,
        )
        .add_principal_event(
            events[1].date,
            events[1].delta,
            Some(events[1].cash),
            events[1].kind,
        );

    let result = builder.build_with_curves(None);
    assert!(
        result.is_ok(),
        "Build should succeed with multiple events on same date"
    );

    // Verify net effect: draw 200k, repay 100k => +100k outstanding
    let schedule = result.unwrap();
    let principal_flows_on_mid: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| {
            cf.date == mid_date && (cf.kind == CFKind::Notional || cf.kind == CFKind::Amortization)
        })
        .collect();

    // Should have both events as separate flows
    assert!(
        principal_flows_on_mid.len() >= 2,
        "Should have at least 2 principal flows on mid_date"
    );

    // Net outstanding change should be +100k (draw 200k, repay 100k)
    let outstanding = schedule.outstanding_by_date().unwrap();
    let mid_outstanding = outstanding
        .iter()
        .find(|(d, _)| *d == mid_date)
        .map(|(_, m)| m.amount())
        .unwrap();
    assert!(
        (mid_outstanding - (init.amount() + 100_000.0)).abs() < 0.01,
        "Outstanding after same-day events should be {}, got {}",
        init.amount() + 100_000.0,
        mid_outstanding
    );
}

// =============================================================================
// Draw and Repay Semantics
// =============================================================================

#[test]
fn principal_event_draw_increases_outstanding() {
    // A draw should increase outstanding balance
    //
    // Note: The sign convention for draws may vary by implementation.
    // Negative delta typically means cash outflow (draw), positive means inflow (repay).
    // Check actual behavior and document.

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Draw 500k more (positive delta increases outstanding)
    let event = PrincipalEvent {
        date: mid_date,
        delta: Money::new(500_000.0, Currency::USD),
        cash: Money::new(500_000.0, Currency::USD),
        kind: CFKind::Notional,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, Some(event.cash), event.kind);

    let schedule = builder.build_with_curves(None).unwrap();
    let outstanding = schedule.outstanding_by_date().unwrap();

    // Find outstanding at mid_date
    let mid_outstanding = outstanding
        .iter()
        .find(|(d, _)| *d == mid_date)
        .map(|(_, m)| m.amount())
        .unwrap();

    assert!(
        (mid_outstanding - (init.amount() + 500_000.0)).abs() < 0.01,
        "Outstanding should increase after draw: expected {}, got {}",
        init.amount() + 500_000.0,
        mid_outstanding
    );
}

#[test]
fn principal_event_repay_effect_on_outstanding() {
    // Test that principal events are included in the outstanding path
    //
    // Note: The sign convention and exact semantics depend on implementation.
    // This test verifies events are processed, not specific values.

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    // Add a repayment event (Amortization kind; cash defaults to -delta)
    let event = PrincipalEvent {
        date: mid_date,
        delta: Money::new(-300_000.0, Currency::USD),
        cash: Money::new(300_000.0, Currency::USD),
        kind: CFKind::Amortization,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(event.date, event.delta, None, event.kind);

    let schedule = builder.build_with_curves(None).unwrap();

    // Verify the event was added to flows
    let notional_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional)
        .collect();

    // Should have at least 2 notional flows: initial funding + our event (+ maturity redemption)
    assert!(
        notional_flows.len() >= 2,
        "Should have at least 2 notional flows, got {}",
        notional_flows.len()
    );

    // Verify a repayment flow exists at mid_date with positive cash (defaulted to -delta)
    let mid_flows: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.date == mid_date && cf.kind == CFKind::Amortization)
        .collect();

    assert!(
        !mid_flows.is_empty(),
        "Should have an amortization flow at mid_date"
    );
    assert_eq!(
        mid_flows[0].amount.amount(),
        300_000.0,
        "cash: None should default to -delta for Amortization events"
    );

    // Outstanding should decrease by 300k
    let outstanding = schedule.outstanding_by_date().unwrap();
    let mid_outstanding = outstanding
        .iter()
        .find(|(d, _)| *d == mid_date)
        .map(|(_, m)| m.amount())
        .unwrap();
    assert!(
        (mid_outstanding - (init.amount() - 300_000.0)).abs() < 0.01,
        "Outstanding should decrease after repayment: expected {}, got {}",
        init.amount() - 300_000.0,
        mid_outstanding
    );
}

#[test]
fn principal_event_emitted_cashflow_sign_follows_kind() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let draw_date = Date::from_calendar_date(2025, Month::April, 15).unwrap();
    let repay_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            draw_date,
            Money::new(100_000.0, Currency::USD),
            Some(Money::new(100_000.0, Currency::USD)),
            CFKind::Notional,
        )
        .add_principal_event(
            repay_date,
            Money::new(-50_000.0, Currency::USD),
            Some(Money::new(50_000.0, Currency::USD)),
            CFKind::Amortization,
        );

    let schedule = builder.build_with_curves(None).unwrap();
    let draw_flow = schedule
        .flows
        .iter()
        .find(|cf| cf.date == draw_date && cf.kind == CFKind::Notional)
        .expect("draw flow emitted");
    let repay_flow = schedule
        .flows
        .iter()
        .find(|cf| cf.date == repay_date && cf.kind == CFKind::Amortization)
        .expect("repayment flow emitted");

    assert_eq!(draw_flow.amount.amount(), -100_000.0);
    assert_eq!(repay_flow.amount.amount(), 50_000.0);

    let outstanding = schedule.outstanding_by_date().unwrap();
    let post_repay = outstanding
        .iter()
        .find(|(d, _)| *d == repay_date)
        .map(|(_, m)| m.amount())
        .unwrap();
    assert!((post_repay - 1_050_000.0).abs() < 0.01);
}

// =============================================================================
// Empty Events List
// =============================================================================

#[test]
fn empty_principal_events_accepted() {
    // Empty events list should be valid (no ad-hoc principal changes)

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_ok(),
        "Build should succeed with empty events list"
    );
}

// =============================================================================
// Pre-Issue / At-Issue Emission Exclusivity (M2)
// =============================================================================

/// Count flows matching a date and kind.
fn count_flows(schedule: &CashFlowSchedule, date: Date, kind: CFKind) -> usize {
    schedule
        .flows
        .iter()
        .filter(|cf| cf.date == date && cf.kind == kind)
        .count()
}

#[test]
fn pre_issue_and_at_issue_events_emit_once_each() {
    // With a pre-issue event present, the issue date enters the date loop's
    // input set; issue-dated events and funding must still be emitted exactly
    // once and outstanding must absorb each delta exactly once.

    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let pre_issue = Date::from_calendar_date(2024, Month::December, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            pre_issue,
            Money::new(100_000.0, Currency::USD),
            None,
            CFKind::Notional,
        )
        .add_principal_event(
            issue,
            Money::new(200_000.0, Currency::USD),
            None,
            CFKind::Notional,
        );

    let schedule = builder.build_with_curves(None).unwrap();

    assert_eq!(
        count_flows(&schedule, pre_issue, CFKind::Notional),
        1,
        "pre-issue event must be emitted exactly once"
    );
    // Issue date carries the initial funding flow plus the at-issue event.
    assert_eq!(
        count_flows(&schedule, issue, CFKind::Notional),
        2,
        "issue date must carry funding + at-issue event exactly once each"
    );

    // Outstanding at issue: 1,000,000 + 100,000 + 200,000 (each delta once).
    let outstanding = schedule.outstanding_by_date().unwrap();
    let issue_outstanding = outstanding
        .iter()
        .find(|(d, _)| *d == issue)
        .map(|(_, m)| m.amount())
        .unwrap();
    assert!(
        (issue_outstanding - 1_300_000.0).abs() < 0.01,
        "outstanding at issue must absorb each delta exactly once: got {issue_outstanding}"
    );

    // Redemption at maturity repays the full outstanding exactly once.
    let redemption: f64 = schedule
        .flows
        .iter()
        .filter(|cf| cf.date >= maturity && cf.kind == CFKind::Notional)
        .map(|cf| cf.amount.amount())
        .sum();
    assert!(
        (redemption - 1_300_000.0).abs() < 0.01,
        "redemption must repay outstanding once: got {redemption}"
    );
}

#[test]
fn two_pre_issue_events_on_different_dates_emit_once_each() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let pre_a = Date::from_calendar_date(2024, Month::November, 15).unwrap();
    let pre_b = Date::from_calendar_date(2024, Month::December, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            pre_a,
            Money::new(50_000.0, Currency::USD),
            None,
            CFKind::Notional,
        )
        .add_principal_event(
            pre_b,
            Money::new(75_000.0, Currency::USD),
            None,
            CFKind::Notional,
        );

    let schedule = builder.build_with_curves(None).unwrap();

    assert_eq!(count_flows(&schedule, pre_a, CFKind::Notional), 1);
    assert_eq!(count_flows(&schedule, pre_b, CFKind::Notional), 1);

    let outstanding = schedule.outstanding_by_date().unwrap();
    let issue_outstanding = outstanding
        .iter()
        .find(|(d, _)| *d == issue)
        .map(|(_, m)| m.amount())
        .unwrap();
    assert!(
        (issue_outstanding - 1_125_000.0).abs() < 0.01,
        "outstanding at issue must include both pre-issue draws once: got {issue_outstanding}"
    );
}

#[test]
fn pre_issue_event_with_issue_dated_fixed_fee_emits_fee_once() {
    // Regression for M2: a pre-issue event used to push the issue date into
    // the loop, double-emitting issue-dated fixed fees.
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let pre_issue = Date::from_calendar_date(2024, Month::December, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            pre_issue,
            Money::new(100_000.0, Currency::USD),
            None,
            CFKind::Notional,
        )
        .fee(FeeSpec::Fixed {
            date: issue,
            amount: Money::new(5_000.0, Currency::USD),
        });

    let schedule = builder.build_with_curves(None).unwrap();

    assert_eq!(
        count_flows(&schedule, issue, CFKind::Fee),
        1,
        "issue-dated fixed fee must be emitted exactly once"
    );
}

#[test]
fn fixed_fee_before_issue_emitted_once_at_its_date() {
    // Policy: fixed fees dated strictly before issue are emitted during
    // initialization on their stated date (delayed-funding upfront fees),
    // not silently dropped.
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let fee_date = Date::from_calendar_date(2024, Month::December, 20).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .fee(FeeSpec::Fixed {
            date: fee_date,
            amount: Money::new(2_500.0, Currency::USD),
        });

    let schedule = builder.build_with_curves(None).unwrap();

    assert_eq!(
        count_flows(&schedule, fee_date, CFKind::Fee),
        1,
        "pre-issue fixed fee must be emitted exactly once on its date"
    );
    let fee = schedule
        .flows
        .iter()
        .find(|cf| cf.date == fee_date && cf.kind == CFKind::Fee)
        .unwrap();
    assert_eq!(fee.amount.amount(), 2_500.0);
}

// =============================================================================
// Maturity Redemption Date Adjustment (M9)
// =============================================================================

#[test]
fn weekend_maturity_redemption_matches_final_coupon_date() {
    // Maturity 2026-01-17 is a Saturday. With ModifiedFollowing on a
    // weekends-only calendar, the final coupon pays Monday 2026-01-19; the
    // principal redemption must land on the same adjusted date.
    let issue = Date::from_calendar_date(2025, Month::January, 17).unwrap(); // Friday
    let maturity = Date::from_calendar_date(2026, Month::January, 17).unwrap(); // Saturday
    let adjusted = Date::from_calendar_date(2026, Month::January, 19).unwrap(); // Monday

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::semi_annual(),

            dc: DayCount::Act360,

            bdc: BusinessDayConvention::ModifiedFollowing,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD);
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).fixed_cf(fixed);

    let schedule = builder.build_with_curves(None).unwrap();

    let redemption = schedule
        .flows
        .iter()
        .find(|cf| cf.kind == CFKind::Notional && cf.amount.amount() > 0.0)
        .expect("redemption flow emitted");
    assert_eq!(
        redemption.date, adjusted,
        "redemption must pay on the business-day-adjusted maturity"
    );

    let final_coupon_date = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind != CFKind::Notional && cf.amount.amount() > 0.0)
        .map(|cf| cf.date)
        .max()
        .expect("coupon flows emitted");
    assert_eq!(
        redemption.date, final_coupon_date,
        "redemption and final coupon must share the same payment date"
    );

    // No flow may remain on the raw (non-business-day) maturity date.
    assert!(
        schedule.flows.iter().all(|cf| cf.date != maturity),
        "no flow may be dated on the unadjusted weekend maturity"
    );
}

// =============================================================================
// Principal Event Sign Conventions
// =============================================================================

#[test]
fn amortization_event_with_positive_delta_rejected() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            mid_date,
            Money::new(100_000.0, Currency::USD), // wrong sign for a repayment
            None,
            CFKind::Amortization,
        );

    let result = builder.build_with_curves(None);
    assert!(result.is_err(), "Amortization events require delta <= 0");
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("delta <= 0"),
        "error should describe the sign convention: {msg}"
    );
}

#[test]
fn notional_event_with_negative_delta_rejected() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mid_date = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let init = Money::new(1_000_000.0, Currency::USD);
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .add_principal_event(
            mid_date,
            Money::new(-100_000.0, Currency::USD), // wrong sign for a draw
            None,
            CFKind::Notional,
        );

    let result = builder.build_with_curves(None);
    assert!(result.is_err(), "Notional events require delta >= 0");
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("delta >= 0"),
        "error should describe the sign convention: {msg}"
    );
}

fn order_independence_fixed_spec() -> FixedCouponSpec {
    FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: dec!(0.05),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    }
}

fn order_independence_float_spec() -> FloatingCouponSpec {
    FloatingCouponSpec {
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: dec!(200),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_freq: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        },
        coupon_type: CouponType::Cash,
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    }
}

fn assert_program_order_independent<F>(
    principal: Money,
    issue: Date,
    maturity: Date,
    mut configure: F,
) where
    F: FnMut(&mut CashFlowBuilder),
{
    let mut first = CashFlowSchedule::builder();
    let _ = first.principal(principal, issue, maturity);
    configure(&mut first);

    let mut second = CashFlowSchedule::builder();
    configure(&mut second);
    let _ = second.principal(principal, issue, maturity);

    assert_eq!(
        first.build_with_curves(None).unwrap().flows,
        second.build_with_curves(None).unwrap().flows
    );
}

#[test]
fn principal_and_amortization_are_order_independent() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let principal = Money::new(1_000_000.0, Currency::USD);
    let amortization = AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 };

    let mut principal_first = CashFlowSchedule::builder();
    let _ = principal_first
        .principal(principal, issue, maturity)
        .amortization(amortization.clone())
        .fixed_cf(order_independence_fixed_spec());

    let mut amortization_first = CashFlowSchedule::builder();
    let _ = amortization_first
        .amortization(amortization)
        .fixed_cf(order_independence_fixed_spec())
        .principal(principal, issue, maturity);

    assert_eq!(
        principal_first.build_with_curves(None).unwrap().flows,
        amortization_first.build_with_curves(None).unwrap().flows
    );
}

#[test]
fn full_horizon_coupon_programs_are_order_independent() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let switch = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2027, Month::January, 15).unwrap();
    let principal = Money::new(1_000_000.0, Currency::USD);

    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder.fixed_cf(order_independence_fixed_spec());
    });
    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder.floating_cf(order_independence_float_spec());
    });

    let step_spec = || StepUpCouponSpec {
        coupon_type: CouponType::Cash,
        initial_rate: dec!(0.04),
        step_schedule: vec![(switch, dec!(0.05))],
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            freq: Tenor::quarterly(),

            dc: DayCount::Act360,

            bdc: BusinessDayConvention::Following,

            calendar_id: "weekends_only".to_string(),

            stub: StubKind::None,

            end_of_month: false,

            payment_lag_days: 0,

            adjust_accrual_dates: false,
        },
    };
    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder.step_up_cf(step_spec());
    });
    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder.fixed_stepup_decimal(
            &[(switch, dec!(0.04))],
            ScheduleParams::semiannual_30360(),
            CouponType::Cash,
        );
    });
    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder
            .fixed_cf(order_independence_fixed_spec())
            .payment_split_program(&[(switch, CouponType::PIK)]);
    });

    let fixed_window = || FixedWindow {
        rate: dec!(0.04),
        schedule: ScheduleParams::semiannual_30360(),
    };
    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder.fixed_to_float(
            switch,
            fixed_window(),
            order_independence_float_spec(),
            CouponType::Cash,
        );
    });
    assert_program_order_independent(principal, issue, maturity, |builder| {
        let _ = builder
            .float_margin_stepup_decimal(&[(switch, dec!(250))], order_independence_float_spec());
    });
}

#[test]
fn principal_does_not_clear_the_first_builder_error() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .fixed_stepup_decimal(&[], ScheduleParams::semiannual_30360(), CouponType::Cash)
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity);

    let error = builder.build_with_curves(None).unwrap_err().to_string();
    assert!(error.contains("requires at least one"), "{error}");
}
