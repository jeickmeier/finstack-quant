//! CDS Index constituents pricing mode tests.
//!
//! Tests cover:
//! - NPV aggregation across constituents
//! - Weight-based notional allocation
//! - Index factor application
//! - Par spread aggregation methods
//! - Risky PV01 aggregation
//! - CS01 aggregation
//! - Weight validation and normalization

use super::test_utils::*;
use finstack_core::currency::Currency;
use finstack_core::money::Money;
use finstack_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_valuations::constants::BASIS_POINTS_PER_UNIT;
use finstack_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, IndexPricing, ParSpreadMethod,
};
use finstack_valuations::instruments::CreditParams;
use finstack_valuations::instruments::Instrument;
use finstack_valuations::metrics::MetricId;
use time::macros::date;

const TEST_NOTIONAL: f64 = 10_000_000.0;

fn metric_value(
    index: &CDSIndex,
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
fn test_constituents_mode_pricing() {
    // Test: Basic constituents pricing works
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let npv = idx.value(&ctx, as_of).unwrap();

    assert!(npv.amount().is_finite());
    assert_eq!(idx.pricing, IndexPricing::Constituents);
}

#[test]
fn test_constituents_weight_based_allocation() {
    // Test: Notional is allocated according to weights
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let total_notional = 10_000_000.0;
    let idx = standard_constituents_index("CDX-WEIGHT", start, end, total_notional, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    // Verify pricing succeeds
    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());

    // Each constituent should have weight = 1/5 = 0.20
    for constituent in &idx.constituents {
        assert_relative_eq(constituent.weight, 0.20, 0.001, "Constituent weight");
    }
}

#[test]
fn test_constituents_npv_components() {
    // Test: NPV = Protection PV - Premium PV (for protection buyer)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-COMP", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

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
fn test_constituents_par_spread() {
    // Test: Par spread calculation with constituents
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-PAR", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let par_spread = metric_value(&idx, &ctx, as_of, MetricId::ParSpread);

    assert_positive(par_spread, "Par spread");
    let expected = flat_hazard_par_spread_bps(STANDARD_HAZARD_RATE, STANDARD_RECOVERY_SENIOR);
    assert_in_range(
        par_spread,
        expected * 0.85,
        expected * 1.15,
        "Par spread near flat-hazard analytic",
    );
}

#[test]
fn test_constituents_risky_pv01() {
    // Test: Risky PV01 aggregation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-RPV01", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let rpv01 = metric_value(&idx, &ctx, as_of, MetricId::RiskyPv01);

    assert_positive(rpv01, "Risky PV01");
    assert_in_range(rpv01, 3_500.0, 5_500.0, "Risky PV01 magnitude");
}

#[test]
fn test_constituents_cs01() {
    // Test: CS01 aggregation across constituents
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-CS01", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let cs01 = idx.cs01(&ctx, as_of).unwrap();

    assert_positive(cs01, "CS01");
}

#[test]
fn test_constituents_weights_sum_to_one() {
    // Test: Constituent weights must sum to 1.0
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    for num_constituents in [3, 5, 10, 125] {
        let id = format!("CDX-W{}", num_constituents);
        let idx = standard_constituents_index(&id, start, end, 10_000_000.0, num_constituents);

        let weight_sum: f64 = idx.constituents.iter().map(|c| c.weight).sum();
        assert!(
            (weight_sum - 1.0).abs() < 1e-10,
            "Weights should sum to 1.0 for {} constituents, got {}",
            num_constituents,
            weight_sum
        );
    }
}

#[test]
fn test_constituents_unequal_weights() {
    // Test: Support for unequal constituent weights
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let constituents = vec![
        CDSIndexConstituent::active(CreditParams::corporate_standard("N1", "HZ1"), 0.50), // Large
        CDSIndexConstituent::active(CreditParams::corporate_standard("N2", "HZ2"), 0.30),
        CDSIndexConstituent::active(CreditParams::corporate_standard("N3", "HZ3"), 0.20), // Small
    ];

    let idx = CDSIndex::from_preset(
        &standard_cdx_params(),
        "CDX-UNEQUAL",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        STANDARD_RECOVERY_SENIOR,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(constituents);

    let ctx = multi_constituent_market_context(as_of, 3);
    let result = idx.value(&ctx, as_of);

    assert!(result.is_ok(), "Unequal weights should be supported");
}

#[test]
fn test_constituents_index_factor_application() {
    // Test: Index factor scales effective notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let factor = 0.95; // 95% surviving
    let constituents = equal_weight_constituents(5);

    let idx = CDSIndex::from_preset(
        &standard_cdx_params(),
        "CDX-FACTOR",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        STANDARD_RECOVERY_SENIOR,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_index_factor(factor)
    .with_constituents(constituents);

    assert_eq!(idx.index_factor, factor);

    let ctx = multi_constituent_market_context(as_of, 5);
    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());
}

#[test]
fn test_constituents_npv_scales_with_notional() {
    // Test: NPV scales linearly with notional in constituents mode
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_10mm = standard_constituents_index("CDX-10MM", start, end, 10_000_000.0, 5);
    let idx_20mm = standard_constituents_index("CDX-20MM", start, end, 20_000_000.0, 5);

    let npv_10mm = idx_10mm.value(&ctx, as_of).unwrap().amount();
    let npv_20mm = idx_20mm.value(&ctx, as_of).unwrap().amount();

    assert_linear_scaling(npv_10mm, 10_000_000.0, npv_20mm, 20_000_000.0, "NPV", 0.01);
}

#[test]
fn test_constituents_risky_pv01_scales_with_notional() {
    // Test: Risky PV01 scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_10mm = standard_constituents_index("CDX-10MM", start, end, 10_000_000.0, 5);
    let idx_20mm = standard_constituents_index("CDX-20MM", start, end, 20_000_000.0, 5);

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
fn test_constituents_cs01_scales_with_notional() {
    // Test: CS01 scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_10mm = standard_constituents_index("CDX-10MM", start, end, 10_000_000.0, 5);
    let idx_20mm = standard_constituents_index("CDX-20MM", start, end, 20_000_000.0, 5);

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
fn test_constituents_par_spread_independent_of_notional() {
    // Test: Par spread independent of notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_1mm = standard_constituents_index("CDX-1MM", start, end, 1_000_000.0, 5);
    let idx_100mm = standard_constituents_index("CDX-100MM", start, end, 100_000_000.0, 5);

    let par_1mm = metric_value(&idx_1mm, &ctx, as_of, MetricId::ParSpread);
    let par_100mm = metric_value(&idx_100mm, &ctx, as_of, MetricId::ParSpread);

    assert_relative_eq(par_1mm, par_100mm, 0.001, "Par spread notional-independent");
}

#[test]
fn test_constituents_different_counts() {
    // Test: Various constituent counts work correctly
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    for count in [3, 5, 10, 25, 125] {
        let id = format!("CDX-N{}", count);
        let idx = standard_constituents_index(&id, start, end, 10_000_000.0, count);
        let ctx = multi_constituent_market_context(as_of, count);

        let result = idx.value(&ctx, as_of);
        assert!(
            result.is_ok(),
            "Pricing should work for {} constituents",
            count
        );
    }
}

#[test]
fn test_constituents_pricing_mode_verification() {
    // Test: Constituents index has correct pricing mode
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_constituents_index("CDX-MODE", start, end, 10_000_000.0, 5);

    assert_eq!(idx.pricing, IndexPricing::Constituents);
    assert_eq!(idx.constituents.len(), 5);
}

#[test]
fn test_constituents_protection_leg_positive() {
    // Test: Protection leg PV is positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-PROT", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let pv_prot = metric_value(&idx, &ctx, as_of, MetricId::ProtectionLegPv);

    assert_positive(pv_prot, "Protection leg PV");
}

#[test]
fn test_constituents_premium_leg_positive() {
    // Test: Premium leg PV is positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-PREM", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let pv_prem = metric_value(&idx, &ctx, as_of, MetricId::PremiumLegPv);

    assert_positive(pv_prem, "Premium leg PV");
}

#[test]
fn test_constituents_single_constituent() {
    // Test: Single constituent case works (edge case)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-SINGLE", start, end, 10_000_000.0, 1);
    let ctx = multi_constituent_market_context(as_of, 1);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());

    assert_eq!(idx.constituents.len(), 1);
    assert_relative_eq(
        idx.constituents[0].weight,
        1.0,
        0.001,
        "Single constituent weight",
    );
}

#[test]
fn test_constituents_large_basket() {
    // Test: Large basket (125 names) works efficiently
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-IG-125", start, end, 10_000_000.0, 125);
    let ctx = multi_constituent_market_context(as_of, 125);

    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok());

    assert_eq!(idx.constituents.len(), 125);
}

#[test]
fn test_constituents_detailed_additive_metrics() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-DETAIL-ADD", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let npv = idx.npv_detailed(&ctx, as_of).unwrap();
    let npv_sum = npv
        .constituents
        .iter()
        .fold(Money::new(0.0, npv.total.currency()), |acc, c| {
            acc.checked_add(c.value).unwrap()
        });
    assert_money_approx_eq(npv.total, npv_sum, 1e-6, "NPV total equals sum");

    let pv_prot = idx.pv_protection_leg_detailed(&ctx, as_of).unwrap();
    let pv_prot_sum = pv_prot
        .constituents
        .iter()
        .fold(Money::new(0.0, pv_prot.total.currency()), |acc, c| {
            acc.checked_add(c.value).unwrap()
        });
    assert_money_approx_eq(
        pv_prot.total,
        pv_prot_sum,
        1e-6,
        "Protection leg PV total equals sum",
    );

    let pv_prem = idx.pv_premium_leg_detailed(&ctx, as_of).unwrap();
    let pv_prem_sum = pv_prem
        .constituents
        .iter()
        .fold(Money::new(0.0, pv_prem.total.currency()), |acc, c| {
            acc.checked_add(c.value).unwrap()
        });
    assert_money_approx_eq(
        pv_prem.total,
        pv_prem_sum,
        1e-6,
        "Premium leg PV total equals sum",
    );

    let rpv01 = idx.risky_pv01_detailed(&ctx, as_of).unwrap();
    let rpv01_sum: f64 = rpv01.constituents.iter().map(|c| c.value).sum();
    assert_relative_eq(rpv01.total, rpv01_sum, 1e-10, "Risky PV01 total equals sum");

    let cs01 = idx.cs01_detailed(&ctx, as_of).unwrap();
    let cs01_sum: f64 = cs01.constituents.iter().map(|c| c.value).sum();
    assert_relative_eq(cs01.total, cs01_sum, 1e-10, "CS01 total equals sum");
}

#[test]
fn test_constituents_detailed_weights_and_ids() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-DETAIL-WGT", start, end, 10_000_000.0, 3);
    let ctx = multi_constituent_market_context(as_of, 3);

    let detailed = idx.risky_pv01_detailed(&ctx, as_of).unwrap();
    let weight_sum: f64 = detailed
        .constituents
        .iter()
        .map(|c| c.weight_effective)
        .sum();
    assert_relative_eq(weight_sum, 1.0, 1e-12, "Effective weights sum to 1");

    for (i, c) in detailed.constituents.iter().enumerate() {
        let expected_curve = format!("HZ{}", i + 1);
        assert_eq!(c.credit_curve_id.as_str(), expected_curve);
        assert_relative_eq(c.weight_raw, 1.0 / 3.0, 1e-12, "Raw weight");
        assert_relative_eq(c.weight_effective, 1.0 / 3.0, 1e-12, "Effective weight");
        assert!(c.recovery_rate > 0.0 && c.recovery_rate < 1.0);
    }
}

#[test]
fn test_constituents_par_spread_detailed_non_additive() {
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-DETAIL-PAR", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let detailed = idx.par_spread_detailed(&ctx, as_of).unwrap();
    assert_eq!(detailed.constituents_spread_bp.len(), 5);

    let sum_spreads: f64 = detailed
        .constituents_spread_bp
        .iter()
        .map(|c| c.value)
        .sum();
    assert_relative_eq(
        sum_spreads,
        detailed.total_spread_bp * 5.0,
        1e-10,
        "Sum of constituent par spreads equals N * total",
    );

    let expected_total = match detailed.method {
        ParSpreadMethod::RiskyAnnuity => {
            detailed.numerator_protection_pv.amount() / detailed.denominator * BASIS_POINTS_PER_UNIT
        }
        ParSpreadMethod::FullPremiumAoD => {
            detailed.numerator_protection_pv.amount() / detailed.denominator
        }
    };
    assert_relative_eq(
        detailed.total_spread_bp,
        expected_total,
        1e-12,
        "Par spread total consistent with numerator/denominator",
    );
}
