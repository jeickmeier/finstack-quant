//! Tests for CDS upfront payment support.
//!
//! Verifies that upfront payments are correctly included in NPV calculations,
//! respecting pay/receive direction and discounting.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

/// Build flat discount curve for testing
fn build_discount_curve(rate: f64, base_date: Date, id: &str) -> DiscountCurve {
    DiscountCurve::builder(id)
        .base_date(base_date)
        .day_count(DayCount::Act360)
        .knots([
            (0.0, 1.0),
            (1.0, (-rate).exp()),
            (5.0, (-rate * 5.0).exp()),
            (10.0, (-rate * 10.0).exp()),
        ])
        .build()
        .unwrap()
}

/// Build flat hazard curve for testing
fn build_hazard_curve(hazard_rate: f64, recovery: f64, base_date: Date, id: &str) -> HazardCurve {
    HazardCurve::builder(id)
        .base_date(base_date)
        .recovery_rate(recovery)
        .knots([
            (0.0, hazard_rate),
            (1.0, hazard_rate),
            (5.0, hazard_rate),
            (10.0, hazard_rate),
        ])
        .build()
        .unwrap()
}

#[test]
fn test_upfront_payment_buyer_payfast() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    // Creates a CDS (Buy Protection)
    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "UPFRONT_BUYER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    // Calculate base NPV (without upfront)
    let base_npv = cds.value_raw(&market, as_of).unwrap();

    // Add Upfront Payment: 500k USD paid by Buyer (me)
    let upfront_amount = 500_000.0;
    cds.upfront = Some((as_of, Money::new(upfront_amount, Currency::USD)));

    let npv_with_upfront = cds.value_raw(&market, as_of).unwrap();

    // As Buyer, I pay the upfront. So my NPV should decrease by exactly the upfront amount (since payment date is as_of).
    // NPV = Protection_PV - Premium_PV - Upfront
    assert!(
        (npv_with_upfront - (base_npv - upfront_amount)).abs() < 1e-6,
        "Buyer NPV should decrease by upfront amount. Base: {}, With Upfront: {}, Expected Diff: {}",
        base_npv,
        npv_with_upfront,
        -upfront_amount
    );
}

#[test]
fn test_upfront_payment_seller_receivefast() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    // Creates a CDS (Sell Protection)
    let mut cds = crate::finstack_quant_test_utils::cds_sell_protection(
        "UPFRONT_SELLER",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    // Calculate base NPV (without upfront)
    let base_npv = cds.value_raw(&market, as_of).unwrap();

    // Add Upfront Payment: 500k USD paid by Buyer (to ME, the Seller)
    let upfront_amount = 500_000.0;
    cds.upfront = Some((as_of, Money::new(upfront_amount, Currency::USD)));

    let npv_with_upfront = cds.value_raw(&market, as_of).unwrap();

    // As Seller, I receive the upfront. So my NPV should increase by exactly the upfront amount.
    // NPV_Seller = Premium_PV - Protection_PV + Upfront
    assert!(
        (npv_with_upfront - (base_npv + upfront_amount)).abs() < 1e-6,
        "Seller NPV should increase by upfront amount. Base: {}, With Upfront: {}, Expected Diff: {}",
        base_npv,
        npv_with_upfront,
        upfront_amount
    );
}

#[test]
fn test_upfront_payment_discounted() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    // Upfront payment 1 year in the future
    let payment_date = date!(2025 - 01 - 01);

    let rate = 0.05;
    let disc = build_discount_curve(rate, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "UPFRONT_DISCOUNTED",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let base_npv = cds.value_raw(&market, as_of).unwrap();

    let upfront_amount = 100_000.0;
    cds.upfront = Some((payment_date, Money::new(upfront_amount, Currency::USD)));

    let npv_with_upfront = cds.value_raw(&market, as_of).unwrap();

    // Expected discount factor roughly exp(-0.05 * 1.0)
    // Actually using ACT/360 in helper, so check exact year fraction or trust the curve calculation.
    // We can fetch DF from curve for precision.
    let curve = market.get_discount("USD_OIS").unwrap();
    let df = curve.df_between_dates(as_of, payment_date).unwrap();
    let expected_pv_impact = upfront_amount * df;

    assert!(
        (npv_with_upfront - (base_npv - expected_pv_impact)).abs() < 1e-6,
        "NPV should reflect discounted upfront. Diff: {}, Expected PV Impact: {}",
        base_npv - npv_with_upfront,
        expected_pv_impact
    );
}

#[test]
fn test_upfront_payment_past_is_ignored() {
    let as_of = date!(2024 - 01 - 01);
    let end = date!(2029 - 01 - 01);
    // Upfront payment in the past
    let payment_date = date!(2023 - 12 - 31);

    let disc = build_discount_curve(0.05, as_of, "USD_OIS");
    let hazard = build_hazard_curve(0.02, 0.40, as_of, "CORP");
    let market = MarketContext::new().insert(disc).insert(hazard);

    let mut cds = crate::finstack_quant_test_utils::cds_buy_protection(
        "UPFRONT_PAST",
        Money::new(10_000_000.0, Currency::USD),
        100.0,
        as_of,
        end,
        "USD_OIS",
        "CORP",
    )
    .expect("CDS construction should succeed");

    let base_npv = cds.value_raw(&market, as_of).unwrap();

    let upfront_amount = 1_000_000.0;
    cds.upfront = Some((payment_date, Money::new(upfront_amount, Currency::USD)));

    let npv_with_upfront = cds.value_raw(&market, as_of).unwrap();

    // Past cashflow should be ignored (PV=0 contribution)
    assert!(
        (npv_with_upfront - base_npv).abs() < 1e-6,
        "Past upfront payment should affect NPV by 0.0"
    );
}
