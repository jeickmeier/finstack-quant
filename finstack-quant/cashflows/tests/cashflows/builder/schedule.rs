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
use finstack_quant_cashflows::builder::specs::{
    CouponType, FeeSpec, FixedCouponSpec, FloatingCouponSpec, FloatingRateFallback,
    FloatingRateSpec, OvernightIndexConstraintApplication,
};
use finstack_quant_cashflows::builder::{AmortizationSpec, CashFlowSchedule, PrincipalExchange};
use finstack_quant_core::cashflow::Discountable;
use finstack_quant_core::cashflow::{CFKind, CashFlow};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::term_structures::DiscountCurve as CoreDiscCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use time::Month;

#[test]
fn linear_vs_step_parity() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000.0, Currency::USD).expect("valid money fixture");

    let mut b1 = CashFlowSchedule::builder();
    let _ = b1
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        })
        .fixed_cf(fixed.clone());
    let s1 = b1.build(None).unwrap();

    let sched = finstack_quant_cashflows::builder::periods::build_periods(
        finstack_quant_cashflows::builder::periods::BuildPeriodsParams {
            start: issue,
            end: maturity,
            frequency: Tenor::quarterly(),
            stub: StubKind::None,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only",
            end_of_month: false,
            day_count: DayCount::Act365F,
            payment_lag_days: 0,
            reset_lag_days: None,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    )
    .unwrap();
    let delta = init.amount() / (sched.len()) as f64;
    let mut remaining = init.amount();
    let mut pairs: Vec<(Date, Money)> = Vec::new();
    for period in &sched {
        let d = period.payment_date;
        remaining = (remaining - delta).max(0.0);
        pairs.push((
            d,
            Money::new(remaining, Currency::USD).expect("valid money fixture"),
        ));
    }

    let mut b2 = CashFlowSchedule::builder();
    let _ = b2
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::StepRemaining { schedule: pairs })
        .fixed_cf(fixed);
    let s2 = b2.build(None).unwrap();

    assert_eq!(s1.get_flows().len(), s2.get_flows().len());
    for (a, b) in s1.get_flows().iter().zip(s2.get_flows().iter()) {
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
    let init = Money::new(1_000.0, Currency::USD).expect("valid money fixture");

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Pik,
        rate: Decimal::try_from(0.10).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let s = b.build(None).unwrap();
    let path = s.outstanding_by_date().unwrap();
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
fn linear_amortization_spans_fixed_to_float_cadences() {
    let issue = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let switch = Date::from_calendar_date(2025, Month::July, 1).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    let init = Money::new(1_200.0, Currency::USD).expect("valid money fixture");

    let monthly_fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
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
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        },
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        })
        .fixed_to_float(switch, monthly_fixed, quarterly_float);
    let schedule = builder.build(None).unwrap();

    let amortization_dates: Vec<Date> = schedule
        .get_flows()
        .iter()
        .filter(|flow| flow.kind == CFKind::Amortization)
        .map(|flow| flow.date)
        .collect();

    assert_eq!(amortization_dates.len(), 8); // six monthly + two quarterly periods
    assert!(amortization_dates.contains(&maturity));
    assert!(
        amortization_dates.contains(&Date::from_calendar_date(2025, Month::October, 1).unwrap())
    );
    for flow in schedule
        .get_flows()
        .iter()
        .filter(|f| f.kind == CFKind::Amortization)
    {
        assert!((flow.amount.amount() - 150.0).abs() < 1e-10);
    }
}

/// A mid-horizon fixed-to-float conversion starts a fresh schedule at
/// `switch` (short-front stub) rather than continuing the pre-switch roll.
#[test]
fn fixed_to_float_window_has_fresh_front_stub_at_switch() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let switch = Date::from_calendar_date(2025, Month::April, 20).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let quarterly = finstack_quant_cashflows::builder::ScheduleParams {
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::Following,
        calendar_id: "weekends_only".to_string(),
        stub: StubKind::ShortFront,
        end_of_month: false,
        payment_lag_days: 0,
        adjust_accrual_dates: false,
        roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
    };
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: quarterly.clone(),
    };
    let floating = FloatingCouponSpec {
        coupon_type: CouponType::Cash,
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR-3M".into(),
            spread_bp: Decimal::try_from(0.0).expect("valid"),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        },
        schedule: quarterly,
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .fixed_to_float(switch, fixed, floating);
    let schedule = builder.build(None).expect("fixed-to-float schedule");

    let first_float = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.kind == CFKind::FloatReset)
        .expect("post-switch coupon");
    let accrual = first_float.accrual.as_ref().expect("float coupon accrual");
    assert_eq!(
        accrual.start, switch,
        "float window must start a fresh schedule at switch, not continue the Jan-15 roll"
    );
    let continued_roll = Date::from_calendar_date(2025, Month::April, 15).unwrap();
    assert_ne!(
        accrual.start, continued_roll,
        "must not continue the pre-switch quarterly roll"
    );
    assert!(
        first_float.accrual_factor < 0.25,
        "first post-switch period should be a short-front stub, got {}",
        first_float.accrual_factor
    );
}

#[test]
fn ordering_invariants_within_date() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let init = Money::new(1_000.0, Currency::USD).expect("valid money fixture");
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Split {
            cash_pct: Decimal::try_from(0.5).expect("valid"),
            pik_pct: Decimal::try_from(0.5).expect("valid"),
        },
        rate: Decimal::try_from(0.10).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 })
        .fixed_cf(fixed);
    let s = b.build(None).unwrap();

    // Same-date order: Fixed/Stub -> Amortization -> PIK -> Notional
    let mut by_date: finstack_quant_core::HashMap<Date, Vec<CFKind>> =
        finstack_quant_core::HashMap::default();
    for cf in s.get_flows() {
        by_date.entry(cf.date).or_default().push(cf.kind);
    }

    for (_d, kinds) in by_date {
        let mut sorted = kinds.clone();
        sorted.sort_by_key(|k| match k {
            CFKind::Fixed | CFKind::Stub | CFKind::FloatReset => 0,
            CFKind::Fee => 1,
            CFKind::Amortization => 2,
            CFKind::Pik => 3,
            CFKind::Notional => 4,
            _ => 5,
        });
        assert_eq!(kinds, sorted);
    }
}

#[test]
fn fixed_schedule_npv_equals_sum_cashflows() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build(None).unwrap();

    // Flat DF=1.0: NPV equals strictly-future cash; issue funding is already settled.
    // Flat knots are not monotonically decreasing, so allow_non_monotonic is required.
    let curve = CoreDiscCurve::builder("USD-OIS")
        .base_date(issue)
        .knots([(0.0, 1.0), (5.0, 1.0)])
        .interp(InterpStyle::Linear)
        .validation(
            finstack_quant_core::market_data::term_structures::ValidationMode::Raw {
                allow_non_monotonic: true,
                forward_floor: None,
            },
        )
        .build()
        .unwrap();

    let pv = schedule.npv(&curve, curve.base_date()).unwrap();

    let expected = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.date > issue)
        .fold(0.0, |sum, cf| sum + cf.amount.amount());
    assert!(
        (pv.amount() - expected).abs() < financial_tolerance(init.amount()),
        "PV should equal sum of cashflows: {} vs {}",
        pv.amount(),
        expected
    );
}

#[test]
fn detects_stub_periods() {
    let issue = Date::from_calendar_date(2025, Month::January, 10).unwrap(); // irregular
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.04).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build(None).unwrap();

    let coupon_flows: Vec<&CashFlow> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub)
        .collect();

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
        rate: Decimal::try_from(-0.005).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            // -0.5% (negative-rate regime)
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            issue,
            maturity,
        )
        .fixed_cf(fixed);
    let schedule = b.build(None).unwrap();

    let coupons: Vec<&CashFlow> = schedule
        .get_flows()
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

/// Unknown (typo'd) fields in nested specs must be rejected, not silently
/// defaulted.
#[test]
fn floating_rate_spec_rejects_unknown_fields() {
    let json = r#"{
        "index_id": "USD-SOFR-3M",
        "spred_bp": "200",
        "reset_frequency": {"count": 3, "unit": "months"},
        "day_count": "act_360",
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

/// Retired field names remain rejected alongside other unknown fields.
#[test]
fn floating_rate_spec_rejects_floor_bp_alias() {
    let json = r#"{
        "index_id": "USD-SOFR-3M",
        "spread_bp": "200",
        "floor_bp": "0",
        "reset_frequency": {"count": 3, "unit": "months"}
    }"#;

    let error = serde_json::from_str::<FloatingRateSpec>(json)
        .expect_err("retired floor_bp field must be rejected");
    assert!(error.to_string().contains("floor_bp"));
}

#[test]
fn outstanding_by_date_dedup_and_values() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let init = Money::new(10_000.0, Currency::USD).expect("valid money fixture");

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Split {
            cash_pct: Decimal::try_from(0.5).expect("valid"),
            pik_pct: Decimal::try_from(0.5).expect("valid"),
        },
        rate: Decimal::try_from(0.12).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .amortization(AmortizationSpec::PercentOfOriginalPerPeriod { pct: 0.25 })
        .fixed_cf(fixed);
    let s = b.build(None).unwrap();

    let end_by_date = s.outstanding_by_date().unwrap();

    let unique_dates: std::collections::BTreeSet<Date> =
        s.get_flows().iter().map(|cf| cf.date).collect();
    assert_eq!(end_by_date.len(), unique_dates.len());
    for ((d1, _), d2) in end_by_date.iter().zip(unique_dates.iter()) {
        assert_eq!(d1, d2);
    }

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

    if let Some((d, m)) = end_by_date.first() {
        assert_eq!(*d, issue);
        assert!(
            (m.amount() - init.amount()).abs() < financial_tolerance(init.amount()),
            "Outstanding at issue should be initial notional {}, got {}",
            init.amount(),
            m.amount()
        );
    }

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
    let schedule = finstack_quant_cashflows::builder::schedule::CashFlowSchedule::from_parts(
        vec![CashFlow::new(
            prepay_date,
            None,
            Money::new(250.0, Currency::USD).expect("valid money fixture"),
            CFKind::PrePayment,
            0.0,
            None,
        )],
        finstack_quant_cashflows::builder::Notional::par(1_000.0, Currency::USD)
            .expect("valid notional fixture"),
        DayCount::Act365F,
        finstack_quant_cashflows::builder::schedule::CashFlowMeta {
            issue_date: Some(prepay_date),
            ..Default::default()
        },
    );

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
    let schedule = finstack_quant_cashflows::builder::schedule::CashFlowSchedule::from_parts(
        vec![
            CashFlow::new(
                default_date,
                None,
                Money::new(300.0, Currency::USD).expect("valid money fixture"),
                CFKind::DefaultedNotional,
                0.0,
                None,
            ),
            CashFlow::new(
                recovery_date,
                None,
                Money::new(120.0, Currency::USD).expect("valid money fixture"),
                CFKind::Recovery,
                0.0,
                None,
            ),
        ],
        finstack_quant_cashflows::builder::Notional::par(1_000.0, Currency::USD)
            .expect("valid notional fixture"),
        DayCount::Act365F,
        finstack_quant_cashflows::builder::schedule::CashFlowMeta {
            issue_date: Some(default_date),
            ..Default::default()
        },
    );

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
    let init = Money::new(10_000.0, Currency::USD).expect("valid money fixture");
    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = builder.build(None).unwrap();

    assert_eq!(schedule.get_meta().issue_date, Some(issue));
    assert!(schedule.outstanding_by_date().is_ok());
}

#[test]
fn outstanding_by_date_requires_issue_date() {
    let prepay_date = Date::from_calendar_date(2025, Month::March, 15).unwrap();
    let schedule = finstack_quant_cashflows::builder::schedule::CashFlowSchedule::from_parts(
        vec![CashFlow::new(
            prepay_date,
            None,
            Money::new(250.0, Currency::USD).expect("valid money fixture"),
            CFKind::PrePayment,
            0.0,
            None,
        )],
        finstack_quant_cashflows::builder::Notional::par(1_000.0, Currency::USD)
            .expect("valid notional fixture"),
        DayCount::Act365F,
        finstack_quant_cashflows::builder::schedule::CashFlowMeta::default(),
    );

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
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            issue,
            maturity,
        )
        .fee(FeeSpec::Fixed {
            date: issue,
            amount: Money::new(12_500.0, Currency::USD).expect("valid money fixture"),
        });

    let schedule = builder.build(None).unwrap();
    let fees: Vec<_> = schedule
        .get_flows()
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

#[test]
fn schedule_errors_on_unknown_calendar() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "UNKNOWN_CALENDAR_XYZ".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            issue,
            maturity,
        )
        .fixed_cf(fixed);

    let result = builder.build(None);
    assert!(
        result.is_err(),
        "Schedule generation should error on unknown calendar"
    );
}

#[test]
fn stub_period_thirty360_produces_proportional_accrual() {
    // Market convention: 30/360 treats each month as 30 days and each year as 360 days
    let issue = Date::from_calendar_date(2025, Month::February, 10).unwrap(); // Irregular start (10th)
    let maturity = Date::from_calendar_date(2026, Month::February, 15).unwrap(); // Regular end (15th)

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.06).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed.clone());
    let schedule = b.build(None).unwrap();

    let coupon_flows: Vec<&CashFlow> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed || cf.kind == CFKind::Stub)
        .collect();

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
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            // 30/360 gives exact 0.5 year fraction for 6 months
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build(None).unwrap();

    let coupons: Vec<&CashFlow> = schedule
        .get_flows()
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
        !schedule
            .get_flows()
            .iter()
            .any(|cf| cf.kind == CFKind::Stub),
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

        assert!(
            (coupon.accrual_factor - 0.5).abs() < 0.01,
            "Accrual factor should be ~0.5 for semi-annual, got {}",
            coupon.accrual_factor
        );
    }

    let redemption: Vec<&CashFlow> = schedule
        .get_flows()
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
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(notional_amt, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build(None).unwrap();

    let principal_sum: f64 = schedule
        .get_flows()
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
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(notional_amt, Currency::USD).expect("valid money fixture");
    let final_notional = Money::new(0.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .fixed_cf(fixed)
        .amortization(AmortizationSpec::LinearTo { final_notional });
    let schedule = b.build(None).unwrap();

    let principal_sum: f64 = schedule
        .get_flows()
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
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(notional_amt, Currency::USD).expect("valid money fixture");
    let final_notional = Money::new(0.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b
        .principal(init, issue, maturity)
        .fixed_cf(fixed)
        .amortization(AmortizationSpec::LinearTo { final_notional });
    let schedule = b.build(None).unwrap();

    let outstanding_path = schedule
        .outstanding_by_date()
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
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut b = CashFlowSchedule::builder();
    let _ = b.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = b.build(None).unwrap();

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

    let npv_3pct = schedule.npv(&curve_3pct, curve_3pct.base_date()).unwrap();
    let npv_5pct = schedule.npv(&curve_5pct, curve_5pct.base_date()).unwrap();
    let npv_7pct = schedule.npv(&curve_7pct, curve_7pct.base_date()).unwrap();

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

#[test]
fn test_weighted_average_life_two_amort() {
    // Two equal amortization payments at ~1y and ~2y from as_of
    // WAL should be ~1.5 years
    use finstack_quant_cashflows::builder::schedule::CashFlowMeta;
    use finstack_quant_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let d1 = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let d2 = Date::from_calendar_date(2027, Month::January, 15).unwrap();

    let flows = vec![
        CashFlow::new(
            d1,
            None,
            Money::new(500.0, Currency::USD).expect("valid money fixture"),
            CFKind::Amortization,
            0.0,
            None,
        ),
        CashFlow::new(
            d2,
            None,
            Money::new(500.0, Currency::USD).expect("valid money fixture"),
            CFKind::Amortization,
            0.0,
            None,
        ),
    ];

    let schedule = CashFlowSchedule::from_parts(
        flows,
        Notional::par(1_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

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
    use finstack_quant_cashflows::builder::schedule::CashFlowMeta;
    use finstack_quant_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2030, Month::January, 15).unwrap();

    let flows = vec![CashFlow::new(
        maturity,
        None,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        CFKind::Notional,
        0.0,
        None,
    )];

    let schedule = CashFlowSchedule::from_parts(
        flows,
        Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

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
    use finstack_quant_cashflows::builder::schedule::CashFlowMeta;
    use finstack_quant_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    let schedule = CashFlowSchedule::from_parts(
        vec![],
        Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

    let wal = schedule
        .weighted_average_life(as_of)
        .expect("WAL should succeed");
    assert!(wal == 0.0, "WAL with no flows should be 0.0, got {}", wal);
}

#[test]
fn test_weighted_average_life_ignores_coupons() {
    // Mix of coupon and principal flows
    // WAL should only count principal flows
    use finstack_quant_cashflows::builder::schedule::CashFlowMeta;
    use finstack_quant_cashflows::builder::Notional;

    let as_of = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let d1 = Date::from_calendar_date(2025, Month::July, 15).unwrap();
    let d2 = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();

    let flows = vec![
        // Coupon at 6m - should be ignored
        CashFlow::new(
            d1,
            None,
            Money::new(25_000.0, Currency::USD).expect("valid money fixture"),
            CFKind::Fixed,
            0.5,
            Some(0.05),
        ),
        // Coupon at 1y - should be ignored
        CashFlow::new(
            d2,
            None,
            Money::new(25_000.0, Currency::USD).expect("valid money fixture"),
            CFKind::Fixed,
            0.5,
            Some(0.05),
        ),
        // Single principal redemption at maturity - only this counts
        CashFlow::new(
            maturity,
            None,
            Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
            CFKind::Notional,
            0.0,
            None,
        ),
    ];

    let schedule = CashFlowSchedule::from_parts(
        flows,
        Notional::par(1_000_000.0, Currency::USD).expect("valid notional fixture"),
        DayCount::Act365F,
        CashFlowMeta::default(),
    );

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

/// Saturday maturity, quarterly Act/360, T+2 payment lag: last coupon and
/// redemption share `adjust(maturity) + 2` business days; no notional on the
/// raw weekend maturity.
#[test]
fn lagged_redemption_matches_final_coupon_on_weekend_maturity() {
    let issue = Date::from_calendar_date(2025, Month::January, 17).unwrap(); // Friday
    let maturity = Date::from_calendar_date(2026, Month::January, 17).unwrap(); // Saturday
                                                                                // Following: Sat 17 -> Mon 19; +2 business days -> Wed 21.
    let redemption_date = Date::from_calendar_date(2026, Month::January, 21).unwrap();

    let float = FloatingCouponSpec {
        coupon_type: CouponType::Cash,
        rate_spec: FloatingRateSpec {
            index_id: "USD-SOFR".into(),
            spread_bp: Decimal::try_from(0.0).expect("valid"),
            gearing: Decimal::ONE,
            gearing_includes_spread: true,
            index_floor_bp: None,
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            overnight_index_constraints: OvernightIndexConstraintApplication::Daily,
            reset_frequency: Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 0,
            fixing_calendar_id: None,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: FloatingRateFallback::SpreadOnly,
        },
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 2,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).floating_cf(float);
    let schedule = builder.build(None).expect("lagged SOFR-style schedule");

    let last_coupon_date = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.date)
        .max()
        .expect("coupon flows");
    let redemption = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.kind == CFKind::Notional && cf.amount.amount() > 0.0)
        .expect("redemption flow");

    assert_eq!(last_coupon_date, redemption_date);
    assert_eq!(redemption.date, redemption_date);
    assert!(
        schedule
            .get_flows()
            .iter()
            .all(|cf| !(cf.kind == CFKind::Notional && cf.date == maturity)),
        "no notional may settle on the raw weekend maturity"
    );
}

/// PIK capitalizes on the lagged payment date before redemption, so the
/// balloon equals post-PIK outstanding and no residual-balance remains.
#[test]
fn pik_then_redemption_on_lagged_payment_date() {
    let issue = Date::from_calendar_date(2025, Month::January, 17).unwrap(); // Friday
    let maturity = Date::from_calendar_date(2026, Month::January, 17).unwrap(); // Saturday
    let redemption_date = Date::from_calendar_date(2026, Month::January, 21).unwrap();

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Pik,
        rate: Decimal::try_from(0.10).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 2,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000.0, Currency::USD).expect("valid money fixture");
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = builder.build(None).expect("PIK lagged schedule");

    let pik_total: f64 = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Pik)
        .map(|cf| cf.amount.amount())
        .sum();
    let last_pik_date = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Pik)
        .map(|cf| cf.date)
        .max()
        .expect("PIK flows");
    let redemption = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.kind == CFKind::Notional && cf.amount.amount() > 0.0)
        .expect("redemption flow");

    assert_eq!(last_pik_date, redemption_date);
    assert_eq!(redemption.date, redemption_date);
    assert!(
        (redemption.amount.amount() - (init.amount() + pik_total)).abs()
            < financial_tolerance(init.amount()),
        "redemption should equal post-PIK outstanding: got {} expected {}",
        redemption.amount.amount(),
        init.amount() + pik_total
    );
    let path = schedule.outstanding_by_date().unwrap();
    let final_outstanding = path.last().expect("outstanding path").1.amount();
    assert!(
        final_outstanding.abs() < financial_tolerance(init.amount()),
        "no residual outstanding after lagged PIK redemption, got {final_outstanding}"
    );
}

/// Lag-0 weekend maturity redeems on the adjusted business day (same as the
/// last coupon), not the Saturday raw maturity.
#[test]
fn lag_zero_weekend_maturity_redeems_on_adjusted_business_day() {
    let issue = Date::from_calendar_date(2025, Month::January, 17).unwrap(); // Friday
    let maturity = Date::from_calendar_date(2026, Month::January, 17).unwrap(); // Saturday
    let adjusted = Date::from_calendar_date(2026, Month::January, 19).unwrap(); // Monday

    let fixed = FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    };

    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let mut builder = CashFlowSchedule::builder();
    let _ = builder.principal(init, issue, maturity).fixed_cf(fixed);
    let schedule = builder.build(None).expect("lag-0 weekend schedule");

    let last_coupon_date = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .map(|cf| cf.date)
        .max()
        .expect("coupon flows");
    let redemption = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.kind == CFKind::Notional && cf.amount.amount() > 0.0)
        .expect("redemption flow");

    assert_eq!(last_coupon_date, adjusted);
    assert_eq!(redemption.date, adjusted);
    assert!(
        schedule.get_flows().iter().all(|cf| cf.date != maturity),
        "no flow may be dated on the unadjusted weekend maturity"
    );
}

fn principal_exchange_fixed_spec() -> FixedCouponSpec {
    FixedCouponSpec {
        coupon_type: CouponType::Cash,
        rate: Decimal::try_from(0.05).expect("valid"),
        schedule: finstack_quant_cashflows::builder::ScheduleParams {
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act365F,
            business_day_convention: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::specs::RollRule::None,
        },
    }
}

/// Default builder still emits issue funding and maturity redemption notionals.
#[test]
fn principal_exchange_default_emits_issue_and_redemption() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .fixed_cf(principal_exchange_fixed_spec());
    let schedule = builder.build(None).expect("default principal exchange");

    let notionals: Vec<_> = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Notional)
        .collect();
    assert_eq!(notionals.len(), 2, "default must emit issue + redemption");
    assert_eq!(notionals[0].date, issue);
    assert!(notionals[0].amount.amount() < 0.0);
    assert_eq!(notionals[1].date, maturity);
    assert!(
        (notionals[1].amount.amount() - init.amount()).abs() < financial_tolerance(init.amount())
    );
}

/// `PrincipalExchange::None` suppresses issue/redemption notionals but keeps
/// coupon amounts on the same outstanding path.
#[test]
fn principal_exchange_none_is_coupon_only() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let spec = principal_exchange_fixed_spec();

    let mut with_exchange = CashFlowSchedule::builder();
    let _ = with_exchange
        .principal(init, issue, maturity)
        .fixed_cf(spec.clone());
    let exchanged = with_exchange.build(None).expect("default exchange");

    let mut without_exchange = CashFlowSchedule::builder();
    let _ = without_exchange
        .principal(init, issue, maturity)
        .principal_exchange(PrincipalExchange::None)
        .fixed_cf(spec);
    let coupons_only = without_exchange.build(None).expect("no principal exchange");

    assert!(
        coupons_only
            .get_flows()
            .iter()
            .all(|cf| cf.kind != CFKind::Notional),
        "PrincipalExchange::None must not emit issue or redemption notionals"
    );
    let exchanged_coupons: Vec<_> = exchanged
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .collect();
    let none_coupons: Vec<_> = coupons_only
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::Fixed)
        .collect();
    assert_eq!(exchanged_coupons.len(), none_coupons.len());
    for (left, right) in exchanged_coupons.iter().zip(none_coupons.iter()) {
        assert_eq!(left.date, right.date);
        assert!(
            (left.amount.amount() - right.amount.amount()).abs()
                < financial_tolerance(init.amount()),
            "coupon amounts must match with or without notional exchange"
        );
    }
}

/// Scheduled amortization still emits when principal exchange is opted out.
#[test]
fn principal_exchange_none_keeps_amortization_flows() {
    let issue = Date::from_calendar_date(2025, Month::January, 15).unwrap();
    let maturity = Date::from_calendar_date(2026, Month::January, 15).unwrap();
    let init = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");

    let mut builder = CashFlowSchedule::builder();
    let _ = builder
        .principal(init, issue, maturity)
        .principal_exchange(PrincipalExchange::None)
        .amortization(AmortizationSpec::LinearTo {
            final_notional: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        })
        .fixed_cf(principal_exchange_fixed_spec());
    let schedule = builder.build(None).expect("amortizing no-exchange");

    assert!(
        schedule
            .get_flows()
            .iter()
            .any(|cf| cf.kind == CFKind::Amortization),
        "scheduled amortization must still emit"
    );
    assert!(
        schedule
            .get_flows()
            .iter()
            .all(|cf| cf.kind != CFKind::Notional),
        "opt-out must not add a residual notional balloon"
    );
}
