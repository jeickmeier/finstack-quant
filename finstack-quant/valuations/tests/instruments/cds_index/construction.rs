//! CDS Index construction and builder tests.
//!
//! Tests cover:
//! - Standard constructors with different parameters
//! - Builder methods for constituents
//! - Weight validation and normalization
//! - Index factor configuration
//! - Pricing mode switching
//! - Trait implementations

use super::test_utils::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::{
    CDSIndex, CDSIndexConstituent, CDSIndexParams, IndexPricing,
};
use finstack_quant_valuations::instruments::CreditParams;
use finstack_quant_valuations::instruments::Instrument;
use time::macros::date;

const TEST_NOTIONAL: f64 = 10_000_000.0;
const TEST_RECOVERY: f64 = 0.40;

#[test]
fn test_construction_single_curve_default() {
    // Test: Default construction creates single-curve index
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-TEST", start, end, 10_000_000.0);

    assert_eq!(idx.id(), "CDX-TEST");
    assert_eq!(idx.index_name, "CDX.NA.IG");
    assert_eq!(idx.series, 42);
    assert_eq!(idx.version, 1);
    assert_eq!(idx.notional.amount(), 10_000_000.0);
    assert_eq!(idx.notional.currency(), Currency::USD);
    assert_eq!(idx.index_factor, 1.0);
    assert_eq!(idx.pricing, IndexPricing::SingleCurve);
    assert!(idx.constituents.is_empty());
}

#[test]
fn test_construction_with_index_factor() {
    // Test: Index factor is properly set
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let factor = 0.95; // 95% surviving notional

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-FACTOR",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_index_factor(factor);

    assert_eq!(idx.index_factor, factor);
}

#[test]
fn test_construction_with_constituents() {
    // Test: Construction with constituents switches to Constituents mode
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    assert_eq!(idx.pricing, IndexPricing::Constituents);
    assert_eq!(idx.constituents.len(), 5);

    // Verify weights sum to 1.0
    let weight_sum: f64 = idx.constituents.iter().map(|c| c.weight).sum();
    assert!(
        (weight_sum - 1.0).abs() < 1e-10,
        "Weights should sum to 1.0"
    );
}

#[test]
fn test_builder_with_constituents_equal_weight() {
    // Test: Builder method creates equal-weighted constituents
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let mut idx = standard_single_curve_index("CDX-BUILD", start, end, 10_000_000.0);
    assert_eq!(idx.pricing, IndexPricing::SingleCurve);

    // Add constituents via builder
    let credits = (0..3)
        .map(|i| CreditParams::corporate_standard(format!("NAME{}", i), format!("HZ{}", i)))
        .collect::<Vec<_>>();

    idx = idx.with_constituents_equal_weight(credits);

    assert_eq!(idx.pricing, IndexPricing::Constituents);
    assert_eq!(idx.constituents.len(), 3);

    for constituent in &idx.constituents {
        assert!((constituent.weight - 1.0 / 3.0).abs() < 1e-10);
    }
}

#[test]
fn test_builder_with_explicit_weights() {
    // Test: Builder accepts explicit weights
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-EXPLICIT", start, end, 10_000_000.0);

    let constituents = vec![
        CDSIndexConstituent {
            credit: CreditParams::corporate_standard("N1", "HZ1"),
            weight: 0.5,
            defaulted: false,
        },
        CDSIndexConstituent {
            credit: CreditParams::corporate_standard("N2", "HZ2"),
            weight: 0.3,
            defaulted: false,
        },
        CDSIndexConstituent {
            credit: CreditParams::corporate_standard("N3", "HZ3"),
            weight: 0.2,
            defaulted: false,
        },
    ];

    let idx = idx.with_constituents(constituents);

    assert_eq!(idx.constituents.len(), 3);
    assert_eq!(idx.constituents[0].weight, 0.5);
    assert_eq!(idx.constituents[1].weight, 0.3);
    assert_eq!(idx.constituents[2].weight, 0.2);
}

#[test]
fn test_builder_empty_constituents_reverts_to_single_curve() {
    // Test: Empty constituents list reverts to single-curve mode
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_constituents_index("CDX-REVERT", start, end, 10_000_000.0, 5);
    assert_eq!(idx.pricing, IndexPricing::Constituents);

    let idx = idx.with_constituents(vec![]);

    assert_eq!(idx.pricing, IndexPricing::SingleCurve);
    assert!(idx.constituents.is_empty());
}

#[test]
fn test_weight_sum_validation() {
    // Test: Weights must sum to approximately 1.0
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    // Create index with weights summing to ~1.0
    let constituents = vec![
        CDSIndexConstituent::active(CreditParams::corporate_standard("N1", "HZ1"), 0.34),
        CDSIndexConstituent::active(CreditParams::corporate_standard("N2", "HZ2"), 0.33),
        CDSIndexConstituent::active(CreditParams::corporate_standard("N3", "HZ3"), 0.33),
    ];

    let idx = CDSIndex::from_preset(
        &CDSIndexParams::cdx_na_ig(42, 1, 100.0),
        "CDX-WEIGHTS",
        Money::new(TEST_NOTIONAL, Currency::USD),
        PayReceive::Pay,
        start,
        end,
        TEST_RECOVERY,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters")
    .with_constituents(constituents);

    let weight_sum: f64 = idx.constituents.iter().map(|c| c.weight).sum();
    assert!(
        (weight_sum - 1.0).abs() < 1e-8,
        "Weight sum = {}",
        weight_sum
    );

    // Test pricing works with valid weights
    let ctx = multi_constituent_market_context(as_of, 3);
    let result = idx.value(&ctx, as_of);
    assert!(result.is_ok(), "Pricing should succeed with valid weights");
}

#[test]
fn test_cdx_na_ig_params() {
    // Test: CDX NA IG constructor
    let params = CDSIndexParams::cdx_na_ig(42, 1, 100.0);

    assert_eq!(params.index_name, "CDX.NA.IG");
    assert_eq!(params.series, 42);
    assert_eq!(params.version, 1);
    assert_eq!(params.fixed_coupon_bp, 100.0);
}

#[test]
fn test_cdx_na_hy_params() {
    // Test: CDX NA HY constructor
    let params = CDSIndexParams::cdx_na_hy(39, 2, 500.0);

    assert_eq!(params.index_name, "CDX.NA.HY");
    assert_eq!(params.series, 39);
    assert_eq!(params.version, 2);
    assert_eq!(params.fixed_coupon_bp, 500.0);
}

#[test]
fn test_itraxx_europe_params() {
    // Test: iTraxx Europe constructor
    let params = CDSIndexParams::itraxx_europe(41, 1, 25.0);

    assert_eq!(params.index_name, "iTraxx Europe");
    assert_eq!(params.series, 41);
    assert_eq!(params.version, 1);
    assert_eq!(params.fixed_coupon_bp, 25.0);
}

#[test]
fn test_to_synthetic_cds_conversion() {
    // Test: Conversion to synthetic CDS preserves key fields
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-SYNTH", start, end, 10_000_000.0);
    let cds = idx.to_synthetic_cds();

    assert_eq!(cds.id, idx.id);
    assert_eq!(cds.notional, idx.notional);
    assert_eq!(cds.side, idx.side);
    assert_eq!(cds.convention, idx.convention);
    assert_eq!(cds.premium.spread_bp, idx.premium.spread_bp);
    assert_eq!(cds.protection.recovery_rate, idx.protection.recovery_rate);
}

#[test]
fn test_instrument_trait_id() {
    // Test: Instrument trait returns correct ID
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-TRAIT-ID", start, end, 10_000_000.0);

    assert_eq!(idx.id(), "CDX-TRAIT-ID");
}

#[test]
fn test_instrument_trait_key() {
    // Test: Instrument trait returns CDSIndex key
    use finstack_quant_valuations::pricer::InstrumentType;

    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-KEY", start, end, 10_000_000.0);

    assert_eq!(idx.key(), InstrumentType::CDSIndex);
}

#[test]
fn test_instrument_trait_clone() {
    // Test: Instrument can be cloned
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-CLONE", start, end, 10_000_000.0);
    let cloned = idx.clone_box();

    assert_eq!(cloned.id(), idx.id());
}

#[test]
fn test_discount_curve_dependency() {
    // Test: discount curve dependency returns correct curve ID

    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-DISC", start, end, 10_000_000.0);

    let disc_id = idx
        .market_dependencies()
        .expect("market_dependencies")
        .curves
        .discount_curves
        .first()
        .cloned()
        .expect("CDS index should declare a discount curve");
    assert_eq!(disc_id.as_str(), "USD-OIS");
}

#[test]
fn test_attributes_default() {
    // Test: Attributes are initialized empty
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-ATTR", start, end, 10_000_000.0);

    assert!(idx.attributes.tags.is_empty());
}

#[test]
fn test_pricing_overrides_default() {
    // Test: Pricing overrides are initialized with defaults
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let idx = standard_single_curve_index("CDX-OVERRIDE", start, end, 10_000_000.0);

    assert!(idx
        .pricing_overrides
        .market_quotes
        .upfront_payment
        .is_none());
}

#[test]
fn test_multiple_index_types_construction() {
    // Test: Can construct different index types
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);

    let indices = vec![
        ("CDX.NA.IG", CDSIndexParams::cdx_na_ig(42, 1, 100.0)),
        ("CDX.NA.HY", CDSIndexParams::cdx_na_hy(39, 1, 500.0)),
        ("iTraxx Europe", CDSIndexParams::itraxx_europe(41, 1, 25.0)),
    ];

    for (name, params) in indices {
        let idx = CDSIndex::from_preset(
            &params,
            format!("{}-TEST", name),
            Money::new(TEST_NOTIONAL, Currency::USD),
            PayReceive::Pay,
            start,
            end,
            TEST_RECOVERY,
            "USD-OIS",
            "HZ-INDEX",
        )
        .expect("valid test parameters");

        assert_eq!(idx.index_name, name);
    }
}
