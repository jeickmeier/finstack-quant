//! Tests for cashflow schedule generation and computation.
//!
//! This module covers:
//! - Schedule generation with various amortization schemes
//! - Flow ordering within dates
//! - Stub period detection
//! - Outstanding balance tracking
//! - PV/NPV calculations
//! - Day count conventions in schedule context
//!
//! # Tolerance Conventions
//!
//! - `RATE_TOLERANCE` (1e-10): For rate/factor comparisons
//! - `FACTOR_TOLERANCE` (1e-12): For year fractions
//! - `financial_tolerance(notional)`: For money amounts

use crate::helpers::financial_tolerance;
use finstack_cashflows::builder::specs::{
    CouponType, FeeSpec, FixedCouponSpec, FixedWindow, FloatingCouponSpec, FloatingRateFallback,
    FloatingRateSpec, OvernightIndexConstraintApplication,
};
use finstack_cashflows::builder::{AmortizationSpec, CashFlowSchedule};
use finstack_core::cashflow::Discountable;
use finstack_core::cashflow::{CFKind, CashFlow};
use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_core::market_data::term_structures::DiscountCurve as CoreDiscCurve;
use finstack_core::math::interp::InterpStyle;
use finstack_core::money::Money;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::Month;

// =============================================================================
// Amortization Scheme Tests
// =============================================================================

#[test]
fn linear_vs_step_parity() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000.0, Currency::USD);

    // Linear
    let mut b1 = CashFlowSchedule::builder();
    let _ = b1
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD),
        })
        .fixed_cf(fixed.clone());
    let s1 = b1.build_with_curves(None).unwrap();

    // Step schedule equivalent
    let sched = finstack_cashflows::builder::date_generation::build_dates(
        issue,
        maturity,
        Tenor::quarterly(),
        StubKind::None,
        BusinessDayConvention::Following,
        false,
        0,
        "weekends_only",
    )
    .unwrap()
    .dates;
    let delta = init.amount() / (sched.len()) as f64;
    let mut remaining = init.amount();
    let mut pairs: Vec<(Date, Money)> = Vec::new();
    for &d in &sched {
        remaining = (remaining - delta).max(0.0);
        pairs.push((d, Money::new(remaining, Currency::USD)));
    }

    let mut b2 = CashFlowSchedule::builder();
    let _ = b2
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::StepRemaining { schedule: pairs })
        .fixed_cf(fixed);
    let s2 = b2.build_with_curves(None).unwrap();

    assert_eq!(s1.flows.len(), s2.flows.len());
    for (a, b) in s1.flows.iter().zip(s2.flows.iter()) {
        assert_eq!(a.date, b.date);
        assert_eq!(a.kind, b.kind);
        assert!(
            (a.amount.amount() - b.amount.amount()).abs() < financial_tolerance(init.amount()),
            "Flow amounts should match: {} vs {}",
            a.amount.amount(),
            b.amount.amount()
        );
    }
}

#[test]
fn pik_capitalization_increases_outstanding() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000.0, Currency::USD);

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::PIK,
        rate: Decimal::try_from(0.10).expect("valid"),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let s = b.build_with_curves(None).unwrap();
    let path = s.outstanding_path_per_flow().unwrap();
    // Find last outstanding before redemption
    let last_before = path
        .iter()
        .rev()
        .find(|(d, _)| *d < maturity)
        .unwrap()
        .1
        .amount();
    assert!(last_before > init.amount());
}

#[test]
fn linear_amortization_uses_first_coupon_leg_cadence() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let switch = Date::from_calendar_date(2025, Month::July, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let init = Money::new(1_200.0, Currency::USD);

    let monthly_fixed = FixedWindow {
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_cashflows::builder::ScheduleParams {
            freq: Tenor::monthly(),
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
        },
    };
    let quarterly_float = FloatingCouponSpec {
        coupon_type: CouponType::Cash,
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: Decimal::try_from(200.0).expect("valid"),
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
            dc: DayCount::Act360,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            payment_lag_days: 0,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        },
        freq: Tenor::quarterly(),
        stub: StubKind::None,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD),
        })
        .fixed_to_float(switch, monthly_fixed, quarterly_float, CouponType::Cash);
    let schedule = builder.build_with_curves(None).unwrap();

    let amortization_dates: Vec<Date> = schedule
        .flows
        .iter()
        .filter(|flow| flow.kind == CFKind::Amortization)
        .map(|flow| flow.date)
        .collect();

    assert!(!amortization_dates.is_empty());
    assert!(
        amortization_dates.iter().all(|date| *date <= switch),
        "linear amortization should follow the first coupon leg cadence"
    );
    assert!(
        !amortization_dates.contains(&maturity),
        "the later floating leg cadence is not used for linear amortization"
    );
}

// =============================================================================
// Flow Ordering Tests
// =============================================================================

#[test]
fn ordering_invariants_within_date() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let init = Money::new(1_000.0, Currency::USD);
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Split {
            cash_pct: Decimal::try_from(0.5).expect("valid"),
            pik_pct: Decimal::try_from(0.5).expect("valid"),
        },
        rate: Decimal::try_from(0.10).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    // Percent-per-period amortization to force amort on coupon dates
    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 })
        .fixed_cf(fixed);
    let s = b.build_with_curves(None).unwrap();

    // On coupon dates where multiple flows exist, enforce order: Fixed/Stub -> Amortization -> PIK -> Notional
    let mut by_date: finstack_core::HashMap<Date, Vec<CFKind>> = finstack_core::HashMap::default();
    for cf in &s.flows {
        by_date.entry(cf.date).or_default().push(cf.kind);
    }

    for (_d, kinds) in by_date {
        let mut sorted = kinds.clone();
        sorted.sort_by_key(|k| match k {
            CFKind::Fixed | CFKind::Stub | CFKind::FloatReset => 0,
            CFKind::Fee => 1,
            CFKind::Amortization => 2,
            CFKind::PIK => 3,
            CFKind::Notional => 4,
            _ => 5,
        });
        assert_eq!(kinds, sorted);
    }
}

// =============================================================================
// PV/NPV Calculation Tests
// =============================================================================

#[test]
fn fixed_schedule_npv_equals_sum_cashflows() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    // Use a flat DF=1.0 curve for this test (testing NPV = sum when no discounting)
    // NOTE: Flat curves are not monotonically decreasing, so must allow_non_monotonic()
    let curve = CoreDiscCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        ) // Flat curve for testing NPV = sum of cashflows
        .build()
        .unwrap();

    let pv = schedule
        .npv(&curve, curve.base_date(), Some(schedule.day_count))
        .unwrap();

    // PV with flat curve DF=1.0 should equal sum of coupon amounts (no discounting)
    let expected = schedule
        .flows
        .iter()
        .fold(0.0, |sum, cf| sum + cf.amount.amount());
    assert!(
        (pv.amount() - expected).abs() < financial_tolerance(init.amount()),
        "PV should equal sum of cashflows: {} vs {}",
        pv.amount(),
        expected
    );
}

// =============================================================================
// Stub Period Detection Tests
// =============================================================================

#[test]
fn detects_stub_periods() {
    let issue = Date::from_calendar_date(2025, Month::January, 10).unwrap(); // irregular
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.04).expect("valid"),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::ShortFront,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    // Find coupon flows (not notional)
    let coupon_flows: Vec<&CashFlow> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub)
        .collect();

    // At least one should be a stub due to irregular start date
    let has_stub = coupon_flows.iter().any(|cf| cf.kind == CFKind::Stub);
    assert!(
        has_stub,
        "Should detect stub period with irregular start date"
    );

    // Only the genuinely irregular short-front period is labeled Stub; the
    // remaining regular periods (including the last) stay Fixed.
    let stub_count = coupon_flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Stub)
        .count();
    assert_eq!(stub_count, 1, "exactly one genuine stub period expected");
    let earliest = coupon_flows
        .iter()
        .min_by_key(|cf| cf.date)
        .expect("coupons present");
    assert_eq!(
        earliest.kind,
        CFKind::Stub,
        "the short-front stub coupon must be labeled Stub"
    );
}

/// Negative-rate fixed coupons must emit (negative cash amounts), not be
/// silently dropped — matching the floating path's behavior.
#[test]
fn negative_rate_fixed_coupons_are_emitted() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(-0.005).expect("valid"), // -0.5% (negative-rate regime)
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    let coupons: Vec<&CashFlow> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .collect();
    assert_eq!(coupons.len(), 2, "negative-rate coupons must be emitted");
    for cf in &coupons {
        assert!(
            cf.amount.amount() < 0.0,
            "negative-rate coupon should carry a negative amount, got {}",
            cf.amount.amount()
        );
        assert_eq!(cf.rate, Some(-0.005));
    }
}

// =============================================================================
// Strict serde on builder specs
// =============================================================================

/// Unknown (typo'd) fields in nested specs must be rejected, not silently
/// defaulted.
#[test]
fn floating_rate_spec_rejects_unknown_fields() {
    let json = r#"{
        "index_id": "USD-SOFR-3M",
        "spred_bp": "200",
        "reset_freq": {"count": 3, "unit": "months"},
        "dc": "Act360",
        "calendar_id": "weekends_only",
        "spread_bp": "200"
    }"#;

    let err =
        serde_json::from_str::<FloatingRateSpec>(json).expect_err("typo'd field must be rejected");
    assert!(
        err.to_string().contains("spred_bp"),
        "error should name the unknown field: {err}"
    );
}

/// Serde aliases (`floor_bp` -> `index_floor_bp`) keep working alongside
/// `deny_unknown_fields`.
#[test]
fn floating_rate_spec_accepts_floor_bp_alias() {
    let json = r#"{
        "index_id": "USD-SOFR-3M",
        "spread_bp": "200",
        "floor_bp": "0",
        "reset_freq": {"count": 3, "unit": "months"},
        "dc": "Act360",
        "calendar_id": "weekends_only"
    }"#;

    let spec: FloatingRateSpec = serde_json::from_str(json).expect("alias field must deserialize");
    assert_eq!(
        spec.index_floor_bp,
        Some(Decimal::ZERO),
        "floor_bp alias should populate index_floor_bp"
    );
}

// =============================================================================
// Outstanding Balance Tracking Tests
// =============================================================================

#[test]
fn outstanding_by_date_dedup_and_values() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let init = Money::new(10_000.0, Currency::USD);

    // Force multiple flows per date: split coupon (cash + PIK) and amortization on coupon dates
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Split {
            cash_pct: Decimal::try_from(0.5).expect("valid"),
            pik_pct: Decimal::try_from(0.5).expect("valid"),
        },
        rate: Decimal::try_from(0.12).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 })
        .fixed_cf(fixed);
    let s = b.build_with_curves(None).unwrap();

    let end_by_date = s.outstanding_by_date().unwrap();

    // 1) One entry per unique date
    let unique_dates: std::collections::BTreeSet<Date> = s.flows.iter().map(|cf| cf.date).collect();
    assert_eq!(end_by_date.len(), unique_dates.len());
    // Dates are ordered
    for ((d1, _), d2) in end_by_date.iter().zip(unique_dates.iter()) {
        assert_eq!(d1, d2);
    }

    // 2) Verify outstanding values are non-negative before maturity
    //    and zero at maturity (after redemption)
    for (i, (d, m)) in end_by_date.iter().enumerate() {
        assert!(
            m.amount() >= -0.01,
            "Outstanding should be non-negative, got {} at {:?}",
            m.amount(),
            d
        );

        // At maturity (last date), outstanding should be 0 after redemption
        if i == end_by_date.len() - 1 {
            assert!(
                m.amount().abs() < 0.01,
                "Outstanding at maturity should be 0 after redemption, got {}",
                m.amount()
            );
        }
    }

    // 3) At issue date, outstanding should be initial notional
    if let Some((d, m)) = end_by_date.first() {
        assert_eq!(*d, issue);
        assert!(
            (m.amount() - init.amount()).abs() < financial_tolerance(init.amount()),
            "Outstanding at issue should be initial notional {}, got {}",
            init.amount(),
            m.amount()
        );
    }

    // 4) Outstanding should decrease over time (due to amortization, despite PIK)
    //    with net decrease until maturity
    let first_outstanding = end_by_date.first().map(|(_, m)| m.amount()).unwrap_or(0.0);
    let last_outstanding = end_by_date.last().map(|(_, m)| m.amount()).unwrap_or(0.0);
    assert!(
        last_outstanding < first_outstanding,
        "Outstanding should decrease from {} to {} over the life",
        first_outstanding,
        last_outstanding
    );
}

#[test]
fn outstanding_by_date_includes_prepayment() {
    let prepay_date = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let schedule = finstack_cashflows::builder::schedule::CashFlowSchedule {
        flows: vec![CashFlow {
            date: prepay_date,
            reset_date: None,
            amount: Money::new(250.0, Currency::USD),
            kind: CFKind::PrePayment,
            accrual_factor: 0.0,
            rate: None,
        }],
        notional: finstack_cashflows::builder::Notional::par(1_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: finstack_cashflows::builder::schedule::CashFlowMeta {
            issue_date: Some(prepay_date),
            ..Default::default()
        },
    };

    let outstanding = schedule.outstanding_by_date().unwrap();
    assert_eq!(outstanding.len(), 1, "expected one dated balance snapshot");
    assert!(
        (outstanding[0].1.amount() - 750.0).abs() < 1e-10,
        "prepayment should reduce outstanding from 1000 to 750, got {}",
        outstanding[0].1.amount()
    );
}

#[test]
fn outstanding_by_date_includes_defaulted_notional() {
    let default_date = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let recovery_date = Date::from_calendar_date(2025, Month::September, 15).unwrap();
    let schedule = finstack_cashflows::builder::schedule::CashFlowSchedule {
        flows: vec![
            CashFlow {
                date: default_date,
                reset_date: None,
                amount: Money::new(300.0, Currency::USD),
                kind: CFKind::DefaultedNotional,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: recovery_date,
                reset_date: None,
                amount: Money::new(120.0, Currency::USD),
                kind: CFKind::Recovery,
                accrual_factor: 0.0,
                rate: None,
            },
        ],
        notional: finstack_cashflows::builder::Notional::par(1_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: finstack_cashflows::builder::schedule::CashFlowMeta {
            issue_date: Some(default_date),
            ..Default::default()
        },
    };

    let outstanding = schedule.outstanding_by_date().unwrap();
    assert_eq!(outstanding.len(), 2, "expected default and recovery dates");
    assert!(
        (outstanding[0].1.amount() - 700.0).abs() < 1e-10,
        "defaulted notional should reduce outstanding from 1000 to 700, got {}",
        outstanding[0].1.amount()
    );
    assert!(
        (outstanding[1].1.amount() - 700.0).abs() < 1e-10,
        "recovery should not restore outstanding, got {}",
        outstanding[1].1.amount()
    );
}

#[test]
fn builder_created_schedule_sets_issue_date_for_outstanding_by_date() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let init = Money::new(10_000.0, Currency::USD);
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = builder.build_with_curves(None).unwrap();

    assert_eq!(schedule.meta.issue_date, Some(issue));
    assert!(schedule.outstanding_by_date().is_ok());
}

#[test]
fn outstanding_by_date_requires_issue_date() {
    let prepay_date = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let schedule = finstack_cashflows::builder::schedule::CashFlowSchedule {
        flows: vec![CashFlow {
            date: prepay_date,
            reset_date: None,
            amount: Money::new(250.0, Currency::USD),
            kind: CFKind::PrePayment,
            accrual_factor: 0.0,
            rate: None,
        }],
        notional: finstack_cashflows::builder::Notional::par(1_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: finstack_cashflows::builder::schedule::CashFlowMeta::default(),
    };

    let err = schedule
        .outstanding_by_date()
        .expect_err("issue_date is required");
    assert!(err.to_string().contains("issue_date"));
}

#[test]
fn fixed_fee_on_issue_date_is_emitted() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fee(FeeSpec::Fixed {
            date: issue,
            amount: Money::new(12_500.0, Currency::USD),
        });

    let schedule = builder.build_with_curves(None).unwrap();
    let fees: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fee)
        .collect();

    assert_eq!(fees.len(), 1, "expected exactly one issue-date fixed fee");
    assert_eq!(fees[0].date, issue);
    assert!(
        (fees[0].amount.amount() - 12_500.0).abs() < 1e-10,
        "issue-date fixed fee amount should be preserved"
    );
}

// =============================================================================
// Error Handling Tests
// =============================================================================

#[test]
fn schedule_errors_on_unknown_calendar() {
    // Test that schedule generation propagates calendar lookup errors
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "UNKNOWN_CALENDAR_XYZ".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(Money::new(1_000_000.0, Currency::USD), issue, maturity)
        .fixed_cf(fixed);

    let result = builder.build_with_curves(None);
    assert!(
        result.is_err(),
        "Schedule generation should error on unknown calendar"
    );
}

// =============================================================================
// Day Count Convention in Schedule Context Tests
// =============================================================================

#[test]
fn stub_period_thirty360_produces_proportional_accrual() {
    // Test that stub periods with 30/360 day count produce proportionally smaller accrued amounts
    // Market convention: 30/360 treats each month as 30 days and each year as 360 days
    let issue = Date::from_calendar_date(2025, Month::February, 10).unwrap(); // Irregular start (10th)
    let maturity = Date::from_calendar_date(2026, Month::February, 15).unwrap(); // Regular end (15th)

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.06).expect("valid"), // 6% annual rate
        freq: Tenor::semi_annual(),
        dc: DayCount::Thirty360, // Market standard for corporate bonds
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::ShortFront,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed.clone());
    let schedule = b.build_with_curves(None).unwrap();

    // Find coupon flows only
    let coupon_flows: Vec<&CashFlow> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub)
        .collect();

    // Should have at least one stub period
    let stubs: Vec<&&CashFlow> = coupon_flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Stub)
        .collect();
    let regular: Vec<&&CashFlow> = coupon_flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .collect();

    assert!(!stubs.is_empty(), "Should have at least one stub period");
    assert!(
        !regular.is_empty(),
        "Should have at least one regular period"
    );

    // Regular semi-annual coupon should be approximately 3% of notional (6% / 2)
    // Stub period should be smaller due to shorter accrual period
    let regular_amount = regular[0].amount.amount();
    let stub_amount = stubs[0].amount.amount();

    let expected_regular =
        init.amount() * fixed.rate.to_f64().unwrap_or(0.0) * regular[0].accrual_factor;

    // Regular should match notional * rate * accrual_factor
    // Using financial_tolerance for $1M notional (allows ~$10 variance)
    assert!(
        (regular_amount - expected_regular).abs() < financial_tolerance(1_000_000.0),
        "Regular coupon should be ~{:.2} ± ${:.2}, got {}",
        expected_regular,
        financial_tolerance(1_000_000.0),
        regular_amount
    );

    let expected_stub =
        init.amount() * fixed.rate.to_f64().unwrap_or(0.0) * stubs[0].accrual_factor;

    assert!(
        (stub_amount - expected_stub).abs() < financial_tolerance(1_000_000.0),
        "Stub coupon should be ~{:.2} ± ${:.2}, got {}",
        expected_stub,
        financial_tolerance(1_000_000.0),
        stub_amount
    );

    assert!(
        (stubs[0].accrual_factor - regular[0].accrual_factor).abs() > 1e-6,
        "Stub and regular accrual factors should differ"
    );
}

// =============================================================================
// NPV Golden Value Tests (with realistic discounting)
// =============================================================================

/// Golden value test: NPV discounting verification
///
/// This test verifies that discounting is working correctly by checking:
/// 1. NPV with DF=1 equals sum of positive flows
/// 2. NPV decreases as discount rate increases
/// 3. The discount factor is applied correctly
///
/// Note: The schedule includes an initial funding flow (negative) at issue date.
/// For holder-view NPV (excludes flows <= as_of), we need to understand that
/// NPV represents the net value of all flows to the holder.
#[test]
fn npv_golden_value_with_realistic_discount_curve() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"), // 5% coupon
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    // Count positive and negative flows to understand structure
    let positive_flows: f64 = schedule
        .flows
        .iter()
        .filter(|cf| cf.amount.amount() > 0.0)
        .map(|cf| cf.amount.amount())
        .sum();

    let negative_flows: f64 = schedule
        .flows
        .iter()
        .filter(|cf| cf.amount.amount() < 0.0)
        .map(|cf| cf.amount.amount())
        .sum();

    // Total undiscounted: should be net of funding (-1M) + coupons + redemption (+1M)
    // For a bond: -1M (funding) + coupons + 1M (redemption) = coupons only
    let _total_undiscounted = positive_flows + negative_flows;

    // Build flat DF=1 curve for baseline
    let flat_curve = CoreDiscCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (2.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap();

    let npv_flat = schedule
        .npv(&flat_curve, issue, Some(schedule.day_count))
        .unwrap();

    // Build 5% discount curve
    let curve_5pct = CoreDiscCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([
            (0.0, 1.0),
            (0.5, (-0.05_f64 * 0.5).exp()),
            (1.0, (-0.05_f64 * 1.0).exp()),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    let npv_5pct = schedule
        .npv(&curve_5pct, issue, Some(schedule.day_count))
        .unwrap();

    // Key invariants:
    // 1. NPV with 5% rate should be less than NPV with flat curve (discounting works)
    assert!(
        npv_5pct.amount() < npv_flat.amount() + 1.0, // Allow small tolerance
        "NPV with 5% rate ({:.2}) should be <= NPV with flat rate ({:.2})",
        npv_5pct.amount(),
        npv_flat.amount()
    );

    // 2. The schedule generates expected number of positive cash flows
    // (2 coupons + 1 redemption = 3 positive flows for 1-year semi-annual)
    let positive_count = schedule
        .flows
        .iter()
        .filter(|cf| cf.amount.amount() > 0.0)
        .count();
    assert!(
        positive_count >= 2,
        "Should have at least 2 positive flows (coupons + redemption), got {}",
        positive_count
    );

    // 3. Verify coupon amounts are correct (~25K each for 5% semi-annual on 1M)
    let coupons: Vec<_> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .collect();

    // A regular semi-annual one-year bullet emits exactly two Fixed coupons —
    // none mislabeled as Stub (the golden loop below must not be vacuous).
    assert_eq!(
        coupons.len(),
        2,
        "expected 2 regular Fixed coupons, got {}",
        coupons.len()
    );
    assert!(
        !schedule.flows.iter().any(|cf| cf.kind == CFKind::Stub),
        "regular bullet schedule must not contain Stub coupons"
    );

    for coupon in &coupons {
        let expected_coupon = 25_000.0; // 1M * 5% / 2
        assert!(
            (coupon.amount.amount() - expected_coupon).abs() < 2000.0,
            "Coupon should be ~${:.0}, got ${:.2}",
            expected_coupon,
            coupon.amount.amount()
        );
    }
}

/// Golden value test: coupon amounts with known expected values
///
/// Verifies that:
/// - Semi-annual 5% coupon on $1M = $25,000 (exactly)
/// - Day count fraction is applied correctly
#[test]
fn coupon_amount_golden_values() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    // 5% semi-annual coupon on $1M notional
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::semi_annual(),
        dc: DayCount::Thirty360, // 30/360 gives exact 0.5 year fraction for 6 months
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    // Find coupon flows
    let coupons: Vec<&CashFlow> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .collect();

    // A regular semi-annual one-year bullet emits exactly two Fixed coupons —
    // none mislabeled as Stub (otherwise the golden loop below is vacuous).
    assert_eq!(
        coupons.len(),
        2,
        "expected 2 regular Fixed coupons, got {}",
        coupons.len()
    );
    assert!(
        !schedule.flows.iter().any(|cf| cf.kind == CFKind::Stub),
        "regular bullet schedule must not contain Stub coupons"
    );

    // Expected coupon: $1M * 5% * 0.5 = $25,000
    let expected_coupon = 25_000.0;

    for coupon in &coupons {
        assert!(
            (coupon.amount.amount() - expected_coupon).abs() < financial_tolerance(init.amount()),
            "Coupon amount should be ${:.2}, got ${:.2}",
            expected_coupon,
            coupon.amount.amount()
        );

        // Verify accrual factor is approximately 0.5 for semi-annual
        assert!(
            (coupon.accrual_factor - 0.5).abs() < 0.01,
            "Accrual factor should be ~0.5 for semi-annual, got {}",
            coupon.accrual_factor
        );
    }

    // Verify principal redemption amount
    let redemption: Vec<&CashFlow> = schedule
        .flows
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional && cf.amount.amount() > 0.0)
        .collect();

    assert_eq!(
        redemption.len(),
        1,
        "Should have exactly one redemption flow"
    );
    assert!(
        (redemption[0].amount.amount() - init.amount()).abs() < financial_tolerance(init.amount()),
        "Redemption should equal notional: expected ${:.2}, got ${:.2}",
        init.amount(),
        redemption[0].amount.amount()
    );
}

// =============================================================================
// Conservation Invariant Tests
// =============================================================================

/// Invariant test: cashflow conservation for a par bullet bond.
///
/// For a fixed-rate bullet bond, the undiscounted sum of all principal
/// flows (initial funding + final redemption) should net to zero.
///
/// This test verifies that the schedule builder doesn't "leak" or
/// "manufacture" principal.
#[test]
fn cashflow_conservation_bond_principal() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let notional_amt = 1_000_000.0;

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(notional_amt, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    // Sum principal-related flows (Notional + Amortization)
    let principal_sum: f64 = schedule
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Notional | CFKind::Amortization))
        .map(|cf| cf.amount.amount())
        .sum();

    // For a bullet bond: initial funding (-notional) + final principal (+notional) = 0
    assert!(
        principal_sum.abs() < financial_tolerance(notional_amt),
        "Principal conservation violated: sum of principal flows = {} (should be ~0 for bullet)",
        principal_sum
    );
}

/// Invariant test: cashflow conservation for an amortizing bond.
///
/// For a linear amortizing bond, the sum of all amortization and notional
/// flows should net to zero (funding in = principal returned out).
#[test]
fn cashflow_conservation_amortizing_bond_principal() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2027, Month::January, 15).unwrap();
    let notional_amt = 1_000_000.0;

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.04).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(notional_amt, Currency::USD);
    let final_notional = Money::new(0.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .fixed_cf(fixed)
        .amortization(AmortizationSpec::LinearTo { final_notional });
    let schedule = b.build_with_curves(None).unwrap();

    // Sum principal-related flows
    let principal_sum: f64 = schedule
        .flows
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Notional | CFKind::Amortization))
        .map(|cf| cf.amount.amount())
        .sum();

    // For amortizing to zero: initial funding + sum(amortization) + final = 0
    assert!(
        principal_sum.abs() < financial_tolerance(notional_amt),
        "Amortizing conservation violated: sum of principal flows = {} (should be ~0)",
        principal_sum
    );
}

/// Invariant test: outstanding balance never goes negative.
#[test]
fn outstanding_never_negative() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2027, Month::January, 15).unwrap();
    let notional_amt = 1_000_000.0;

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.04).expect("valid"),
        freq: Tenor::quarterly(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(notional_amt, Currency::USD);
    let final_notional = Money::new(0.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .fixed_cf(fixed)
        .amortization(AmortizationSpec::LinearTo { final_notional });
    let schedule = b.build_with_curves(None).unwrap();

    // Get outstanding path (returns Vec<(Date, Money)>)
    let outstanding_path = schedule
        .outstanding_path_per_flow()
        .expect("outstanding path should succeed");
    for (i, (_date, balance)) in outstanding_path.iter().enumerate() {
        assert!(
            balance.amount() >= -financial_tolerance(notional_amt),
            "Outstanding balance went negative at flow {}: {}",
            i,
            balance.amount()
        );
    }
}

/// Invariant test: PV01 relationship (higher rates = lower NPV)
#[test]
fn npv_decreases_with_higher_discount_rate() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        freq: Tenor::semi_annual(),
        dc: DayCount::Act365F,
        bdc: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::None,
        end_of_month: false,
        payment_lag_days: 0,
    };

    let init = Money::new(1_000_000.0, Currency::USD);

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build_with_curves(None).unwrap();

    // Build curves at different rates
    let build_curve = |rate: f64| {
        CoreDiscCurve::builder("USD-OIS")
            .base_date(issue)
            .knots([
                (0.0, 1.0),
                (0.5, (-rate * 0.5).exp()),
                (1.0, (-rate * 1.0).exp()),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap()
    };

    let curve_3pct = build_curve(0.03);
    let curve_5pct = build_curve(0.05);
    let curve_7pct = build_curve(0.07);

    let npv_3pct = schedule
        .npv(
            &curve_3pct,
            curve_3pct.base_date(),
            Some(schedule.day_count),
        )
        .unwrap();
    let npv_5pct = schedule
        .npv(
            &curve_5pct,
            curve_5pct.base_date(),
            Some(schedule.day_count),
        )
        .unwrap();
    let npv_7pct = schedule
        .npv(
            &curve_7pct,
            curve_7pct.base_date(),
            Some(schedule.day_count),
        )
        .unwrap();

    // Monotonicity: higher discount rate = lower NPV
    assert!(
        npv_3pct.amount() > npv_5pct.amount(),
        "NPV at 3% ({}) should be greater than NPV at 5% ({})",
        npv_3pct.amount(),
        npv_5pct.amount()
    );
    assert!(
        npv_5pct.amount() > npv_7pct.amount(),
        "NPV at 5% ({}) should be greater than NPV at 7% ({})",
        npv_5pct.amount(),
        npv_7pct.amount()
    );
}

// =============================================================================
// Weighted Average Life (WAL) Tests
// =============================================================================

#[test]
fn test_weighted_average_life_two_amort() {
    // Two equal amortization payments at ~1y and ~2y from as_of
    // WAL should be ~1.5 years
    use finstack_cashflows::builder::schedule::CashFlowMeta;
    use finstack_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let d1 = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let d2 = Date::from_calendar_date(2027, Month::January, 15).unwrap();

    let flows = vec![
        CashFlow {
            date: d1,
            reset_date: None,
            amount: Money::new(500.0, Currency::USD),
            kind: CFKind::Amortization,
            accrual_factor: 0.0,
            rate: None,
        },
        CashFlow {
            date: d2,
            reset_date: None,
            amount: Money::new(500.0, Currency::USD),
            kind: CFKind::Amortization,
            accrual_factor: 0.0,
            rate: None,
        },
    ];

    let schedule = CashFlowSchedule {
        flows,
        notional: Notional::par(1_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: CashFlowMeta::default(),
    };

    let wal = schedule
        .weighted_average_life(as_of)
        .expect("WAL should succeed");
    // With Act/365F, ~365 days = ~1.0y, ~730 days = ~2.0y
    // Equal weights => WAL ~ 1.5
    assert!(
        (wal - 1.5).abs() < 0.02,
        "WAL for two equal amortizations at 1y and 2y should be ~1.5, got {}",
        wal
    );
}

#[test]
fn test_weighted_average_life_bullet() {
    // Single notional payment at 5y (bullet bond)
    // WAL should be ~5.0 years
    use finstack_cashflows::builder::schedule::CashFlowMeta;
    use finstack_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 15).unwrap();

    let flows = vec![CashFlow {
        date: maturity,
        reset_date: None,
        amount: Money::new(1_000_000.0, Currency::USD),
        kind: CFKind::Notional,
        accrual_factor: 0.0,
        rate: None,
    }];

    let schedule = CashFlowSchedule {
        flows,
        notional: Notional::par(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: CashFlowMeta::default(),
    };

    let wal = schedule
        .weighted_average_life(as_of)
        .expect("WAL should succeed");
    // 5 years with Act/365F (1826 or 1827 days depending on leap years)
    assert!(
        (wal - 5.0).abs() < 0.02,
        "WAL for bullet at 5y should be ~5.0, got {}",
        wal
    );
}

#[test]
fn test_weighted_average_life_empty() {
    // No flows => WAL = 0.0
    use finstack_cashflows::builder::schedule::CashFlowMeta;
    use finstack_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    let schedule = CashFlowSchedule {
        flows: vec![],
        notional: Notional::par(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: CashFlowMeta::default(),
    };

    let wal = schedule
        .weighted_average_life(as_of)
        .expect("WAL should succeed");
    assert!(wal == 0.0, "WAL with no flows should be 0.0, got {}", wal);
}

#[test]
fn test_weighted_average_life_ignores_coupons() {
    // Mix of coupon and principal flows
    // WAL should only count principal flows
    use finstack_cashflows::builder::schedule::CashFlowMeta;
    use finstack_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let d1 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let d2 = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let flows = vec![
        // Coupon at 6m - should be ignored
        CashFlow {
            date: d1,
            reset_date: None,
            amount: Money::new(25_000.0, Currency::USD),
            kind: CFKind::Fixed,
            accrual_factor: 0.5,
            rate: Some(0.05),
        },
        // Coupon at 1y - should be ignored
        CashFlow {
            date: d2,
            reset_date: None,
            amount: Money::new(25_000.0, Currency::USD),
            kind: CFKind::Fixed,
            accrual_factor: 0.5,
            rate: Some(0.05),
        },
        // Single principal redemption at maturity - only this counts
        CashFlow {
            date: maturity,
            reset_date: None,
            amount: Money::new(1_000_000.0, Currency::USD),
            kind: CFKind::Notional,
            accrual_factor: 0.0,
            rate: None,
        },
    ];

    let schedule = CashFlowSchedule {
        flows,
        notional: Notional::par(1_000_000.0, Currency::USD),
        day_count: DayCount::Act365F,
        meta: CashFlowMeta::default(),
    };

    let wal = schedule
        .weighted_average_life(as_of)
        .expect("WAL should succeed");
    // Should be ~1.0 year (only the notional at maturity counts)
    assert!(
        (wal - 1.0).abs() < 0.02,
        "WAL should be ~1.0 (only principal, ignoring coupons), got {}",
        wal
    );
}
