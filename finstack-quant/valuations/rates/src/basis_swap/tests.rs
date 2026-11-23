use super::types::*;
use crate::FloatLegSpec;
use finstack_quant_cashflows::CashflowProvider;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{BusinessDayConvention, Date, DayCount, StubKind, Tenor};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::market_data::traits::Discounting;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CurveId, InstrumentId};
use finstack_quant_valuations_core::instruments::Instrument;
use rust_decimal::Decimal;
use time::Month;

// Helper function for tests
fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).expect("valid date"), day)
        .expect("should succeed")
}

#[test]
fn test_basis_swap_pricing() {
    let base_date = date(2024, 1, 1);
    let start_date = date(2024, 1, 3);
    let maturity = date(2025, 1, 3);

    // Create test curves with flat rates
    let discount_curve = DiscountCurve::builder("OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");

    let forward_3m = ForwardCurve::flat(
        "3M-SOFR",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("should succeed");

    let forward_6m = ForwardCurve::flat(
        "6M-SOFR",
        base_date,
        0.0305,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("should succeed");

    // Create context
    let context = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve")
        .try_insert(forward_3m)
        .expect("insert market curve")
        .try_insert(forward_6m)
        .expect("insert market curve");

    // Create basis swap: 3M receives 6M + 5bp
    let primary_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::from(5), // 5bp
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let reference_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let swap = BasisSwap::new(
        "TEST_BASIS",
        Money::from((1_000_000_i64, Currency::USD)),
        primary_leg,
        reference_leg,
    )
    .expect("should succeed");

    // Price the swap
    let pv = swap.value(&context, base_date).expect("should succeed");

    // The 3M leg has rate 3.00% while 6M leg has rate 3.05%.
    // With a 5bp (0.0005) spread on the primary (3M) leg, the PV reflects the basis.
    // Expected PV ≈ (primary_rate + spread - reference_rate) × notional × annuity
    // ≈ (0.03 + 0.0005 - 0.0305) × 1_000_000 × ~1 ≈ -450 to -500
    // Use 600 tolerance to account for day count and discounting effects.
    assert!(
        pv.amount().abs() < 600.0,
        "PV should be small for near-par swap: {}",
        pv.amount()
    );
}

#[test]
fn test_basis_swap_requires_calendar_by_default() {
    let base_date = date(2024, 1, 1);
    let start_date = date(2024, 1, 3);
    let maturity = date(2025, 1, 3);

    let discount_curve = DiscountCurve::builder("OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.99)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");
    let forward_3m = ForwardCurve::flat(
        "3M-SOFR",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("should succeed");
    let forward_6m = ForwardCurve::flat(
        "6M-SOFR",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("should succeed");

    let context = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve")
        .try_insert(forward_3m)
        .expect("insert market curve")
        .try_insert(forward_6m)
        .expect("insert market curve");

    let primary_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };
    let reference_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let swap = BasisSwap::builder()
        .id("TEST_BASIS_NO_CAL".into())
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .primary_leg(primary_leg)
        .reference_leg(reference_leg)
        .allow_calendar_fallback(true)
        .attributes(Default::default())
        .build()
        .expect("should succeed");

    let pv = swap.value(&context, base_date).expect("should succeed");
    assert!(
        pv.amount().is_finite(),
        "Expected finite PV when defaulting to weekends_only"
    );
}

#[test]
fn test_basis_swap_payment_lag_affects_pv() {
    let base_date = date(2024, 1, 1);
    let start_date = date(2024, 1, 3);
    let maturity = date(2025, 1, 3);

    // Use a steep-ish discount curve so payment timing matters.
    let discount_curve = DiscountCurve::builder("OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.90), (2.0, 0.82)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");

    let forward_3m = ForwardCurve::flat(
        "3M-SOFR",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("should succeed");
    let forward_6m = ForwardCurve::flat(
        "6M-SOFR",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("should succeed");

    let context = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve")
        .try_insert(forward_3m)
        .expect("insert market curve")
        .try_insert(forward_6m)
        .expect("insert market curve");

    let primary_leg_no_lag = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::from(10),
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };
    let primary_leg_with_lag = FloatLegSpec {
        payment_lag_days: 10,
        ..primary_leg_no_lag.clone()
    };

    let reference_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let swap_no_lag = BasisSwap::new(
        "TEST_BASIS_NO_LAG",
        Money::from((10_000_000_i64, Currency::USD)),
        primary_leg_no_lag,
        reference_leg.clone(),
    )
    .expect("should succeed");

    let swap_with_lag = BasisSwap::new(
        "TEST_BASIS_WITH_LAG",
        Money::from((10_000_000_i64, Currency::USD)),
        primary_leg_with_lag,
        reference_leg,
    )
    .expect("should succeed");

    let pv_no_lag = swap_no_lag
        .value(&context, base_date)
        .expect("should succeed");
    let pv_with_lag = swap_with_lag
        .value(&context, base_date)
        .expect("should succeed");

    assert!(
        (pv_no_lag.amount() - pv_with_lag.amount()).abs() > 1e-6,
        "Expected payment lag to change PV: no_lag={}, with_lag={}",
        pv_no_lag.amount(),
        pv_with_lag.amount()
    );
}

#[test]
fn test_basis_swap_rejects_invalid_dates() {
    let base_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: date(2024, 1, 3),
        end: date(2025, 1, 3),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    // Test start == end (equal dates)
    let primary_eq = FloatLegSpec {
        start: date(2024, 1, 3),
        end: date(2024, 1, 3),
        ..base_leg.clone()
    };
    let reference_eq = FloatLegSpec {
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        ..primary_eq.clone()
    };
    let err = BasisSwap::new(
        "INVALID_DATES",
        Money::from((1_000_000_i64, Currency::USD)),
        primary_eq,
        reference_eq,
    )
    .expect_err("should fail for equal dates");
    assert!(
        format!("{err}").contains("start") && format!("{err}").contains("positive tenor"),
        "Expected date validation error, got: {err}"
    );

    // Test start > end (inverted dates)
    let primary_inv = FloatLegSpec {
        start: date(2025, 1, 3),
        end: date(2024, 1, 3),
        ..base_leg
    };
    let reference_inv = FloatLegSpec {
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        ..primary_inv.clone()
    };
    let err = BasisSwap::new(
        "INVERTED_DATES",
        Money::from((1_000_000_i64, Currency::USD)),
        primary_inv,
        reference_inv,
    )
    .expect_err("should fail for inverted dates");
    assert!(
        format!("{err}").contains("positive tenor"),
        "Expected positive tenor error, got: {err}"
    );
}

#[test]
fn test_basis_swap_rejects_same_curve_by_default() {
    let leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        discount_curve_id: CurveId::new("OIS"),
        start: date(2024, 1, 3),
        end: date(2025, 1, 3),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::from(5),
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let err = BasisSwap::new(
        "SAME_CURVE",
        Money::from((1_000_000_i64, Currency::USD)),
        leg.clone(),
        FloatLegSpec {
            spread_bp: Decimal::ZERO,
            ..leg
        },
    )
    .expect_err("should fail for same forward curve");

    assert!(
        format!("{err}").contains("identical forward curves"),
        "Expected same-curve error, got: {err}"
    );
}

#[test]
fn test_basis_swap_allows_same_curve_when_explicit() {
    let base_date = date(2024, 1, 1);

    let discount_curve = DiscountCurve::builder("OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");
    let forward = ForwardCurve::flat(
        "USD-SOFR-3M",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("should succeed");

    let context = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve")
        .try_insert(forward)
        .expect("insert market curve");

    let leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        discount_curve_id: CurveId::new("OIS"),
        start: date(2024, 1, 3),
        end: date(2025, 1, 3),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::from(10), // 10bp spread
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    // Same-curve trades opt in through the builder's `allow_same_curve` flag.
    let swap = BasisSwap::builder()
        .id(InstrumentId::new("SAME_CURVE_OK"))
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .primary_leg(leg.clone())
        .reference_leg(FloatLegSpec {
            spread_bp: Decimal::ZERO,
            ..leg
        })
        .allow_same_curve(true)
        .attributes(Default::default())
        .build()
        .expect("should succeed with explicit allow");

    // Should price successfully
    let pv = swap.value(&context, base_date).expect("should succeed");

    // For same-curve swap, PV = spread × notional × annuity
    // With 10bp spread on $1M notional for ~1Y, expect PV ≈ 1000 × 0.0010 × ~0.98 ≈ ~980
    assert!(
        (pv.amount() - 980.0).abs() < 100.0,
        "Same-curve swap PV should be spread × annuity: {}",
        pv.amount()
    );
}

#[test]
fn test_par_spread_zeroes_npv() {
    use finstack_quant_valuations_core::instruments::Instrument;
    use finstack_quant_valuations_core::metrics::MetricId;

    let base_date = date(2024, 1, 1);
    let start_date = date(2024, 1, 3);
    let maturity = date(2026, 1, 3); // 2Y swap for more interesting basis

    // Create curves with a basis between 3M and 6M
    let discount_curve = DiscountCurve::builder("OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.97), (2.0, 0.94), (3.0, 0.91)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");

    let forward_3m = ForwardCurve::from_zero_rates(
        "3M-SOFR",
        base_date,
        &[(0.0, 0.030), (1.0, 0.032), (2.0, 0.034)],
        finstack_quant_core::math::Compounding::Continuous,
        finstack_quant_core::dates::DayCount::Act360,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .expect("should succeed");

    let forward_6m = ForwardCurve::from_zero_rates(
        "6M-SOFR",
        base_date,
        &[(0.0, 0.031), (1.0, 0.033), (2.0, 0.035)],
        finstack_quant_core::math::Compounding::Continuous,
        finstack_quant_core::dates::DayCount::Act360,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .expect("should succeed");

    let context = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve")
        .try_insert(forward_3m)
        .expect("insert market curve")
        .try_insert(forward_6m)
        .expect("insert market curve");

    // Create swap with zero spread initially
    let primary_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO, // Start at zero spread
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let reference_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::semi_annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };

    let swap = BasisSwap::new(
        "PAR_SPREAD_TEST",
        Money::from((10_000_000_i64, Currency::USD)),
        primary_leg.clone(),
        reference_leg.clone(),
    )
    .expect("should succeed");

    // Get the par spread using metrics
    let result = swap
        .price_with_metrics(
            &context,
            base_date,
            &[MetricId::BasisParSpread, MetricId::AnnuityPrimary],
            finstack_quant_valuations_core::instruments::PricingOptions::default(),
        )
        .expect("should succeed");

    let par_spread_bp = *result
        .measures
        .get(MetricId::BasisParSpread.as_str())
        .expect("should have par spread");

    // Create a new swap with the par spread applied
    let primary_leg_at_par = FloatLegSpec {
        spread_bp: Decimal::try_from(par_spread_bp).expect("metric result is finite"),
        ..primary_leg
    };

    let swap_at_par = BasisSwap::new(
        "PAR_SPREAD_VERIFY",
        Money::from((10_000_000_i64, Currency::USD)),
        primary_leg_at_par,
        reference_leg,
    )
    .expect("should succeed");

    // Verify NPV is now zero
    let pv_at_par = swap_at_par
        .value(&context, base_date)
        .expect("should succeed");

    // NPV should be very close to zero (within rounding tolerance)
    assert!(
        pv_at_par.amount().abs() < 1.0, // Less than $1 on a $10M notional
        "Swap at par spread should have ~zero NPV: got {} (par spread was {:.2}bp)",
        pv_at_par.amount(),
        par_spread_bp
    );
}

#[test]
fn test_basis_swap_rejects_negative_lags() {
    let valid_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: date(2024, 1, 3),
        end: date(2025, 1, 3),
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };
    let reference_leg = FloatLegSpec {
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        ..valid_leg.clone()
    };

    // Test negative payment_lag_days on primary leg
    let primary_neg_payment = FloatLegSpec {
        payment_lag_days: -1,
        ..valid_leg.clone()
    };
    let err = BasisSwap::new(
        "NEG_PAY_PRIMARY",
        Money::from((1_000_000_i64, Currency::USD)),
        primary_neg_payment,
        reference_leg.clone(),
    )
    .expect_err("should fail for negative payment lag");
    assert!(
        format!("{err}").contains("payment_lag_days") && format!("{err}").contains("non-negative"),
        "Expected negative payment lag error, got: {err}"
    );

    // Test negative reset_lag_days on primary leg
    let primary_neg_reset = FloatLegSpec {
        reset_lag_days: -1,
        ..valid_leg.clone()
    };
    let err = BasisSwap::new(
        "NEG_RESET_PRIMARY",
        Money::from((1_000_000_i64, Currency::USD)),
        primary_neg_reset,
        reference_leg.clone(),
    )
    .expect_err("should fail for negative reset lag");
    assert!(
        format!("{err}").contains("reset_lag_days") && format!("{err}").contains("non-negative"),
        "Expected negative reset lag error, got: {err}"
    );

    // Test negative payment_lag_days on reference leg
    let ref_neg_payment = FloatLegSpec {
        payment_lag_days: -2,
        ..reference_leg.clone()
    };
    let err = BasisSwap::new(
        "NEG_PAY_REF",
        Money::from((1_000_000_i64, Currency::USD)),
        valid_leg.clone(),
        ref_neg_payment,
    )
    .expect_err("should fail for negative reference leg payment lag");
    assert!(
        format!("{err}").contains("payment_lag_days") && format!("{err}").contains("reference leg"),
        "Expected negative reference leg payment lag error, got: {err}"
    );

    // Test negative reset_lag_days on reference leg
    let ref_neg_reset = FloatLegSpec {
        reset_lag_days: -3,
        ..reference_leg
    };
    let err = BasisSwap::new(
        "NEG_RESET_REF",
        Money::from((1_000_000_i64, Currency::USD)),
        valid_leg,
        ref_neg_reset,
    )
    .expect_err("should fail for negative reference leg reset lag");
    assert!(
        format!("{err}").contains("reset_lag_days") && format!("{err}").contains("reference leg"),
        "Expected negative reference leg reset lag error, got: {err}"
    );
}

#[test]
fn test_basis_swap_cashflow_provider_emits_signed_leg_flows() {
    let base_date = date(2024, 1, 1);
    let start_date = date(2024, 1, 3);
    let maturity = date(2025, 1, 3);
    let discount_curve = DiscountCurve::builder("OIS")
        .base_date(base_date)
        .knots(vec![(0.0, 1.0), (1.0, 0.98), (2.0, 0.96)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("should succeed");
    let forward_3m = ForwardCurve::flat(
        "3M-SOFR",
        base_date,
        0.03,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("should succeed");
    let forward_6m = ForwardCurve::flat(
        "6M-SOFR",
        base_date,
        0.031,
        finstack_quant_core::dates::DayCount::Act365F,
    )
    .expect("should succeed");
    let context = MarketContext::new()
        .try_insert(discount_curve)
        .expect("insert market curve")
        .try_insert(forward_3m)
        .expect("insert market curve")
        .try_insert(forward_6m)
        .expect("insert market curve");

    let primary_leg = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "3M-SOFR".into(),
        forward_curve_id: CurveId::new("3M-SOFR"),
        discount_curve_id: CurveId::new("OIS"),
        start: start_date,
        end: maturity,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: None,
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: Default::default(),
    };
    let reference_leg = FloatLegSpec {
        index_id: "6M-SOFR".into(),
        forward_curve_id: CurveId::new("6M-SOFR"),
        frequency: Tenor::semi_annual(),
        ..primary_leg.clone()
    };
    let swap = BasisSwap::builder()
        .id("BASIS-CF".into())
        .notional(Money::from((1_000_000_i64, Currency::USD)))
        .primary_leg(primary_leg)
        .reference_leg(reference_leg)
        .allow_calendar_fallback(true)
        .attributes(Default::default())
        .build()
        .expect("should succeed");

    let flows = swap
        .dated_cashflows(&context, base_date)
        .expect("basis swap contractual schedule should build");

    assert!(!flows.is_empty(), "basis swap should emit coupon cashflows");
    assert!(flows.iter().any(|(_, money)| money.amount() > 0.0));
    assert!(flows.iter().any(|(_, money)| money.amount() < 0.0));
}

#[test]
fn overnight_rfr_leg_matches_shared_compounding_engine() {
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::dates::calendar_by_id_strict;
    use finstack_quant_valuations_core::instruments::pricing::overnight::{
        OvernightCouponProjectionInput, OvernightProjectionCurve, project_overnight_coupon,
    };

    let start = date(2025, 1, 2);
    let end = date(2026, 1, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .knots(vec![(0.0, 1.0), (1.0, 0.96)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount");
    let ois = ForwardCurve::flat(
        "USD-SOFR-OIS",
        start,
        0.04,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("ois forward");
    let term = ForwardCurve::flat(
        "USD-SOFR-3M",
        start,
        0.041,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("term forward");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(ois.clone())
        .expect("insert market curve")
        .try_insert(term)
        .expect("insert market curve");

    let primary = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "USD-SOFR-OIS".into(),
        forward_curve_id: CurveId::new("USD-SOFR-OIS"),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
    };
    let reference = FloatLegSpec {
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        compounding: FloatingLegCompounding::Simple,
        ..primary.clone()
    };
    let swap = BasisSwap::new(
        "SOFR-OIS-3M",
        Money::from((10_000_000_i64, Currency::USD)),
        primary,
        reference,
    )
    .expect("overnight vs term basis");

    let schedule = swap
        .floating_leg_schedule(&swap.primary_leg, &market, start)
        .expect("overnight schedule");
    let first = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.get_kind() == finstack_quant_core::cashflow::CFKind::Floating)
        .expect("overnight coupon");
    let calendar = calendar_by_id_strict("usny").expect("usny");
    let periods = finstack_quant_cashflows::builder::build_periods(
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start,
            end,
            frequency: Tenor::quarterly(),
            stub: StubKind::ShortFront,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: Some(0),
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    )
    .expect("periods");
    let first_period = periods.first().expect("first period");
    let (accrual_start, accrual_end) =
        finstack_quant_valuations_core::instruments::pricing::overnight::adjust_overnight_accrual_boundaries(
            first_period.accrual_start,
            first_period.accrual_end,
            BusinessDayConvention::ModifiedFollowing,
            calendar,
        )
        .expect("accrual window");
    let expected = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&ois),
        fixings: None,
        fixing_id: "USD-SOFR-OIS",
        as_of: start,
        accrual_start,
        accrual_end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: Some(Tenor::quarterly()),
        coupon_period: (accrual_start, accrual_end),
        compounding: &FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("shared engine");
    let expected_amount = 10_000_000.0 * (expected.compound_factor - 1.0);
    assert!(
        (first.get_amount().amount() - expected_amount).abs() < 1e-6,
        "basis overnight coupon {} != shared engine {expected_amount}",
        first.get_amount().amount()
    );
}

#[test]
fn overnight_rfr_honors_distinct_fixing_calendar() {
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::dates::calendar_by_id_strict;
    use finstack_quant_valuations_core::instruments::pricing::overnight::{
        OvernightCouponProjectionInput, OvernightProjectionCurve, project_overnight_coupon,
    };

    let start = date(2025, 1, 2);
    let end = date(2026, 1, 2);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .knots(vec![(0.0, 1.0), (1.0, 0.96)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount");
    let ois = ForwardCurve::from_zero_rates(
        "USD-SOFR-OIS",
        start,
        &[(0.0, 0.04), (1.0, 0.08)],
        finstack_quant_core::math::Compounding::Continuous,
        finstack_quant_core::dates::DayCount::Act360,
        finstack_quant_core::math::interp::InterpStyle::LogLinear,
        finstack_quant_core::math::interp::ExtrapolationPolicy::Extend,
    )
    .expect("ois forward");
    let term = ForwardCurve::flat(
        "USD-SOFR-3M",
        start,
        0.041,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("term forward");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(ois.clone())
        .expect("insert market curve")
        .try_insert(term)
        .expect("insert market curve");

    let primary = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: Some("weekends_only".into()),
        index_id: "USD-SOFR-OIS".into(),
        forward_curve_id: CurveId::new("USD-SOFR-OIS"),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
    };
    let reference = FloatLegSpec {
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        compounding: FloatingLegCompounding::Simple,
        ..primary.clone()
    };
    let swap = BasisSwap::new(
        "SOFR-OIS-3M",
        Money::from((10_000_000_i64, Currency::USD)),
        primary,
        reference,
    )
    .expect("overnight vs term basis");

    let schedule = swap
        .floating_leg_schedule(&swap.primary_leg, &market, start)
        .expect("overnight schedule");
    let first = schedule
        .get_flows()
        .iter()
        .find(|cf| cf.get_kind() == finstack_quant_core::cashflow::CFKind::Floating)
        .expect("overnight coupon");
    let calendar = calendar_by_id_strict("weekends_only").expect("fixing calendar");
    let periods = finstack_quant_cashflows::builder::build_periods(
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start,
            end,
            frequency: Tenor::quarterly(),
            stub: StubKind::ShortFront,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: Some(0),
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    )
    .expect("periods");
    let first_period = periods.first().expect("first period");
    let (accrual_start, accrual_end) =
        finstack_quant_valuations_core::instruments::pricing::overnight::adjust_overnight_accrual_boundaries(
            first_period.accrual_start,
            first_period.accrual_end,
            BusinessDayConvention::ModifiedFollowing,
            calendar,
        )
        .expect("accrual window");
    let expected = project_overnight_coupon(OvernightCouponProjectionInput {
        curve: OvernightProjectionCurve::Forward(&ois),
        fixings: None,
        fixing_id: "USD-SOFR-OIS",
        as_of: start,
        accrual_start,
        accrual_end,
        end_is_termination_date: None,
        day_count: DayCount::Act360,
        coupon_frequency: Some(Tenor::quarterly()),
        coupon_period: (accrual_start, accrual_end),
        compounding: &FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
        fixing_calendar: calendar,
        compounded_spread: 0.0,
        need_observation_exposures: false,
    })
    .expect("shared engine");
    let expected_amount = 10_000_000.0 * (expected.compound_factor - 1.0);
    assert!(
        (first.get_amount().amount() - expected_amount).abs() < 1e-6,
        "basis overnight coupon {} != shared engine {expected_amount}",
        first.get_amount().amount()
    );
}

#[test]
fn overnight_annuity_uses_projector_year_fraction() {
    use finstack_quant_cashflows::builder::FloatingLegCompounding;
    use finstack_quant_core::dates::calendar_by_id_strict;

    let start = date(2025, 7, 4);
    let end = date(2025, 10, 6);
    let disc = DiscountCurve::builder("USD-OIS")
        .base_date(start)
        .knots(vec![(0.0, 1.0), (1.0, 0.96)])
        .day_count(finstack_quant_core::dates::DayCount::Act365F)
        .interp(finstack_quant_core::math::interp::InterpStyle::MonotoneConvex)
        .extrapolation(finstack_quant_core::math::interp::ExtrapolationPolicy::Extend)
        .build()
        .expect("discount");
    let ois = ForwardCurve::flat(
        "USD-SOFR-OIS",
        start,
        0.04,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("ois forward");
    let term = ForwardCurve::flat(
        "USD-SOFR-3M",
        start,
        0.041,
        finstack_quant_core::dates::DayCount::Act360,
    )
    .expect("term forward");
    let market = MarketContext::new()
        .try_insert(disc)
        .expect("insert market curve")
        .try_insert(ois)
        .expect("insert market curve")
        .try_insert(term)
        .expect("insert market curve");
    let primary = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "USD-SOFR-OIS".into(),
        forward_curve_id: CurveId::new("USD-SOFR-OIS"),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::annual(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
    };
    let reference = FloatLegSpec {
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        compounding: FloatingLegCompounding::Simple,
        ..primary.clone()
    };
    let swap = BasisSwap::new(
        "SOFR-OIS-ANN",
        Money::from((10_000_000_i64, Currency::USD)),
        primary,
        reference,
    )
    .expect("overnight vs term basis");
    let annuity = swap
        .annuity_for_leg(&swap.primary_leg, &market, start)
        .expect("overnight annuity");
    let periods = finstack_quant_cashflows::builder::build_periods(
        finstack_quant_cashflows::builder::BuildPeriodsParams {
            start,
            end,
            frequency: Tenor::annual(),
            stub: StubKind::ShortFront,
            business_day_convention: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "usny",
            end_of_month: false,
            day_count: DayCount::Act360,
            payment_lag_days: 0,
            reset_lag_days: Some(0),
            adjust_accrual_dates: false,
            roll_rule: finstack_quant_cashflows::builder::RollRule::None,
        },
    )
    .expect("periods");
    let calendar = calendar_by_id_strict("usny").expect("usny");
    let disc = market.get_discount("USD-OIS").expect("discount");
    let fwd = market.get_forward("USD-SOFR-OIS").expect("forward");
    let mut expected = 0.0;
    let mut schedule_yf = 0.0;
    for period in &periods {
        if period.payment_date <= start {
            continue;
        }
        let df = disc
            .as_ref()
            .df_between_dates(start, period.payment_date)
            .expect("df");
        schedule_yf += period.accrual_year_fraction * df;
        let (accrual_start, accrual_end) =
            finstack_quant_valuations_core::instruments::pricing::overnight::adjust_overnight_accrual_boundaries(
                period.accrual_start,
                period.accrual_end,
                BusinessDayConvention::ModifiedFollowing,
                calendar,
            )
            .expect("adjust");
        let projection = finstack_quant_valuations_core::instruments::pricing::overnight::project_overnight_coupon(
            finstack_quant_valuations_core::instruments::pricing::overnight::OvernightCouponProjectionInput {
                curve: finstack_quant_valuations_core::instruments::pricing::overnight::OvernightProjectionCurve::Forward(
                    fwd.as_ref(),
                ),
                fixings: None,
                fixing_id: "USD-SOFR-OIS",
                as_of: start,
                accrual_start,
                accrual_end,
                end_is_termination_date: None,
                day_count: DayCount::Act360,
                coupon_frequency: Some(Tenor::quarterly()),
                coupon_period: (accrual_start, accrual_end),
                compounding: &FloatingLegCompounding::CompoundedInArrears { lookback_days: 0 },
                fixing_calendar: calendar,
                compounded_spread: 0.0,
                need_observation_exposures: false,
            },
        )
        .expect("projection");
        expected += projection.accrual_year_fraction * df;
    }
    assert!(
        (annuity - expected).abs() < 1e-12,
        "overnight annuity {annuity} != projector yf annuity {expected}"
    );
    assert!(
        (annuity - schedule_yf).abs() > 1e-8,
        "holiday-adjusted overnight annuity must differ from raw schedule yf"
    );
}

#[test]
fn validate_rejects_simple_compounding_on_overnight_index() {
    let start = date(2025, 1, 2);
    let end = date(2026, 1, 2);
    let overnight = FloatLegSpec {
        end_of_month: false,
        fixing_calendar_id: None,
        index_id: "USD-SOFR-OIS".into(),
        forward_curve_id: CurveId::new("USD-SOFR-OIS"),
        discount_curve_id: CurveId::new("USD-OIS"),
        start,
        end,
        frequency: Tenor::quarterly(),
        day_count: DayCount::Act360,
        business_day_convention: BusinessDayConvention::ModifiedFollowing,
        calendar_id: Some("usny".into()),
        stub: StubKind::ShortFront,
        spread_bp: Decimal::ZERO,
        payment_lag_days: 0,
        reset_lag_days: 0,
        compounding: finstack_quant_cashflows::builder::FloatingLegCompounding::Simple,
    };
    let term = FloatLegSpec {
        index_id: "USD-SOFR-3M".into(),
        forward_curve_id: CurveId::new("USD-SOFR-3M"),
        ..overnight.clone()
    };
    let err = BasisSwap::new(
        "OIS-SIMPLE",
        Money::from((1_000_000_i64, Currency::USD)),
        overnight,
        term,
    )
    .expect_err("Simple on USD-SOFR-OIS must fail at construction");
    assert!(
        format!("{err}").contains("Overnight RFR"),
        "expected overnight/Simple rejection, got {err}"
    );
}
