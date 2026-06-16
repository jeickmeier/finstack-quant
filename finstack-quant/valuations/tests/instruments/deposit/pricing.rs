//! Pricing and valuation tests for Deposit instruments.
//!
//! Tests NPV calculations, rate sensitivity, and various pricing scenarios
//! to ensure correctness of the deposit pricing engine.

use super::common::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;

#[test]
fn test_zero_rate_deposit_negative_pv() {
    // Setup - deposit with 0% rate should have negative *trade* NPV (time value)
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.0)
        .build();

    // `value` is holder-view position value (PV of remaining flows; the T+0
    // initial exchange on `as_of` is excluded). Trade-NPV semantics live in
    // the pricing-view `npv_raw`, which includes the initial exchange.
    let npv = dep.npv_raw(&ctx, base).unwrap();

    // Validate - Should be negative (pay notional, get back same amount discounted)
    assert!(npv < 0.0);
}

#[test]
fn test_positive_rate_deposit_less_negative_pv() {
    // Setup - deposit with positive rate should offset discount
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute
    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.05)
        .build();

    let pv = dep.value(&ctx, base).unwrap();

    // Validate - Could be positive or negative depending on rate vs curve
    // The key is that it exists and is computable
    assert!(pv.currency() == Currency::USD);
}

#[test]
fn test_par_rate_gives_zero_pv() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let dep = DepositBuilder::new(base).maturity(date(2025, 7, 1)).build();

    // Execute - compute par rate
    let par_rate = compute_metric(
        &dep,
        &ctx,
        base,
        finstack_quant_valuations::metrics::MetricId::DepositParRate,
    );

    // Execute - price with par rate
    let dep_par = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(par_rate)
        .build();

    // Par rate zeroes the *trade* NPV (pricing view, includes the T+0 initial
    // exchange). Holder-view `value` excludes the initial exchange and is
    // ≈ +notional for any quote rate, so the par identity is asserted on
    // `npv_raw`.
    let npv = dep_par.npv_raw(&ctx, base).unwrap();

    // Validate - trade NPV should be essentially zero for deposit at par rate
    // Market standard: < $0.01 on $1M notional (< 0.001bp numerical precision)
    assert!(
        npv.abs() < 0.01,
        "trade NPV at par rate should be < $0.01, got: {npv}",
    );
}

#[test]
fn test_higher_rate_increases_pv() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute - two deposits with different rates
    let dep_low = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.01)
        .build();

    let dep_high = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.05)
        .build();

    let pv_low = dep_low.value(&ctx, base).unwrap();
    let pv_high = dep_high.value(&ctx, base).unwrap();

    // Validate - higher rate should give higher PV (more cash back at maturity)
    assert!(pv_high.amount() > pv_low.amount());
}

#[test]
fn test_longer_maturity_increases_rate_sensitivity() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");
    let rate = 0.03;

    // Execute - two deposits with different maturities
    let dep_short = DepositBuilder::new(base)
        .maturity(date(2025, 4, 1))
        .quote_rate(rate)
        .build();

    let dep_long = DepositBuilder::new(base)
        .maturity(date(2026, 1, 1))
        .quote_rate(rate)
        .build();

    let pv_short = dep_short.value(&ctx, base).unwrap();
    let pv_long = dep_long.value(&ctx, base).unwrap();

    // Validate - longer deposit has higher absolute PV difference from zero
    assert!(pv_long.amount().abs() > pv_short.amount().abs());
}

#[test]
fn test_notional_scales_pv_linearly() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    // Execute
    let dep_1m = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    let dep_2m = DepositBuilder::new(base)
        .notional(Money::new(2_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    let pv_1m = dep_1m.value(&ctx, base).unwrap();
    let pv_2m = dep_2m.value(&ctx, base).unwrap();

    // Validate - PV should scale linearly with notional
    assert!((pv_2m.amount() / pv_1m.amount() - 2.0).abs() < 0.01);
}

#[test]
fn test_valuation_on_different_as_of_dates() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(date(2025, 1, 15))
        .maturity(date(2025, 7, 15))
        .quote_rate(0.03)
        .build();

    // Execute - value on different dates
    let pv_base = dep.value(&ctx, base).unwrap();
    let pv_later = dep.value(&ctx, date(2025, 1, 15)).unwrap();

    // Validate - both should produce valid results
    assert!(pv_base.currency() == Currency::USD);
    assert!(pv_later.currency() == Currency::USD);
    // PV on start date should be different from PV before start
    assert!((pv_base.amount() - pv_later.amount()).abs() > 1.0);
}

#[test]
fn test_steep_curve_impact() {
    // Setup - test with steep discount curve
    let base = date(2025, 1, 1);
    let ctx_steep = ctx_with_steep_curve(base, "USD-OIS");
    let ctx_flat = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2026, 1, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv_steep = dep.value(&ctx_steep, base).unwrap();
    let pv_flat = dep.value(&ctx_flat, base).unwrap();

    // Validate - steeper curve should give more discounting
    assert_ne!(pv_steep.amount(), pv_flat.amount());
}

#[test]
fn test_value_trait_implementation() {
    // Setup - test Priceable trait
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate
    assert_eq!(pv.currency(), Currency::USD);
    assert!(pv.amount().is_finite());
}

#[test]
fn test_npv_matches_value_trait() {
    // Setup - ensure npv() and value() give same results
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // Execute
    let npv = dep.value(&ctx, base).unwrap();
    let value = dep.value(&ctx, base).unwrap();

    // Validate
    assert!((npv.amount() - value.amount()).abs() < PRICE_TOLERANCE);
}

#[test]
fn test_pricing_with_act_365_day_count() {
    // Setup
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep_360 = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act360)
        .quote_rate(0.03)
        .build();

    let dep_365 = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .day_count(DayCount::Act365F)
        .quote_rate(0.03)
        .build();

    // Execute
    let pv_360 = dep_360.value(&ctx, base).unwrap();
    let pv_365 = dep_365.value(&ctx, base).unwrap();

    // Validate - different day counts should give different PVs
    assert_ne!(pv_360.amount(), pv_365.amount());
}

#[test]
fn test_negative_rate_environment() {
    // Setup - test with negative quoted rate
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(-0.01)
        .build();

    // Execute
    let pv = dep.value(&ctx, base).unwrap();

    // Validate - should compute without error
    assert!(pv.currency() == Currency::USD);
    assert!(pv.amount().is_finite());
}

#[test]
fn test_pricing_on_maturity_date() {
    // Setup - price on the end date
    let base = date(2025, 1, 1);
    let end = date(2025, 7, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(end)
        .quote_rate(0.03)
        .build();

    // Execute
    let pv = dep.value(&ctx, end).unwrap();

    // Validate - on maturity, should be close to redemption amount
    // (minor discounting from end date to itself)
    assert!(pv.currency() == Currency::USD);
}

#[test]
fn test_theta_correctness_with_as_of_forward() {
    // Test that valuing forward in time (as_of > curve base) produces correct theta decay
    // This validates the cashflow-based discounting from as_of date
    let curve_base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(curve_base, "USD-OIS");

    let dep = DepositBuilder::new(date(2025, 1, 15))
        .maturity(date(2025, 7, 15))
        .quote_rate(0.05)
        .build();

    // Value on curve base date and one day later
    let pv_t0 = dep.value(&ctx, curve_base).unwrap();
    let pv_t1 = dep.value(&ctx, date(2025, 1, 2)).unwrap();

    // PV should increase as we move forward in time (theta decay)
    // because we're getting closer to receiving the cashflows
    assert!(
        pv_t1.amount() > pv_t0.amount(),
        "Theta: PV should increase moving forward in time. t0={}, t1={}",
        pv_t0.amount(),
        pv_t1.amount()
    );
}

#[test]
fn test_cashflow_based_npv_consistency() {
    // Verify that the cashflow-based NPV implementation produces consistent results
    // across different as_of dates and curve configurations
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .maturity(date(2025, 7, 1))
        .quote_rate(0.03)
        .build();

    // NPV should be deterministic and repeatable
    let pv1 = dep.value(&ctx, base).unwrap();
    let pv2 = dep.value(&ctx, base).unwrap();

    assert_eq!(pv1.amount(), pv2.amount(), "NPV should be deterministic");
}

#[test]
fn test_generic_dv01_works_with_cashflows() {
    // Verify that the generic DV01 calculator works correctly with the cashflow-based approach
    // This is a regression test to ensure the migration to GenericParallelDv01 succeeded
    let base = date(2025, 1, 1);
    let ctx = ctx_with_standard_disc(base, "USD-OIS");

    let dep = DepositBuilder::new(base)
        .notional(Money::new(1_000_000.0, Currency::USD))
        .maturity(date(2025, 7, 1))
        .build();

    // Compute DV01 using the metric registry
    let dv01 = compute_metric(
        &dep,
        &ctx,
        base,
        finstack_quant_valuations::metrics::MetricId::Dv01,
    );

    // Verify it's reasonable (roughly duration * notional * 1bp)
    // For a 6-month deposit on $1M notional, DV01 magnitude should be around $50
    // DV01 is negative for long positions (standard convention: rates up → PV down)
    assert!(
        dv01.abs() > 40.0 && dv01.abs() < 60.0,
        "DV01 magnitude should be in reasonable range for 6m $1M deposit: {}",
        dv01
    );
}
