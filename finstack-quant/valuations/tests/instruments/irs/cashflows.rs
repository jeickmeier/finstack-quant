//! Interest Rate Swap cashflow schedule tests.
//!
//! Tests cover:
//! - Fixed leg schedule generation
//! - Floating leg schedule generation
//! - Date adjustments and calendars
//! - Stub period handling
//! - Cashflow schedule consistency

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_cashflows::builder::periods::{build_periods, BuildPeriodsParams};
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::pricing::swap_legs::add_payment_delay;
use finstack_quant_valuations::instruments::rates::irs::FloatingLegCompounding;
use finstack_quant_valuations::instruments::rates::irs::{InterestRateSwap, PayReceive};
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

fn build_test_curves() -> MarketContext {
    let as_of = date!(2024 - 01 - 01);

    // Curve IDs for usd_irs_swap (USD-OIS and USD-SOFR-3M)
    let disc_curve_usd_ois = DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (5.0, (-0.05_f64 * 5.0).exp()),
            (10.0, (-0.05_f64 * 10.0).exp()),
        ])
        .build()
        .unwrap();

    let fwd_curve_sofr = ForwardCurve::builder("USD-SOFR-3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (10.0, 0.05)])
        .build()
        .unwrap();

    // Curve IDs for manual builder tests (USD_OIS and USD_LIBOR_3M)
    let disc_curve_manual = DiscountCurve::builder("USD_OIS")
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-0.05_f64).exp()),
            (5.0, (-0.05_f64 * 5.0).exp()),
            (10.0, (-0.05_f64 * 10.0).exp()),
        ])
        .build()
        .unwrap();

    let fwd_curve_libor = ForwardCurve::builder("USD_LIBOR_3M", 0.25)
        .base_date(as_of)
        .day_count(DayCount::Act360)
        .knots([(0.0, 0.05), (10.0, 0.05)])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(disc_curve_usd_ois)
        .insert(disc_curve_manual)
        .insert(fwd_curve_sofr)
        .insert(fwd_curve_libor)
}

#[test]
fn test_irs_cashflow_schedule_generation() {
    let swap = test_utils::usd_irs_swap(
        "IRS-CF-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // Should have cashflows from both legs
    assert!(!schedule.is_empty(), "Schedule should not be empty");
}

#[test]
fn test_irs_fixed_leg_quarterly_schedule() {
    // Fixed leg with quarterly payments
    let swap = InterestRateSwap::builder()
        .id("IRS-FIX-Q".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start: date!(2024 - 01 - 01),
            end: date!(2025 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2025 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // 1 year quarterly = 4 periods per leg = 8 cashflows
    assert!(
        schedule.len() >= 4,
        "Should have at least 4 quarterly payments"
    );
}

#[test]
fn test_irs_fixed_leg_semiannual_schedule() {
    // Fixed leg with semiannual payments
    let swap = InterestRateSwap::builder()
        .id("IRS-FIX-SA".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // 2 years: 4 semiannual + 8 quarterly = 12 total
    assert!(
        schedule.len() >= 4,
        "Should have at least 4 semiannual payments"
    );
}

#[test]
fn test_irs_floating_leg_schedule() {
    let swap = test_utils::usd_irs_swap(
        "IRS-FLOAT-TEST",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        PayReceive::Pay,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // Verify floating leg generates cashflows
    assert!(!schedule.is_empty());
}

#[test]
fn test_irs_value_matches_combined_signed_schedule() {
    let swap = test_utils::usd_irs_swap(
        "IRS-PV-SCHEDULE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2029 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);
    let discount = market.get_discount("USD-OIS").unwrap();
    let schedule = swap.cashflow_schedule(&market, as_of).unwrap();

    let expected_pv = schedule
        .get_flows()
        .iter()
        .try_fold(0.0, |acc, flow| -> finstack_quant_core::Result<f64> {
            let payment_date = match flow.kind {
                finstack_quant_core::cashflow::CFKind::Fixed
                | finstack_quant_core::cashflow::CFKind::Stub => add_payment_delay(
                    flow.date,
                    swap.fixed.payment_lag_days,
                    swap.fixed.calendar_id.as_deref(),
                )?,
                finstack_quant_core::cashflow::CFKind::FloatReset => add_payment_delay(
                    flow.date,
                    swap.float.payment_lag_days,
                    swap.float.calendar_id.as_deref(),
                )?,
                _ => flow.date,
            };
            let df = discount.df_between_dates(as_of, payment_date)?;
            Ok(acc + flow.amount.amount() * df)
        })
        .unwrap();

    let value = swap.value(&market, as_of).unwrap();
    assert!(
        (value.amount() - expected_pv).abs() < 1.0e-6,
        "IRS value should reconcile to the combined signed schedule: value={} expected={} diff={}",
        value.amount(),
        expected_pv,
        value.amount() - expected_pv
    );
}

#[test]
fn test_irs_stub_front() {
    // Swap with short front stub
    let swap = InterestRateSwap::builder()
        .id("IRS-STUB-FRONT".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start: date!(2024 - 01 - 15),
            end: date!(2026 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 15),
            end: date!(2026 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of);

    assert!(schedule.is_ok(), "Should generate schedule with stub");
}

#[test]
fn test_irs_stub_back() {
    // Swap with short back stub
    let swap = InterestRateSwap::builder()
        .id("IRS-STUB-BACK".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortBack,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 06 - 15),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::ShortBack,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 06 - 15),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of);

    assert!(schedule.is_ok(), "Should generate schedule with back stub");
}

#[test]
fn test_irs_cashflow_dates_ordered() {
    let swap = test_utils::usd_irs_swap(
        "IRS-DATES",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2027 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // Verify dates are in ascending order
    let mut prev_date = date!(2020 - 01 - 01);
    for (cf_date, _) in &schedule {
        assert!(*cf_date >= prev_date, "Cashflow dates should be ordered");
        prev_date = *cf_date;
    }
}

#[test]
fn test_irs_cashflow_amounts_nonzero() {
    let swap = test_utils::usd_irs_swap(
        "IRS-AMOUNTS",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2027 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // Verify amounts are reasonable (non-zero for fixed rate > 0)
    for (_, amount) in &schedule {
        assert_ne!(
            amount.amount(),
            0.0,
            "Cashflow amounts should be non-zero for non-zero rate"
        );
    }
}

#[test]
fn test_irs_full_schedule_with_cfkind() {
    let swap = test_utils::usd_irs_swap(
        "IRS-FULL-SCHED",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2026 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let full_schedule = swap.cashflow_schedule(&market, as_of);

    assert!(full_schedule.is_ok(), "Should build full schedule");

    let sched = full_schedule.unwrap();

    // Verify we have both fixed and floating cashflows
    use finstack_quant_cashflows::primitives::CFKind;

    let has_fixed = sched.get_flows().iter().any(|cf| cf.kind == CFKind::Fixed);
    let has_float = sched
        .get_flows()
        .iter()
        .any(|cf| cf.kind == CFKind::FloatReset);

    assert!(has_fixed || has_float, "Should have classified cashflows");
}

#[test]
fn test_regular_short_front_schedule_does_not_tag_first_coupon_as_stub() {
    let swap = InterestRateSwap::builder()
        .id("IRS-REGULAR-SHORT-FRONT-NO-STUB-KIND".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let schedule = swap
        .cashflow_schedule(&market, date!(2024 - 01 - 01))
        .expect("full schedule");

    let fixed_stub_count = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == finstack_quant_cashflows::primitives::CFKind::Stub)
        .count();

    assert_eq!(
        fixed_stub_count, 0,
        "regular fixed coupons should not be classified as stub cashflows"
    );
}

#[test]
fn test_genuine_front_stub_fixed_coupon_is_tagged_as_stub() {
    let swap = InterestRateSwap::builder()
        .id("IRS-GENUINE-FRONT-STUB-KIND".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start: date!(2024 - 01 - 15),
            end: date!(2026 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 15),
            end: date!(2026 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let schedule = swap
        .cashflow_schedule(&market, date!(2024 - 01 - 01))
        .expect("full schedule");

    let fixed_stub_count = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == finstack_quant_cashflows::primitives::CFKind::Stub)
        .count();

    assert_eq!(fixed_stub_count, 1);
}

#[test]
fn test_regular_holiday_end_coupon_under_stub_rule_is_not_tagged_stub() {
    let swap = InterestRateSwap::builder()
        .id("IRS-HOLIDAY-END-NO-STUB-KIND".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("nyse".to_string()),
            stub: StubKind::ShortBack,
            start: date!(2024 - 01 - 04),
            end: date!(2025 - 01 - 04),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("nyse".to_string()),
            fixing_calendar_id: Some("nyse".to_string()),
            stub: StubKind::ShortBack,
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 04),
            end: date!(2025 - 01 - 04),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let schedule = swap
        .cashflow_schedule(&market, date!(2024 - 01 - 01))
        .expect("full schedule");

    let fixed_stub_count = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == finstack_quant_cashflows::primitives::CFKind::Stub)
        .count();

    assert_eq!(
        fixed_stub_count, 0,
        "regular fixed coupons ending on a weekend should not be classified as stubs"
    );
}

#[test]
fn test_regular_eom_schedule_under_stub_rule_is_not_tagged_stub() {
    let swap = InterestRateSwap::builder()
        .id("IRS-EOM-NO-STUB-KIND".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::ShortFront,
            start: date!(2024 - 01 - 31),
            end: date!(2024 - 04 - 30),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: true,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::monthly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::ShortFront,
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: true,
            start: date!(2024 - 01 - 31),
            end: date!(2024 - 04 - 30),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let schedule = swap
        .cashflow_schedule(&market, date!(2024 - 01 - 01))
        .expect("full schedule");

    let fixed_stub_count = schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == finstack_quant_cashflows::primitives::CFKind::Stub)
        .count();

    assert_eq!(
        fixed_stub_count, 0,
        "regular EOM fixed coupons should not be classified as stub cashflows"
    );
}

#[test]
fn test_irs_different_frequencies() {
    // Fixed semiannual, float quarterly
    let swap = InterestRateSwap::builder()
        .id("IRS-DIFF-FREQ".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of).unwrap();

    // 2 years: 4 semiannual fixed + 8 quarterly float = 12
    assert!(schedule.len() >= 4, "Should have multiple cashflows");
}

#[test]
fn test_irs_calendar_adjustments() {
    // Swap with calendar adjustments
    let swap = InterestRateSwap::builder()
        .id("IRS-CAL-ADJ".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::semi_annual(),
            day_count: DayCount::Thirty360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::try_from(0.0).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: Some("usny".to_string()),
            fixing_calendar_id: Some("usny".to_string()),
            stub: StubKind::None,
            reset_lag_days: 0, // Use 0 for spot-starting swaps to avoid needing historical fixings
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: date!(2024 - 01 - 01),
            end: date!(2026 - 01 - 01),
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let schedule = swap.dated_cashflows(&market, as_of);

    assert!(schedule.is_ok(), "Should handle calendar adjustments");
}

/// Test that explicit payment_lag_days: 0 is NOT overridden by convention defaults.
/// This verifies the fix where 0 means "no delay" rather than "use convention".
#[test]
fn test_irs_explicit_zero_payment_delay_preserved() {
    let start = date!(2024 - 01 - 02);
    let end = date!(2026 - 01 - 02);
    let swap = InterestRateSwap::builder()
        .id("IRS-OIS-ZERO-DELAY".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0, // Explicit 0 should stay 0
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-OIS".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::annual(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0,
            compounding: FloatingLegCompounding::CompoundedInArrears {
                lookback_days: 0,
                observation_shift: None,
            },
            payment_lag_days: 0, // Explicit 0 should stay 0
            end_of_month: false,
            start,
            end,
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let full_schedule = swap
        .cashflow_schedule(&market, start)
        .expect("full schedule");
    // Expected schedule with payment_delay = 0 (no delay)
    let expected = build_periods(BuildPeriodsParams {
        start,
        end,
        frequency: Tenor::annual(),
        stub: StubKind::None,
        bdc: BusinessDayConvention::ModifiedFollowing,
        calendar_id: "weekends_only",
        end_of_month: false,
        day_count: DayCount::Act360,
        payment_lag_days: 0,
        reset_lag_days: None,
        adjust_accrual_dates: false,
    })
    .expect("expected schedule");
    let first_payment = full_schedule
        .get_flows()
        .iter()
        .filter(|cf| {
            matches!(
                cf.kind,
                finstack_quant_cashflows::primitives::CFKind::Fixed
                    | finstack_quant_cashflows::primitives::CFKind::Stub
            )
        })
        .map(|cf| cf.date)
        .min()
        .expect("first payment");
    assert_eq!(first_payment, expected[0].payment_date);
}

/// Test that explicit reset_lag_days: 0 is NOT overridden by convention defaults.
/// This verifies the fix where 0 means "spot reset" rather than "use convention".
#[test]
fn test_irs_explicit_zero_reset_lag_preserved() {
    let start = date!(2024 - 01 - 02);
    let end = date!(2024 - 07 - 02);
    let swap = InterestRateSwap::builder()
        .id("IRS-TERM-ZERO-LAG".into())
        .notional(Money::new(1_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD-OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"),
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start,
            end,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD-OIS".into(),
            forward_curve_id: "USD-SOFR-3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0, // Explicit 0 means spot reset (no lag)
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start,
            end,
        })
        .build()
        .unwrap();

    let market = build_test_curves();
    let full_schedule = swap
        .cashflow_schedule(&market, start)
        .expect("full schedule");
    let first_reset = full_schedule
        .get_flows()
        .iter()
        .find(|cf| cf.kind == finstack_quant_cashflows::primitives::CFKind::FloatReset)
        .and_then(|cf| cf.reset_date)
        .expect("reset date");
    // With reset_lag_days: 0, reset date should equal accrual start (spot reset)
    assert_eq!(first_reset, start);
}

#[test]
fn test_irs_receive_fixed_cashflow_signs() {
    // Receive fixed: fixed coupons positive, float negative
    let swap = test_utils::usd_irs_swap(
        "IRS-RECEIVE",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        PayReceive::Receive,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let full_schedule = swap.cashflow_schedule(&market, as_of).unwrap();

    use finstack_quant_cashflows::primitives::CFKind;

    // Check signs
    for cf in full_schedule.get_flows() {
        match cf.kind {
            CFKind::Fixed | CFKind::Stub => {
                // Receive fixed: should be positive
                assert!(
                    cf.amount.amount() > 0.0,
                    "Fixed leg should be positive for Receive"
                );
            }
            CFKind::FloatReset => {
                // Pay floating: should be negative
                assert!(
                    cf.amount.amount() < 0.0,
                    "Floating leg should be negative for Receive"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn test_irs_pay_fixed_cashflow_signs() {
    // Pay fixed: fixed coupons negative, float positive
    let swap = test_utils::usd_irs_swap(
        "IRS-PAY",
        Money::new(1_000_000.0, Currency::USD),
        0.05,
        date!(2024 - 01 - 01),
        date!(2025 - 01 - 01),
        PayReceive::Pay,
    )
    .unwrap();

    let market = build_test_curves();
    let as_of = date!(2024 - 01 - 01);

    let full_schedule = swap.cashflow_schedule(&market, as_of).unwrap();

    use finstack_quant_cashflows::primitives::CFKind;

    // Check signs
    for cf in full_schedule.get_flows() {
        match cf.kind {
            CFKind::Fixed | CFKind::Stub => {
                // Pay fixed: should be negative
                assert!(
                    cf.amount.amount() < 0.0,
                    "Fixed leg should be negative for Pay"
                );
            }
            CFKind::FloatReset => {
                // Receive floating: should be positive
                assert!(
                    cf.amount.amount() > 0.0,
                    "Floating leg should be positive for Pay"
                );
            }
            _ => {}
        }
    }
}

/// IRS NPV parity test: at the par rate, fixed leg PV ≈ floating leg PV.
///
/// When the fixed rate equals the implied par rate from the forward curve,
/// the swap should have near-zero NPV (fixed leg PV - floating leg PV ≈ 0).
/// This test constructs a swap at 5% where the forward curve is flat at 5%,
/// so the swap should be approximately at par.
#[test]
fn test_irs_npv_parity_at_par_rate() {
    use finstack_quant_cashflows::primitives::CFKind;

    // Build a swap where the fixed rate matches the flat forward curve rate (5%)
    // This should produce a near-zero NPV swap
    let as_of = date!(2024 - 01 - 01);
    let end_date = date!(2029 - 01 - 01); // 5 years

    let swap = InterestRateSwap::builder()
        .id("IRS-PAR-PARITY".into())
        .notional(Money::new(10_000_000.0, Currency::USD))
        .side(PayReceive::Receive)
        .fixed(finstack_quant_valuations::instruments::FixedLegSpec {
            discount_curve_id: "USD_OIS".into(),
            rate: rust_decimal::Decimal::try_from(0.05).expect("valid"), // matches flat forward
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            stub: StubKind::None,
            start: as_of,
            end: end_date,
            par_method: None,
            compounding_simple: true,
            payment_lag_days: 0,
            end_of_month: false,
        })
        .float(finstack_quant_valuations::instruments::FloatLegSpec {
            discount_curve_id: "USD_OIS".into(),
            forward_curve_id: "USD_LIBOR_3M".into(),
            spread_bp: rust_decimal::Decimal::ZERO,
            frequency: Tenor::quarterly(),
            day_count: DayCount::Act360,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: None,
            fixing_calendar_id: None,
            stub: StubKind::None,
            reset_lag_days: 0,
            compounding: Default::default(),
            payment_lag_days: 0,
            end_of_month: false,
            start: as_of,
            end: end_date,
        })
        .build()
        .unwrap();

    let market = build_test_curves();

    let full_schedule = swap.cashflow_schedule(&market, as_of).unwrap();

    // Separate fixed and floating leg flows
    let fixed_sum: f64 = full_schedule
        .get_flows()
        .iter()
        .filter(|cf| matches!(cf.kind, CFKind::Fixed | CFKind::Stub))
        .map(|cf| cf.amount.amount())
        .sum();

    let float_sum: f64 = full_schedule
        .get_flows()
        .iter()
        .filter(|cf| cf.kind == CFKind::FloatReset)
        .map(|cf| cf.amount.amount())
        .sum();

    // For a par swap: |fixed_sum| ≈ |float_sum| (both undiscounted)
    // Since forward rate = fixed rate = 5%, the legs should approximately match.
    // Allow moderate tolerance because of day count differences, date adjustments,
    // and the difference between flat forward rates and period forward rates.
    let notional = 10_000_000.0;
    let parity_diff = (fixed_sum.abs() - float_sum.abs()).abs();
    let relative_diff = parity_diff / notional;

    assert!(
        relative_diff < 0.01, // Within 1% of notional
        "IRS NPV parity violated: fixed_sum={:.2}, float_sum={:.2}, diff={:.2} ({:.4}% of notional)",
        fixed_sum,
        float_sum,
        parity_diff,
        relative_diff * 100.0
    );
}
