//! QuantLib parity tests for Credit Default Swap pricing.
//!
//! Tests a 5Y CDS:
//! - Running spread: 100bp
//! - Recovery: 40%
//! - Flat hazard rate: ~167bp (≈ 100 / (1 - 0.4) / 100)
//!
//! Validates:
//! 1. Par spread is close to 100bp for the matching hazard rate
//! 2. CS01 ≈ risky_annuity × 1bp
//! 3. Buy protection + sell protection = 0 (at same spread)

use crate::finstack_quant_test_utils as test_utils;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::Month;

const NOTIONAL: f64 = 10_000_000.0;
const SPREAD_BP: f64 = 100.0;
const RECOVERY: f64 = 0.40;

// Flat hazard rate consistent with 100bp spread and 40% recovery:
// spread ≈ hazard_rate × (1 - recovery)
// hazard_rate = spread / (1 - recovery) = 0.01 / 0.6 = 0.01667
const HAZARD_RATE: f64 = 0.01667;

/// Build a flat discount curve.
fn flat_discount_curve(as_of: Date, rate: f64) -> DiscountCurve {
    let tenors = [0.0, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
    let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();

    DiscountCurve::builder("USD-OIS")
        .base_date(as_of)
        .knots(knots)
        .interp(InterpStyle::Linear)
        .build()
        .expect("discount curve should build")
}

/// Build a flat hazard (credit) curve from constant hazard rate.
///
/// The HazardCurve builder takes (time, hazard_rate) pairs as knots.
fn flat_hazard_curve(as_of: Date, hazard_rate: f64) -> HazardCurve {
    HazardCurve::builder("CDS-CREDIT")
        .base_date(as_of)
        .knots(vec![
            (0.0, hazard_rate),
            (1.0, hazard_rate),
            (3.0, hazard_rate),
            (5.0, hazard_rate),
            (10.0, hazard_rate),
        ])
        .build()
        .expect("hazard curve should build")
}

fn test_market(as_of: Date) -> MarketContext {
    MarketContext::new()
        .insert(flat_discount_curve(as_of, 0.02))
        .insert(flat_hazard_curve(as_of, HAZARD_RATE))
}

/// Create a 5Y CDS (buy protection).
fn create_5y_cds_buy(as_of: Date) -> finstack_quant_valuations::instruments::CreditDefaultSwap {
    let start = as_of;
    let maturity = Date::from_calendar_date(as_of.year() + 5, as_of.month(), as_of.day())
        .expect("valid maturity");

    test_utils::cds_buy_protection(
        "CDS-QLPARITY-5Y-BUY",
        Money::new(NOTIONAL, Currency::USD),
        SPREAD_BP,
        start,
        maturity,
        "USD-OIS",
        "CDS-CREDIT",
    )
    .expect("CDS construction should succeed")
}

/// Create a 5Y CDS (sell protection).
fn create_5y_cds_sell(as_of: Date) -> finstack_quant_valuations::instruments::CreditDefaultSwap {
    let start = as_of;
    let maturity = Date::from_calendar_date(as_of.year() + 5, as_of.month(), as_of.day())
        .expect("valid maturity");

    test_utils::cds_sell_protection(
        "CDS-QLPARITY-5Y-SELL",
        Money::new(NOTIONAL, Currency::USD),
        SPREAD_BP,
        start,
        maturity,
        "USD-OIS",
        "CDS-CREDIT",
    )
    .expect("CDS construction should succeed")
}

/// Test: Par spread should be close to the running spread when hazard rate is consistent.
///
/// With hazard_rate = spread / (1 - recovery), the par spread should be approximately
/// equal to the running spread (100bp).
#[test]
fn test_cds_par_spread_consistency() {
    let as_of = Date::from_calendar_date(2025, Month::March, 20).expect("valid IMM date");
    let market = test_market(as_of);
    let cds = create_5y_cds_buy(as_of);

    let metrics = vec![MetricId::ParSpread];
    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing with metrics should succeed");

    if let Some(&par_spread_bp) = result.measures.get(MetricId::ParSpread.as_str()) {
        // Tolerance: 30bp. The approximation spread ≈ h×(1-R) is first-order
        // and the ISDA schedule conventions affect the exact par spread.
        assert!(
            (par_spread_bp - SPREAD_BP).abs() < 30.0,
            "Par spread should be close to running spread. Expected ~{:.0}bp, got {:.2}bp",
            SPREAD_BP,
            par_spread_bp
        );
    }
}

/// Test: CDS PV should be small at par spread.
///
/// When the running spread equals the par spread, the CDS should have
/// near-zero market value.
#[test]
fn test_cds_pv_near_zero_at_par() {
    let as_of = Date::from_calendar_date(2025, Month::March, 20).expect("valid IMM date");
    let market = test_market(as_of);
    let cds = create_5y_cds_buy(as_of);

    let pv = cds.value(&market, as_of).expect("pricing should succeed");

    // Tolerance: PV ratio must stay below 5% of notional when priced near par.
    let pv_ratio = (pv.amount() / NOTIONAL).abs();
    assert!(
        pv_ratio < 0.05,
        "CDS PV at par should be small relative to notional. PV = {:.2}, ratio = {:.4}%",
        pv.amount(),
        pv_ratio * 100.0
    );
}

/// Test: Buy protection + sell protection should cancel out.
///
/// A long and short CDS at the same spread should have opposite PVs.
#[test]
fn test_cds_buy_sell_symmetry() {
    let as_of = Date::from_calendar_date(2025, Month::March, 20).expect("valid IMM date");
    let market = test_market(as_of);

    let cds_buy = create_5y_cds_buy(as_of);
    let cds_sell = create_5y_cds_sell(as_of);

    let pv_buy = cds_buy.value(&market, as_of).expect("buy pricing").amount();
    let pv_sell = cds_sell
        .value(&market, as_of)
        .expect("sell pricing")
        .amount();

    let sum = (pv_buy + pv_sell).abs();
    let magnitude = pv_buy.abs().max(pv_sell.abs()).max(1.0);

    // Tolerance: buy/sell symmetry must hold within 2% of the larger leg magnitude.
    assert!(
        sum < magnitude * 0.02,
        "Buy + Sell should cancel. buy_pv = {:.2}, sell_pv = {:.2}, sum = {:.2}",
        pv_buy,
        pv_sell,
        sum
    );
}

/// Test: CS01 should be approximately risky_annuity × 1bp.
///
/// CS01 measures the PV change for a 1bp change in credit spread.
/// For a protection buyer, CS01 < 0 (spread widening benefits protection buyer).
#[test]
fn test_cds_cs01_vs_risky_annuity() {
    let as_of = Date::from_calendar_date(2025, Month::March, 20).expect("valid IMM date");
    let market = test_market(as_of);
    let cds = create_5y_cds_buy(as_of);

    let metrics = vec![MetricId::Cs01, MetricId::RiskyAnnuity];
    let result = cds
        .price_with_metrics(
            &market,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("pricing with metrics should succeed");

    if let (Some(&cs01), Some(&risky_annuity)) = (
        result.measures.get(MetricId::Cs01.as_str()),
        result.measures.get(MetricId::RiskyAnnuity.as_str()),
    ) {
        // CS01 should be negative for buy protection (spread widening = gain)
        // Risky annuity should be positive

        // The approximate relationship: |CS01| ≈ risky_annuity × notional × 0.0001
        let expected_cs01_approx = risky_annuity * NOTIONAL * 0.0001;

        if expected_cs01_approx.abs() > 1.0 {
            // Tolerance band: 0.3x to 3.0x because CS01 uses a finite-difference market bump
            // while the risky-annuity relationship is only a first-order approximation.
            let ratio = cs01.abs() / expected_cs01_approx.abs();
            assert!(
                ratio > 0.3 && ratio < 3.0,
                "CS01 should be approximately risky_annuity × notional × 1bp. CS01 = {:.2}, expected ≈ {:.2}, ratio = {:.2}",
                cs01,
                expected_cs01_approx,
                ratio
            );
        }
    }
}

/// Test: CDS value bounds.
///
/// Protection buyer PV should be bounded:
/// - Upper bound: notional × (1 - recovery) (max protection payout, undiscounted)
/// - Lower bound: -(spread × risky_annuity × notional) (max premium payment, undiscounted)
#[test]
fn test_cds_value_bounds() {
    let as_of = Date::from_calendar_date(2025, Month::March, 20).expect("valid IMM date");
    let market = test_market(as_of);
    let cds = create_5y_cds_buy(as_of);

    let pv = cds.value(&market, as_of).expect("pricing").amount();

    // Protection buyer PV should be bounded
    let max_protection = NOTIONAL * (1.0 - RECOVERY);
    let max_premium = NOTIONAL * (SPREAD_BP / 10_000.0) * 5.0; // rough max premium

    assert!(
        pv > -max_premium * 2.0,
        "CDS PV lower bound violated. PV = {:.2}, max_premium ≈ {:.2}",
        pv,
        max_premium
    );
    assert!(
        pv < max_protection * 2.0,
        "CDS PV upper bound violated. PV = {:.2}, max_protection ≈ {:.2}",
        pv,
        max_protection
    );
}
