//! CDS Index single-curve pricing mode tests.
//!
//! Tests cover:
//! - NPV calculations in single-curve mode
//! - Leg PV calculations (premium and protection)
//! - Par spread computation
//! - Risky PV01 calculation
//! - CS01 approximation
//! - Sign conventions for buy/sell protection
//! - Notional scaling behavior

use super::test_utils::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, IndexPricing,
};
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

fn metric_value(
    index: &CDSIndex,
    market: &finstack_quant_core::market_data::context::MarketContext,
    as_of: finstack_quant_core::dates::Date,
    metric: MetricId,
) -> f64 {
    let result = index
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .expect("metric should compute");
    result.measures[&metric]
}

#[test]
fn test_single_curve_npv_calculation() {
    // Test: NPV calculation in single-curve mode
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-NPV", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let npv = idx.value(&ctx, as_of).unwrap();

    assert_eq!(npv.currency(), Currency::USD);
    assert!(npv.amount().is_finite());
}

#[test]
fn test_single_curve_npv_components() {
    // Test: NPV = Protection PV - Premium PV (for protection buyer)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-COMPONENTS", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let npv = idx.value(&ctx, as_of).unwrap();
    let pv_prot = metric_value(&idx, &ctx, as_of, MetricId::ProtectionLegPv);
    let pv_prem = metric_value(&idx, &ctx, as_of, MetricId::PremiumLegPv);

    let expected_npv = pv_prot - pv_prem;
    assert!(
        (npv.amount() - expected_npv).abs() < 1.0,
        "NPV = Protection - Premium"
    );
}

#[test]
fn test_single_curve_protection_leg_positive() {
    // Test: Protection leg PV should be positive for protection buyer
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-PROT", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let pv_prot = metric_value(&idx, &ctx, as_of, MetricId::ProtectionLegPv);

    assert!(
        pv_prot > 0.0,
        "Protection leg PV should be positive, got {}",
        pv_prot
    );
}

#[test]
fn test_single_curve_premium_leg_positive() {
    // Test: Premium leg PV should be positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-PREM", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let pv_prem = metric_value(&idx, &ctx, as_of, MetricId::PremiumLegPv);

    assert!(
        pv_prem > 0.0,
        "Premium leg PV should be positive, got {}",
        pv_prem
    );
}

#[test]
fn test_single_curve_par_spread() {
    // Test: Par spread calculation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-PAR", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let par_spread = metric_value(&idx, &ctx, as_of, MetricId::ParSpread);

    assert_positive(par_spread, "Par spread");
    // Flat hazard approximation: spread ≈ hazard × (1 - recovery) × 10,000
    let expected = flat_hazard_par_spread_bps(STANDARD_HAZARD_RATE, STANDARD_RECOVERY_SENIOR);
    assert_in_range(
        par_spread,
        expected * 0.85,
        expected * 1.15,
        "Par spread near flat-hazard analytic",
    );
}

#[test]
fn test_single_curve_risky_pv01() {
    // Test: Risky PV01 calculation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-RPV01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let rpv01 = metric_value(&idx, &ctx, as_of, MetricId::RiskyPv01);

    assert_positive(rpv01, "Risky PV01");
    // For $10MM, 5Y: expect $4,000-$5,000
    assert_in_range(rpv01, 3_500.0, 5_500.0, "Risky PV01 magnitude");
}

#[test]
fn test_single_curve_cs01() {
    // Test: CS01 (credit sensitivity) calculation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-CS01", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let cs01 = idx.cs01(&ctx, as_of).unwrap();

    assert_positive(cs01, "CS01");
    // CS01 should be positive and reasonable
    assert!(cs01 > 100.0, "CS01 should be meaningful for $10MM notional");
}

#[test]
fn test_single_curve_buy_vs_sell_protection() {
    // Test: Sign conventions for buy vs sell protection
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    // Buy protection (pay fixed)
    let idx_buy = standard_single_curve_index("CDX-BUY", start, end, 10_000_000.0);

    // Sell protection (receive fixed)
    let idx_sell = CDSIndex::from_preset(
        &standard_cdx_params(),
        "CDX-SELL",
        Money::new(10_000_000.0, Currency::USD),
        PayReceive::Receive,
        start,
        end,
        STANDARD_RECOVERY_SENIOR,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters");

    let ctx = standard_market_context(as_of);

    let npv_buy = idx_buy.value(&ctx, as_of).unwrap();
    let npv_sell = idx_sell.value(&ctx, as_of).unwrap();

    // Buy and sell protection should offset to ~0
    let sum = npv_buy.amount() + npv_sell.amount();
    assert!(
        sum.abs() < 1.0,
        "Buy + sell NPV should net to ~0: buy={}, sell={}, sum={}",
        npv_buy.amount(),
        npv_sell.amount(),
        sum
    );
}

#[test]
fn test_single_curve_npv_scales_with_notional() {
    // Test: NPV scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-10MM", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-20MM", start, end, 20_000_000.0);

    let npv_10mm = idx_10mm.value(&ctx, as_of).unwrap().amount();
    let npv_20mm = idx_20mm.value(&ctx, as_of).unwrap().amount();

    assert_linear_scaling(npv_10mm, 10_000_000.0, npv_20mm, 20_000_000.0, "NPV", 0.01);
}

#[test]
fn test_single_curve_risky_pv01_scales_with_notional() {
    // Test: Risky PV01 scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-10MM", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-20MM", start, end, 20_000_000.0);

    let rpv01_10mm = metric_value(&idx_10mm, &ctx, as_of, MetricId::RiskyPv01);
    let rpv01_20mm = metric_value(&idx_20mm, &ctx, as_of, MetricId::RiskyPv01);

    assert_linear_scaling(
        rpv01_10mm,
        10_000_000.0,
        rpv01_20mm,
        20_000_000.0,
        "Risky PV01",
        0.01,
    );
}

#[test]
fn test_single_curve_cs01_scales_with_notional() {
    // Test: CS01 scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-10MM", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-20MM", start, end, 20_000_000.0);

    let cs01_10mm = idx_10mm.cs01(&ctx, as_of).unwrap();
    let cs01_20mm = idx_20mm.cs01(&ctx, as_of).unwrap();

    assert_linear_scaling(
        cs01_10mm,
        10_000_000.0,
        cs01_20mm,
        20_000_000.0,
        "CS01",
        0.05,
    );
}

#[test]
fn test_single_curve_par_spread_independent_of_notional() {
    // Test: Par spread should be independent of notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_1mm = standard_single_curve_index("CDX-1MM", start, end, 1_000_000.0);
    let idx_100mm = standard_single_curve_index("CDX-100MM", start, end, 100_000_000.0);

    let par_1mm = metric_value(&idx_1mm, &ctx, as_of, MetricId::ParSpread);
    let par_100mm = metric_value(&idx_100mm, &ctx, as_of, MetricId::ParSpread);

    assert_relative_eq(
        par_1mm,
        par_100mm,
        0.001,
        "Par spread should be notional-independent",
    );
}

#[test]
fn test_single_curve_pricing_mode_verification() {
    // Test: Single-curve index has correct pricing mode
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-MODE", start, end, 10_000_000.0);

    assert_eq!(idx.pricing, IndexPricing::SingleCurve);
    assert!(idx.constituents.is_empty());
}

#[test]
fn test_single_curve_maturity_impact() {
    // Test: Longer maturity increases protection value
    let start = date!(2025 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_3y = standard_single_curve_index("CDX-3Y", start, date!(2028 - 01 - 01), 10_000_000.0);
    let idx_5y = standard_single_curve_index("CDX-5Y", start, date!(2030 - 01 - 01), 10_000_000.0);
    let idx_10y =
        standard_single_curve_index("CDX-10Y", start, date!(2035 - 01 - 01), 10_000_000.0);

    let prot_3y = metric_value(&idx_3y, &ctx, as_of, MetricId::ProtectionLegPv);
    let prot_5y = metric_value(&idx_5y, &ctx, as_of, MetricId::ProtectionLegPv);
    let prot_10y = metric_value(&idx_10y, &ctx, as_of, MetricId::ProtectionLegPv);

    assert!(
        prot_3y < prot_5y && prot_5y < prot_10y,
        "Protection PV should increase with maturity: 3Y={}, 5Y={}, 10Y={}",
        prot_3y,
        prot_5y,
        prot_10y
    );
}

#[test]
fn test_single_curve_risky_pv01_maturity_impact() {
    // Test: Longer maturity increases risky PV01
    let start = date!(2025 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_3y = standard_single_curve_index("CDX-3Y", start, date!(2028 - 01 - 01), 10_000_000.0);
    let idx_5y = standard_single_curve_index("CDX-5Y", start, date!(2030 - 01 - 01), 10_000_000.0);

    let rpv01_3y = metric_value(&idx_3y, &ctx, as_of, MetricId::RiskyPv01);
    let rpv01_5y = metric_value(&idx_5y, &ctx, as_of, MetricId::RiskyPv01);

    assert!(
        rpv01_3y < rpv01_5y,
        "Risky PV01 should increase with maturity: 3Y={}, 5Y={}",
        rpv01_3y,
        rpv01_5y
    );
}

#[test]
fn test_single_curve_synthetic_cds_pricing_equivalence() {
    // Test: Direct pricing matches synthetic CDS pricing
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx = standard_single_curve_index("CDX-EQUIV", start, end, 10_000_000.0);
    let synthetic_cds = idx.to_synthetic_cds();

    let idx_npv = idx.value(&ctx, as_of).unwrap();
    let cds_npv = synthetic_cds.value(&ctx, as_of).unwrap();

    assert_money_approx_eq(
        idx_npv,
        cds_npv,
        50.0,
        "Index and synthetic CDS NPV should match",
    );
}

#[test]
fn test_single_curve_upfront_payment() {
    // Test: Upfront payment affects NPV
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let mut idx = standard_single_curve_index("CDX-UPFRONT", start, end, 10_000_000.0);
    let npv_no_upfront = idx.value(&ctx, as_of).unwrap();

    // Add upfront payment
    let upfront = Money::new(100_000.0, Currency::USD);
    idx.pricing_overrides.market_quotes.upfront_payment = Some(upfront);

    let npv_with_upfront = idx.value(&ctx, as_of).unwrap();
    let expected = npv_no_upfront.checked_sub(upfront).unwrap();

    assert_money_approx_eq(
        npv_with_upfront,
        expected,
        1.0,
        "Upfront (buyer pays) should reduce protection buyer NPV",
    );
}
