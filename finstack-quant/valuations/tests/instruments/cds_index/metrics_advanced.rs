//! CDS Index advanced metrics tests.
//!
//! Tests cover:
//! - Jump-to-Default (JTD)
//! - Expected Loss
//! - Theta (time decay)
//! - Bucketed DV01 (term structure sensitivity)

use super::test_utils::*;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::constants::isda::STANDARD_RECOVERY_SENIOR;
use finstack_quant_valuations::instruments::credit_derivatives::cds::PayReceive;
use finstack_quant_valuations::instruments::credit_derivatives::cds_index::CDSIndex;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use time::macros::date;

#[test]
fn test_jump_to_default_positive_for_protection_buyer() {
    // Test: JTD is positive for protection buyer (gain on default)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-JTD", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    assert_positive(jtd, "JTD for protection buyer");
}

#[test]
fn test_jump_to_default_negative_for_protection_seller() {
    // Test: JTD is negative for protection seller (loss on default)
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = CDSIndex::from_preset(
        &standard_cdx_params(),
        "CDX-JTD-SELL",
        Money::new(10_000_000.0, Currency::USD),
        PayReceive::Receive, // Sell protection
        start,
        end,
        STANDARD_RECOVERY_SENIOR,
        "USD-OIS",
        "HZ-INDEX",
    )
    .expect("valid test parameters");

    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    assert!(
        jtd < 0.0,
        "JTD should be negative for protection seller, got {}",
        jtd
    );
}

#[test]
fn test_jump_to_default_with_constituents() {
    // Test: JTD with explicit constituents
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_constituents_index("CDX-JTD-CONST", start, end, 10_000_000.0, 5);
    let ctx = multi_constituent_market_context(as_of, 5);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    assert!(jtd.is_finite());
    assert_positive(jtd, "JTD with constituents");
}

#[test]
fn test_jump_to_default_scales_with_notional() {
    // Test: JTD scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-10MM", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-20MM", start, end, 20_000_000.0);

    let result_10mm = idx_10mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_20mm = idx_20mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let jtd_10mm = *result_10mm.measures.get("jump_to_default").unwrap();
    let jtd_20mm = *result_20mm.measures.get("jump_to_default").unwrap();

    // JTD = (1 - Recovery) × Notional / Count is purely linear; expect exact scaling
    assert_linear_scaling(jtd_10mm, 10_000_000.0, jtd_20mm, 20_000_000.0, "JTD", 1e-10);
}

#[test]
fn test_jump_to_default_reasonable_magnitude() {
    // Test: JTD has reasonable magnitude
    // For CDX IG (125 names), $10MM, 40% recovery: ~$48K per name
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-JTD", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    // For standard index: expect $30K-$70K range per name default
    assert_in_range(jtd, 20_000.0, 100_000.0, "JTD magnitude");
}

#[test]
fn test_expected_loss_positive() {
    // Test: Expected loss is positive
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-EL", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let el = *result.measures.get("expected_loss").unwrap();

    assert_positive(el, "Expected loss");
}

#[test]
fn test_expected_loss_scales_with_notional() {
    // Test: Expected loss scales linearly with notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_10mm = standard_single_curve_index("CDX-10MM", start, end, 10_000_000.0);
    let idx_20mm = standard_single_curve_index("CDX-20MM", start, end, 20_000_000.0);

    let result_10mm = idx_10mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_20mm = idx_20mm
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let el_10mm = *result_10mm.measures.get("expected_loss").unwrap();
    let el_20mm = *result_20mm.measures.get("expected_loss").unwrap();

    // Expected loss scales linearly with notional; expect exact scaling
    assert_linear_scaling(
        el_10mm,
        10_000_000.0,
        el_20mm,
        20_000_000.0,
        "Expected loss",
        1e-10,
    );
}

#[test]
fn test_expected_loss_increases_with_maturity() {
    // Test: Expected loss increases with longer maturity
    let start = date!(2025 - 01 - 01);
    let as_of = start;
    let ctx = standard_market_context(as_of);

    let idx_3y = standard_single_curve_index("CDX-3Y", start, date!(2028 - 01 - 01), 10_000_000.0);
    let idx_5y = standard_single_curve_index("CDX-5Y", start, date!(2030 - 01 - 01), 10_000_000.0);

    let result_3y = idx_3y
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_5y = idx_5y
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let el_3y = *result_3y.measures.get("expected_loss").unwrap();
    let el_5y = *result_5y.measures.get("expected_loss").unwrap();

    assert!(
        el_3y < el_5y,
        "Expected loss should increase with maturity: 3Y={}, 5Y={}",
        el_3y,
        el_5y
    );
}

#[test]
fn test_theta_calculation() {
    // Test: Theta (time decay) calculation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-THETA", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::Theta],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Theta may or may not be present depending on implementation
    if let Some(theta) = result.measures.get("theta") {
        assert!(theta.is_finite(), "Theta should be finite");
    }
}

#[test]
fn test_bucketed_dv01_calculation() {
    // Test: Bucketed DV01 calculation
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-BUCKETED", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::BucketedDv01],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    // Bucketed DV01 may be present
    if let Some(bucketed) = result.measures.get("bucketed_dv01") {
        assert!(bucketed.is_finite(), "Bucketed DV01 should be finite");
    }
}

#[test]
fn test_all_advanced_metrics_together() {
    // Test: All advanced metrics computed together
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-ALL-ADV", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let metrics = vec![
        MetricId::JumpToDefault,
        MetricId::ExpectedLoss,
        MetricId::Theta,
    ];

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    assert!(result.measures.contains_key("jump_to_default"));
    assert!(result.measures.contains_key("expected_loss"));
    // Theta may or may not be always available
}

#[test]
fn test_expected_loss_reasonable_magnitude() {
    // Test: Expected loss has reasonable magnitude
    // EL = Notional × PD × LGD
    // For 1.5% hazard over 5Y: PD ≈ 7%, LGD ≈ 60% → EL ≈ 4.2% of notional
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-EL", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let el = *result.measures.get("expected_loss").unwrap();

    // Expect 2%-8% of notional = $200K-$800K
    assert_in_range(el, 100_000.0, 1_000_000.0, "Expected loss magnitude");
}

#[test]
fn test_jump_to_default_constituents_vs_single() {
    // Test: JTD consistency across modes
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_const = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let result_single = idx_single
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_const = idx_const
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let jtd_single = *result_single.measures.get("jump_to_default").unwrap();
    let jtd_const = *result_const.measures.get("jump_to_default").unwrap();

    // JTD methodologies differ significantly:
    // - Single curve: simplified calculation assuming 125 equal-weight names
    // - Constituents: actual weight-based calculation
    // Both should be positive and reasonable, but values may differ substantially
    assert_positive(jtd_single, "JTD single-curve");
    assert_positive(jtd_const, "JTD constituents");

    // Verify both are in reasonable ranges for $10MM notional
    assert_in_range(jtd_single, 1_000.0, 200_000.0, "JTD single-curve range");
    assert_in_range(jtd_const, 1_000.0, 10_000_000.0, "JTD constituents range");
}

#[test]
fn test_expected_loss_constituents_vs_single() {
    // Test: Expected loss consistency across modes
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;
    let ctx = multi_constituent_market_context(as_of, 5);

    let idx_single = standard_single_curve_index("CDX-SINGLE", start, end, 10_000_000.0);
    let idx_const = standard_constituents_index("CDX-CONST", start, end, 10_000_000.0, 5);

    let result_single = idx_single
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let result_const = idx_const
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::ExpectedLoss],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    let el_single = *result_single.measures.get("expected_loss").unwrap();
    let el_const = *result_const.measures.get("expected_loss").unwrap();

    // Expected loss should be identical (<0.1%) - both modes use the same
    // hazard rates, recovery, and integration methodology
    assert_relative_eq(el_single, el_const, 0.001, "Expected loss cross-mode");
}

#[test]
fn test_advanced_metrics_finite() {
    // Test: All advanced metrics are finite
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-FINITE", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let metrics = vec![MetricId::JumpToDefault, MetricId::ExpectedLoss];

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &metrics,
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();

    for (name, value) in &result.measures {
        assert!(
            value.is_finite(),
            "Advanced metric '{}' is not finite: {}",
            name,
            value
        );
    }
}

#[test]
fn test_jtd_per_name_basis() {
    // Test: JTD on per-name basis matches analytical formula exactly
    // For 125-name index, JTD per name = (1/125) × Notional × LGD
    //
    // The implementation uses:
    //   - num_constituents = 125 (CDX IG standard)
    //   - recovery_rate = STANDARD_RECOVERY_SENIOR = 0.40
    //   - LGD = 1 - 0.40 = 0.60
    //   - JTD = (1/125) × $10MM × 0.60 = $48,000
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-JTD", start, end, 10_000_000.0);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    // Exact formula: (1/125) × Notional × (1 - Recovery)
    // = (1/125) × $10MM × 0.60 = $48,000
    let per_name_estimate = 10_000_000.0 * 0.6 / 125.0;

    // JTD should match the analytical formula exactly (within machine epsilon)
    // Both use identical calculations: JTD = (1/125) × Notional × LGD
    assert_relative_eq(jtd, per_name_estimate, 1e-10, "JTD per-name basis");
}

#[test]
fn test_jtd_uses_explicit_constituent_count_over_name_inference() {
    // Regression (W-19): an index whose name maps to a hardcoded pool size
    // by substring inference but whose actual series has a different
    // membership must use the explicitly supplied `num_constituents`.
    //
    // "CDX.NA.HY" infers 100 names; this off-series index has only 97.
    // JTD per name must be (1/97) × Notional × LGD, NOT (1/100) × ...
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let idx = standard_single_curve_index("CDX-HY-OFFSERIES", start, end, 10_000_000.0)
        .with_num_constituents(97);
    let ctx = standard_market_context(as_of);

    let result = idx
        .price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap();
    let jtd = *result.measures.get("jump_to_default").unwrap();

    // Expected with the supplied count of 97.
    let expected_97 = 10_000_000.0 * (1.0 - STANDARD_RECOVERY_SENIOR) / 97.0;
    assert_relative_eq(jtd, expected_97, 1e-10, "JTD uses explicit count of 97");

    // And it must NOT equal the count the name (or preset default) would imply.
    let with_default_125 = 10_000_000.0 * (1.0 - STANDARD_RECOVERY_SENIOR) / 125.0;
    assert!(
        (jtd - with_default_125).abs() > 1.0,
        "JTD must not fall back to the preset/inferred pool size: jtd={jtd}, \
         default-125={with_default_125}"
    );
}

#[test]
fn test_jtd_per_name_independent_of_index_factor() {
    // Regression (2026-07 credit-derivatives audit F1): per-name JTD on a
    // seasoned (factored) index must be independent of `index_factor`.
    //
    // Markit index mechanics: a trade of `Notional` (original-notional
    // quotation) on a series with factor f has current notional f·Notional
    // spread over f·N_orig surviving names, so each surviving name carries
    // its original per-name notional Notional/N_orig and
    //
    //     JTD_per_name = LGD × Notional / N_orig     (no factor term).
    //
    // The old SingleCurve branch multiplied by `index_factor` while dividing
    // by the ORIGINAL pool size, understating JTD by exactly `f` (e.g. -8%
    // at f = 0.92) and disagreeing with the Constituents branch.
    let start = date!(2025 - 01 - 01);
    let end = date!(2030 - 01 - 01);
    let as_of = start;

    let jtd_at_factor = |factor: f64| -> f64 {
        let idx = standard_single_curve_index("CDX-JTD-FACTOR", start, end, 10_000_000.0)
            .with_index_factor(factor);
        let ctx = standard_market_context(as_of);
        *idx.price_with_metrics(
            &ctx,
            as_of,
            &[MetricId::JumpToDefault],
            finstack_quant_valuations::instruments::PricingOptions::default(),
        )
        .unwrap()
        .measures
        .get("jump_to_default")
        .unwrap()
    };

    // Analytical value: LGD × Notional / N_orig = 0.60 × $10MM / 125 = $48,000.
    let expected = 10_000_000.0 * (1.0 - STANDARD_RECOVERY_SENIOR) / 125.0;

    let jtd_seasoned = jtd_at_factor(0.92);
    assert_relative_eq(
        jtd_seasoned,
        expected,
        1e-10,
        "seasoned-index (factor 0.92) per-name JTD",
    );

    // Factor must not change the per-name number at all.
    let jtd_fresh = jtd_at_factor(1.0);
    assert_relative_eq(
        jtd_seasoned,
        jtd_fresh,
        1e-12,
        "per-name JTD must be independent of index_factor",
    );
}
