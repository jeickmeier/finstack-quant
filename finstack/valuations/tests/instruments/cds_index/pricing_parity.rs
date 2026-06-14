//! CDS Index pricing parity tests between single-curve and constituents modes.
//!
//! Market Standard: When all constituents have identical credit quality,
//! single-curve and constituents pricing should produce similar results.
//!
//! ## Tolerance Rationale
//!
//! With identical hazard rates and recovery, tolerances are tight:
//!
//! - **NPV/Leg PV (1%)**: Small interpolation differences from different
//!   curve lookups (HZ-INDEX vs HZ1..HZn).
//!
//! - **Par Spread (0.5%)**: Pure ratio, errors largely cancel.
//!
//! - **Risky PV01/CS01 (1%)**: Aggregation over periods, same methodology.
//!
//! Tests cover:
//! - NPV parity with equal hazard rates
//! - Par spread consistency
//! - Risky PV01 consistency
//! - CS01 consistency
//! - Protection/premium leg consistency

use super::test_utils::*;
use finstack_valuations::instruments::credit_derivatives::cds_index::IndexPricing;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

fn metric_value(
    index: &finstack_valuations::instruments::credit_derivatives::cds_index::CDSIndex,
    market: &finstack_core::market_data::context::MarketContext,
    as_of: finstack_core::dates::Date,
    metric: MetricId,
) -> f64 {
    let result = index
        .price_with_metrics(
            market,
            as_of,
            std::slice::from_ref(&metric),
            finstack_valuations::instruments::PricingOptions::default(),
        )
        .expect("metric should compute");
    result.measures[&metric]
}

#[test]
fn test_npv_parity_equal_hazards() {
    // Market Standard: Single curve vs constituents pricing with equal hazards
    //
    // Both modes use flat 0.015 hazard rate and 40% recovery, so NPV should
    // be very close. Differences arise from curve lookup paths only.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    // Single curve index
    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);

    // Constituents index with equal hazards
    let idx_constituents = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let npv_single = idx_single.value(&ctx, as_of).unwrap();
    let npv_constituents = idx_constituents.value(&ctx, as_of).unwrap();

    // With identical hazard rates and recovery, expect very close results
    // Market standard: 1% for same-data different-path calculation
    assert_relative_eq(
        npv_single.amount(),
        npv_constituents.amount(),
        0.01,
        "NPV parity between pricing modes",
    );
}

#[test]
fn test_par_spread_parity_equal_hazards() {
    // Test: Par spread consistency across modes
    //
    // Par spread = Protection PV / Risky Annuity is a ratio, so curve
    // interpolation differences should largely cancel out.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_constituents = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let par_single = metric_value(&idx_single, &ctx, as_of, MetricId::ParSpread);
    let par_constituents = metric_value(&idx_constituents, &ctx, as_of, MetricId::ParSpread);

    // Par spread is a ratio (protection PV / annuity), errors cancel
    assert_relative_eq(par_single, par_constituents, 0.005, "Par spread parity");
}

#[test]
fn test_risky_pv01_parity_equal_hazards() {
    // Test: Risky PV01 consistency across modes
    //
    // Risky PV01 (annuity) is sum of discounted survival probabilities.
    // Aggregation across N constituents vs single calculation.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_constituents = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let rpv01_single = metric_value(&idx_single, &ctx, as_of, MetricId::RiskyPv01);
    let rpv01_constituents = metric_value(&idx_constituents, &ctx, as_of, MetricId::RiskyPv01);

    // Annuity is sum of discounted survival - tight for identical hazards
    assert_relative_eq(rpv01_single, rpv01_constituents, 0.01, "Risky PV01 parity");
}

#[test]
fn test_cs01_parity_equal_hazards() {
    // Test: CS01 consistency across modes
    //
    // CS01 uses finite-difference bumping of hazard curves:
    // - Single-curve: bumps HZ-INDEX by 1bp
    // - Constituents: bumps each HZ1..HZn by 1bp independently
    //
    // With identical hazard rates, the sum of constituent CS01s should
    // equal the single-curve CS01.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_constituents = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let cs01_single = idx_single.cs01(&ctx, as_of).unwrap();
    let cs01_constituents = idx_constituents.cs01(&ctx, as_of).unwrap();

    // CS01 uses same bump size in both modes - tight for identical hazards
    assert_relative_eq(cs01_single, cs01_constituents, 0.01, "CS01 parity");
}

#[test]
fn test_protection_leg_parity_equal_hazards() {
    // Test: Protection leg PV consistency
    //
    // Protection leg integrates (1 - Recovery) × hazard × discount over time.
    // With identical hazard rates and recovery, results should be close.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_constituents = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let prot_single = metric_value(&idx_single, &ctx, as_of, MetricId::ProtectionLegPv);
    let prot_constituents = metric_value(&idx_constituents, &ctx, as_of, MetricId::ProtectionLegPv);

    // Protection leg integral - tight for identical hazards/recovery
    assert_relative_eq(
        prot_single,
        prot_constituents,
        0.01,
        "Protection leg parity",
    );
}

#[test]
fn test_premium_leg_parity_equal_hazards() {
    // Test: Premium leg PV consistency
    //
    // Premium leg is spread × risky annuity + accrued-on-default.
    // With identical parameters, aggregation should match.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_constituents = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let prem_single = metric_value(&idx_single, &ctx, as_of, MetricId::PremiumLegPv);
    let prem_constituents = metric_value(&idx_constituents, &ctx, as_of, MetricId::PremiumLegPv);

    // Premium leg = spread × annuity - tight for identical hazards
    assert_relative_eq(prem_single, prem_constituents, 0.01, "Premium leg parity");
}

#[test]
fn test_mode_switching_preserves_basics() {
    // Test: Switching pricing mode maintains core fields
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let mut idx = standard_single_curve_index("CDX-SWITCH", start, end, 10_000_000.0);
    assert_eq!(idx.pricing, IndexPricing::SingleCurve);

    // Switch to constituents
    let credits = equal_weight_constituents(5)
        .into_iter()
        .map(|c| c.credit)
        .collect::<Vec<_>>();
    idx = idx.with_constituents_equal_weight(credits);

    // Verify basics preserved
    assert_eq!(idx.id(), "CDX-SWITCH");
    assert_eq!(idx.notional.amount(), 10_000_000.0);
    assert_eq!(idx.pricing, IndexPricing::Constituents);
}

#[test]
fn test_parity_with_different_constituent_counts() {
    // Test: Parity holds across different constituent counts
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);

    for count in [3, 5, 10, 25] {
        let id = format!("CDX-N{}", count);
        let idx_const = standard_constituents_index(&id, start, end, 10_000_000.0, count);

        let ctx = multi_constituent_market_context(as_of, count);

        let npv_single = idx_single.value(&ctx, as_of).unwrap();
        let npv_const = idx_const.value(&ctx, as_of).unwrap();

        let msg = format!("NPV parity with {} constituents", count);
        // Constituent count shouldn't affect parity with identical hazards
        assert_relative_eq(npv_single.amount(), npv_const.amount(), 0.01, &msg);
    }
}

#[test]
fn test_parity_across_maturities() {
    // Test: Parity holds across different maturities
    let start = date!(2025 - 01 - 01);
    let as_of = start;

    let maturities = [
        ("3Y", date!(2028 - 01 - 01)),
        ("5Y", date!(2030 - 01 - 01)),
        ("7Y", date!(2032 - 01 - 01)),
    ];

    for (label, end) in maturities {
        let single_id = format!("CDX-SINGLE-{}", label);
        let const_id = format!("CDX-CONST-{}", label);
        let idx_single = standard_single_curve_index(&single_id, start, end, 10_000_000.0);
        let idx_const = standard_constituents_index(&const_id, start, end, 10_000_000.0, 5);

        let ctx = multi_constituent_market_context(as_of, 5);

        let npv_single = idx_single.value(&ctx, as_of).unwrap();
        let npv_const = idx_const.value(&ctx, as_of).unwrap();

        let msg = format!("NPV parity for {} maturity", label);
        // Short maturities have more interpolation error between curve knots
        assert_relative_eq(npv_single.amount(), npv_const.amount(), 0.02, &msg);
    }
}

#[test]
fn test_parity_with_different_notionals() {
    // Test: Relative parity holds across notional sizes
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    for notional in [1_000_000.0, 10_000_000.0, 100_000_000.0] {
        let single_id = format!("CDX-SINGLE-{}", notional);
        let const_id = format!("CDX-CONST-{}", notional);
        let idx_single = standard_single_curve_index(&single_id, start, end, notional);
        let idx_const = standard_constituents_index(&const_id, start, end, notional, 5);

        let npv_single = idx_single.value(&ctx, as_of).unwrap();
        let npv_const = idx_const.value(&ctx, as_of).unwrap();

        let msg = format!("NPV parity for ${:.0} notional", notional);
        // Notional scales linearly, shouldn't affect relative parity
        assert_relative_eq(npv_single.amount(), npv_const.amount(), 0.01, &msg);
    }
}

#[test]
fn test_consistent_sign_conventions_across_modes() {
    // Test: Sign conventions consistent between modes
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_const = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let ctx = multi_constituent_market_context(as_of, 5);

    let npv_single = idx_single.value(&ctx, as_of).unwrap().amount();
    let npv_const = idx_const.value(&ctx, as_of).unwrap().amount();

    // Both should have the same sign
    assert!(
        npv_single.signum() == npv_const.signum(),
        "NPV signs should match: single={}, const={}",
        npv_single,
        npv_const
    );
}
