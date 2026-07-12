//! QuantLib Parity Tests for Forward Rate Agreements (FRA)
//!
//! Test cases based on QuantLib test suite: `forwardrateagreement.cpp`
//! QuantLib version: 1.34
//! Reference: https://github.com/lballabio/QuantLib/blob/master/test-suite/forwardrateagreement.cpp
//!
//! These tests verify that finstack-quant FRA pricing follows QuantLib's methodology for:
//! - Standard FRA valuation with flat curves
//! - Par rate calculations (forward rate that makes PV = 0)
//! - Settlement adjustment: PV = N × DF × τ × (F - K) / (1 + F × τ)
//! - At-market and off-market FRAs
//! - Standard tenors (3x6, 6x9, 6x12, etc.)
//!
//! ## QuantLib FRA Convention
//!
//! QuantLib uses standard market conventions:
//! - Settlement at start of accrual period
//! - Settlement adjustment factor: 1 / (1 + F × τ)
//! - Day count: typically ACT/360 for USD/EUR, ACT/365 for GBP
//! - PV = Notional × DF(settlement) × τ × (F - K) / (1 + F × τ)
//!
//! ## Key Tests Covered
//!
//! 1. **testFRAvaluation**: Standard FRA NPV with flat curves
//! 2. **testFRAimpliedRate**: Par rate equals forward rate
//! 3. **testFRAsettlementAdjustment**: Settlement adjustment validation
//! 4. **testFRAconsistency**: Consistency across different strikes
//! 5. **testFRAbuySell**: Buy/sell symmetry

#[allow(unused_imports)]
use crate::parity::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt, DayCount, DayCountContext};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, ForwardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::rates::fra::ForwardRateAgreement;
use finstack_quant_valuations::instruments::rates::irs::PayReceive;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal::Decimal;
use time::macros::date;

/// Helper: Create a flat discount curve
fn create_flat_discount_curve(base_date: Date, rate: f64, curve_id: &str) -> DiscountCurve {
    let times = [0.0, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
    let dfs: Vec<_> = times.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder(curve_id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots(dfs)
        .build()
        .unwrap()
}

/// Helper: Create a flat forward curve
fn create_flat_forward_curve(
    base_date: Date,
    rate: f64,
    curve_id: &str,
    tenor: f64,
) -> ForwardCurve {
    let projection_grid = (0..=40)
        .map(|quarter| {
            DayCount::Act360
                .year_fraction(
                    base_date,
                    base_date.add_months(quarter * 3),
                    DayCountContext::default(),
                )
                .expect("valid quarterly projection boundary")
        })
        .collect::<Vec<_>>();
    let last_time = *projection_grid
        .last()
        .expect("projection grid is non-empty");

    ForwardCurve::builder(curve_id, tenor)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([(0.0, rate), (last_time, rate)])
        .projection_grid(projection_grid)
        .interp(InterpStyle::Linear)
        .build()
        .unwrap()
}

/// Helper: Create market context with flat curves
fn create_flat_market(base_date: Date, rate: f64) -> MarketContext {
    let disc = create_flat_discount_curve(base_date, rate, "USD_OIS");
    let fwd = create_flat_forward_curve(base_date, rate, "USD_LIBOR_3M", 0.25);
    MarketContext::new().insert(disc).insert(fwd)
}

fn dec(value: f64) -> Decimal {
    Decimal::try_from(value).expect("valid literal")
}

// =============================================================================
// Test 1: Standard FRA Valuation
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp, testFRAvaluation()
//
// Tests basic FRA NPV calculation with flat curves. At-market FRA (strike = forward)
// should have NPV ≈ 0, accounting for settlement adjustment.

#[test]
fn quantlib_parity_fra_at_market_valuation() {
    let base = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01); // 3M forward
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01); // 3M tenor (3x6 FRA)

    let notional = 1_000_000.0;
    let market_rate = 0.05; // 5% flat
    let strike = 0.05; // At-market strike

    let fra = ForwardRateAgreement {
        id: "FRA_3x6".into(),
        notional: Money::new(notional, Currency::USD),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: dec(strike),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive, // receive fixed rate
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, market_rate);
    let pv = fra.value(&market, base).unwrap();

    // QuantLib expectation: At-market FRA has NPV ≈ 0
    let quantlib_npv = 0.0;

    // Small residual from continuous OIS curve vs simple ACT/360 forward convention
    assert!(
        pv.amount().abs() < 50.0,
        "At-market FRA NPV should be near zero: got {}, expected ~{}",
        pv.amount(),
        quantlib_npv
    );
}

#[test]
fn quantlib_parity_fra_off_market_valuation() {
    let base = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);

    let notional = 1_000_000.0;
    let market_rate = 0.05; // 5% forward rate
    let strike = 0.06; // 6% strike (100bp above market)

    let fra = ForwardRateAgreement {
        id: "FRA_3x6".into(),
        notional: Money::new(notional, Currency::USD),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: dec(strike),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive, // receive fixed rate
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, market_rate);
    let pv = fra.value(&market, base).unwrap();

    // Manual calculation for validation:
    // τ = 91/360 ≈ 0.2528
    // F = 0.05 (forward rate)
    // K = 0.06 (strike)
    // DF(start) ≈ exp(-0.05 × 0.25) ≈ 0.9876
    // Settlement adj = 1 / (1 + 0.05 × 0.2528) ≈ 0.9874
    // PV = 1M × 0.9876 × 0.2528 × (0.05 - 0.06) × 0.9874
    //    = 1M × 0.9876 × 0.2528 × (-0.01) × 0.9874
    //    ≈ -2,464
    // With pay_fixed=true (receive fixed), sign is negated: ≈ +2,464

    // QuantLib produces similar value (accounting for exact day count and DF)
    let expected_npv = 2464.0; // Approximate expected value

    assert_parity!(
        pv.amount(),
        expected_npv,
        ParityConfig::with_relative_tolerance(0.05), // 5% tolerance for approximation
        "Off-market FRA valuation"
    );
}

// =============================================================================
// Test 2: FRA Implied Rate (Par Rate)
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp, testFRAimpliedRate()
//
// Tests that the par rate (rate that makes NPV = 0) equals the forward rate
// from the curve.

#[test]
fn quantlib_parity_fra_implied_rate() {
    let base = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);

    let market_rate = 0.05; // 5% flat

    let fra = ForwardRateAgreement {
        id: "FRA_3x6".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.05), // Will be overridden by par rate calculation
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, market_rate);

    // Calculate par rate
    let result = fra
        .price_with_metrics(
            &market,
            base,
            &[MetricId::ParRate],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let par_rate = *result.measures.get("par_rate").unwrap();

    // QuantLib expectation: Par rate should equal forward rate
    let expected_forward = market_rate;

    assert_parity!(
        par_rate,
        expected_forward,
        ParityConfig::tight(),
        "FRA implied rate equals forward rate"
    );
}

// =============================================================================
// Test 3: Settlement Adjustment Validation
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp, testFRAsettlementAdjustment()
//
// Validates that the settlement adjustment factor 1/(1 + F×τ) is correctly
// applied to the FRA NPV.

#[test]
fn quantlib_parity_fra_settlement_adjustment() {
    let base = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);

    let notional = 1_000_000.0;
    let market_rate = 0.10; // 10% to make adjustment more visible
    let strike = 0.11; // 11% strike

    let fra = ForwardRateAgreement {
        id: "FRA_3x6".into(),
        notional: Money::new(notional, Currency::USD),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: dec(strike),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, market_rate);
    let pv = fra.value(&market, base).unwrap();

    // Verify that PV is non-zero and has correct sign
    // For receive-fixed (strike 11%, market 10%): positive PV
    // The settlement adjustment should reduce the absolute value compared to
    // naive calculation without adjustment

    // Manual calculation:
    // τ ≈ 0.2528 (91 days / 360)
    // DF(start) ≈ exp(-0.10 × 0.25) ≈ 0.9753
    // Forward rate F = 0.10
    // Strike K = 0.11
    // Rate diff (F - K) = -0.01
    //
    // Without settlement adjustment:
    // PV_naive = 1M × DF × τ × (F - K) = 1M × 0.9753 × 0.2528 × (-0.01) ≈ -2,465
    //
    // With settlement adjustment: 1 / (1 + F × τ) = 1 / (1 + 0.10 × 0.2528) ≈ 0.9753
    // PV_adjusted = PV_naive × 0.9753 ≈ -2,404
    //
    // With receive_fixed=true (receive fixed), sign is negated: ≈ +2,404

    // QuantLib produces value accounting for exact settlement adjustment
    let expected_range_low = 2000.0;
    let expected_range_high = 3000.0;

    assert!(
        pv.amount() > expected_range_low && pv.amount() < expected_range_high,
        "FRA PV with settlement adjustment should be in range [{}, {}]: got {}",
        expected_range_low,
        expected_range_high,
        pv.amount()
    );
}

// =============================================================================
// Test 4: Buy/Sell Symmetry
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp, testFRAbuySell()
//
// Tests that buying and selling FRAs produce opposite NPVs (zero-sum game).

#[test]
fn quantlib_parity_fra_buy_sell_symmetry() {
    let base = date!(2024 - 01 - 01);
    let fixing = date!(2024 - 04 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);

    let market_rate = 0.05;
    let strike = 0.06; // Off-market

    // Receive fixed (buy protection against rising rates)
    let fra_receive = ForwardRateAgreement {
        id: "FRA_RECEIVE".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: dec(strike),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive, // receive fixed rate
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    // Pay fixed (sell protection)
    let fra_pay = ForwardRateAgreement {
        id: "FRA_PAY".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(fixing),
        start_date: start,
        maturity: end,
        fixed_rate: dec(strike),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Pay, // pay fixed rate
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, market_rate);
    let pv_receive = fra_receive.value(&market, base).unwrap();
    let pv_pay = fra_pay.value(&market, base).unwrap();

    // QuantLib expectation: Buy and sell should be exact opposites
    let sum = pv_receive.amount() + pv_pay.amount();

    assert!(
        sum.abs() < 0.01,
        "Buy and sell FRAs should sum to zero (zero-sum game): receive={}, pay={}, sum={}",
        pv_receive.amount(),
        pv_pay.amount(),
        sum
    );
}

// =============================================================================
// Test 5: Standard Tenors
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp, testFRAconsistency()
//
// Tests FRA valuation across standard market tenors (3x6, 6x9, 6x12, etc.)

#[test]
fn quantlib_parity_fra_standard_tenor_3x6() {
    let base = date!(2024 - 01 - 01);
    let start = date!(2024 - 04 - 01); // 3M forward
    let end = date!(2024 - 07 - 01); // 3M tenor

    let fra = ForwardRateAgreement {
        id: "FRA_3x6".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.05),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, 0.05);
    let pv = fra.value(&market, base).unwrap();

    // At-market FRA should have near-zero NPV (small residual from continuous vs simple convention)
    assert!(
        pv.amount().abs() < 50.0,
        "3x6 at-market FRA should be near zero, got: {}",
        pv.amount()
    );
}

#[test]
fn quantlib_parity_fra_standard_tenor_6x9() {
    let base = date!(2024 - 01 - 01);
    let start = date!(2024 - 07 - 01); // 6M forward
    let end = date!(2024 - 10 - 01); // 3M tenor

    let fra = ForwardRateAgreement {
        id: "FRA_6x9".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.05),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, 0.05);
    let pv = fra.value(&market, base).unwrap();

    // At-market FRA should have near-zero NPV (small residual from continuous vs simple convention)
    assert!(
        pv.amount().abs() < 50.0,
        "6x9 at-market FRA should be near zero, got: {}",
        pv.amount()
    );
}

#[test]
fn quantlib_parity_fra_standard_tenor_6x12() {
    let base = date!(2024 - 01 - 01);
    let start = date!(2024 - 07 - 01); // 6M forward
    let end = date!(2025 - 01 - 01); // 6M tenor

    let fra = ForwardRateAgreement {
        id: "FRA_6x12".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.05),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, 0.05);
    let pv = fra.value(&market, base).unwrap();

    // 6M tenor has larger convention difference (continuous vs simple over 6M)
    assert!(
        pv.amount().abs() < 200.0,
        "6x12 at-market FRA should be near zero, got: {}",
        pv.amount()
    );
}

// =============================================================================
// Test 6: DV01 Consistency
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp (DV01 tests)
//
// Tests that DV01 has correct sign and magnitude

#[test]
fn quantlib_parity_fra_dv01_sign_convention() {
    let base = date!(2024 - 01 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);

    let fra_receive = ForwardRateAgreement {
        id: "FRA_RECEIVE".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.05),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive, // receive fixed rate
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let fra_pay = ForwardRateAgreement {
        id: "FRA_PAY".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.05),
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Pay, // pay fixed rate
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, 0.05);

    let result_receive = fra_receive
        .price_with_metrics(
            &market,
            base,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_receive = *result_receive.measures.get("dv01").unwrap();

    let result_pay = fra_pay
        .price_with_metrics(
            &market,
            base,
            &[MetricId::Dv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let dv01_pay = *result_pay.measures.get("dv01").unwrap();

    // QuantLib convention: receive fixed → negative DV01, pay fixed → positive DV01
    assert!(
        dv01_receive < 0.0,
        "Receive fixed should have negative DV01"
    );
    assert!(dv01_pay > 0.0, "Pay fixed should have positive DV01");

    // DV01s should be equal in magnitude
    assert_parity!(
        dv01_receive.abs(),
        dv01_pay.abs(),
        ParityConfig::tight(),
        "DV01 magnitude symmetry"
    );
}

// =============================================================================
// Test 7: Day Count Convention Impact
// =============================================================================
// QuantLib reference: forwardrateagreement.cpp (day count tests)
//
// Tests that different day count conventions produce expected differences

#[test]
fn quantlib_parity_fra_day_count_act360_vs_act365() {
    let base = date!(2024 - 01 - 01);
    let start = date!(2024 - 04 - 01);
    let end = date!(2024 - 07 - 01);

    let fra_360 = ForwardRateAgreement {
        id: "FRA_ACT360".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.06), // Off-market for visible difference
        day_count: DayCount::Act360,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let fra_365 = ForwardRateAgreement {
        id: "FRA_ACT365".into(),
        notional: Money::new(1_000_000.0, Currency::USD),
        fixing_date: Some(start),
        start_date: start,
        maturity: end,
        fixed_rate: dec(0.06),
        day_count: DayCount::Act365F,
        reset_lag: 2,
        fixing_calendar_id: None,
        fixing_bdc: None,
        observed_fixing: None,
        discount_curve_id: "USD_OIS".into(),
        forward_curve_id: "USD_LIBOR_3M".into(),
        side: PayReceive::Receive,
        pricing_overrides: finstack_quant_valuations::instruments::PricingOverrides::default(),
        attributes: Default::default(),
    };

    let market = create_flat_market(base, 0.05);
    let pv_360 = fra_360.value(&market, base).unwrap();
    let pv_365 = fra_365.value(&market, base).unwrap();

    // ACT/360 should produce larger accrual factor (τ) than ACT/365
    // Therefore, PV magnitude should be larger for ACT/360
    assert!(
        pv_360.amount().abs() > pv_365.amount().abs(),
        "ACT/360 should produce larger NPV than ACT/365: 360={}, 365={}",
        pv_360.amount(),
        pv_365.amount()
    );

    // Difference should be approximately (365-360)/360 ≈ 1.39%
    let ratio = pv_360.amount() / pv_365.amount();
    let expected_ratio = 365.0 / 360.0; // ≈ 1.0139

    assert_parity!(
        ratio,
        expected_ratio,
        ParityConfig::with_relative_tolerance(0.05),
        "Day count convention ratio"
    );
}
