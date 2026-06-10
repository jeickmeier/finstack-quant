//! Index ratio calculation tests for Inflation-Linked Bonds
//!
//! Tests cover:
//! - Index ratio calculations with linear interpolation (TIPS/Canadian)
//! - Index ratio with step interpolation (UK)
//! - Lag application (3-month, 8-month)
//! - Deflation protection (none, maturity-only, all payments)
//! - Index vs curve-based ratio calculations
//! - Market context routing

use super::common::*;
use finstack_core::dates::DateExt;
use finstack_core::market_data::scalars::{InflationInterpolation, InflationLag};
use finstack_valuations::instruments::fixed_income::inflation_linked_bond::{
    DeflationProtection, IndexationMethod,
};

#[test]
fn test_index_ratio_basic_linear_interpolation() {
    // Arrange
    let ilb = sample_tips();
    let (_, index) = market_context_with_index();

    // Verify interpolation method matches TIPS requirements
    assert_eq!(index.interpolation(), InflationInterpolation::Linear);

    // Act - calculate index ratio for a date with 3-month lag
    let ratio = ilb.index_ratio(d(2025, 4, 1), &index).unwrap();

    // Assert - ratio should be positive and reasonable (near 1.0 for small inflation)
    assert!(ratio > 0.9);
    assert!(ratio < 1.3);
}

#[test]
fn test_index_ratio_with_3month_lag() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.lag = InflationLag::Months(3);
    ilb.base_index = 300.0;
    ilb.base_date = d(2024, 12, 1);

    // Create index with specific values for precise lag testing
    let observations = vec![
        (d(2024, 12, 1), 300.0), // Reference for Jan 1 (3mo lag)
        (d(2025, 1, 1), 301.0),  // Reference for Apr 1 (3mo lag)
    ];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    // Act - valuation date Apr 1, 2025 → reference date Jan 1, 2025 (3-month lag)
    let ratio = ilb.index_ratio(d(2025, 4, 1), &index).unwrap();

    // Assert - should use Jan 1 CPI (301) vs base (300)
    assert_approx_eq(ratio, 301.0 / 300.0, REL_TOL, "3-month lag ratio");
}

#[test]
fn test_index_ratio_with_8month_lag_uk() {
    // Arrange
    let mut ilb = sample_uk_linker();
    ilb.lag = InflationLag::Months(8);
    ilb.base_index = 320.0;
    ilb.base_date = d(2024, 6, 1);

    // Create index with specific values for precise lag testing
    let observations = vec![
        (d(2024, 6, 1), 320.0), // Reference for Feb 1 (8mo lag)
        (d(2025, 2, 1), 326.4), // Reference for Oct 1 (8mo lag)
    ];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "UK-RPI",
        observations,
        finstack_core::currency::Currency::GBP,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Step);

    // Act - valuation date Oct 1, 2025 → reference date Feb 1, 2025 (8-month lag)
    let ratio = ilb.index_ratio(d(2025, 10, 1), &index).unwrap();

    // Assert
    assert_approx_eq(ratio, 326.4 / 320.0, REL_TOL, "8-month lag ratio");
}

/// Monthly US CPI-U (NSA) observations around early 2019, anchored at the
/// first of each month, as used by the published Treasury Ref CPI tables.
fn cpi_2019_index() -> finstack_core::market_data::scalars::InflationIndex {
    let observations = vec![
        (d(2019, 1, 1), 251.712), // CPI-U Jan 2019
        (d(2019, 2, 1), 252.776), // CPI-U Feb 2019
        (d(2019, 3, 1), 254.202), // CPI-U Mar 2019
        (d(2019, 4, 1), 255.548), // CPI-U Apr 2019
    ];
    finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear)
}

#[test]
fn test_ref_cpi_mid_month_matches_treasury_golden() {
    // Official TIPS formula for settlement 2019-04-15 with 3-month lag:
    // RefCPI = CPI(Jan) + (15−1)/30 × [CPI(Feb) − CPI(Jan)]
    //        = 251.712 + 14/30 × 1.064 = 252.20853 (Treasury-published value).
    let index = cpi_2019_index();
    let ref_cpi = index.ref_cpi_months_lag(d(2019, 4, 15), 3).unwrap();
    let expected = 251.712 + 14.0 / 30.0 * (252.776 - 251.712);
    assert_approx_eq(ref_cpi, expected, 1e-12, "mid-month RefCPI");
    assert!(
        (ref_cpi - 252.20853).abs() < 5e-6,
        "Treasury golden: {ref_cpi}"
    );

    // index_ratio routes through the same formula for TIPS-style bonds.
    let mut ilb = sample_tips();
    ilb.lag = InflationLag::Months(3);
    ilb.base_index = 251.712;
    let ratio = ilb.index_ratio(d(2019, 4, 15), &index).unwrap();
    assert_approx_eq(ratio, expected / 251.712, 1e-12, "mid-month index ratio");
}

#[test]
fn test_ref_cpi_end_of_month_31_day_month() {
    // Settlement 2019-05-31 (31-day month): weight = (31−1)/31 against the
    // Feb/Mar anchors. The previous generic `add_months(-3)` lookup clamped
    // Feb 31 → Feb 28 and interpolated on the calendar axis, producing a kink
    // at month ends; the official formula weights by the settlement month.
    let index = cpi_2019_index();
    let ref_cpi = index.ref_cpi_months_lag(d(2019, 5, 31), 3).unwrap();
    let expected = 252.776 + 30.0 / 31.0 * (254.202 - 252.776);
    assert_approx_eq(ref_cpi, expected, 1e-12, "end-of-month RefCPI");

    // One day later (June 1) the anchors roll to Mar/Apr with weight 0.
    let ref_cpi_next = index.ref_cpi_months_lag(d(2019, 6, 1), 3).unwrap();
    assert_approx_eq(ref_cpi_next, 254.202, 1e-12, "first-of-month RefCPI");
    // Continuity across the month boundary: the jump must be small (one
    // day's interpolation step), not a day-clamping artifact.
    assert!(
        (ref_cpi_next - ref_cpi).abs() < 0.1,
        "RefCPI discontinuity at month end: {ref_cpi} -> {ref_cpi_next}"
    );
}

#[test]
fn test_index_ratio_no_deflation_protection() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.deflation_protection = DeflationProtection::None;
    ilb.base_index = 300.0;

    // Create deflation scenario index
    let observations = vec![
        (d(2024, 10, 1), 295.0), // Deflation scenario
        (d(2025, 1, 1), 295.0),
    ];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    // Act - calculate ratio when CPI drops below base
    let ratio = ilb.index_ratio(d(2025, 1, 1), &index).unwrap();

    // Assert - no floor, ratio can be < 1.0
    assert!(ratio < 1.0);
    assert_approx_eq(ratio, 295.0 / 300.0, REL_TOL, "deflation no protection");
}

#[test]
fn test_index_ratio_maturity_only_deflation_protection() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.deflation_protection = DeflationProtection::MaturityOnly;
    ilb.base_index = 300.0;
    ilb.maturity = d(2025, 1, 15);

    // Create deflation scenario index
    let observations = vec![
        (d(2024, 10, 1), 295.0), // Deflation
        (d(2025, 1, 1), 295.0),
        (d(2025, 1, 15), 295.0),
    ];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    // index_ratio now returns the RAW ratio (no deflation floor).
    // Deflation protection is applied when building the cashflow schedule.
    let ratio_at_maturity = ilb.index_ratio(ilb.maturity, &index).unwrap();
    let ratio_before_maturity = ilb.index_ratio(d(2025, 1, 1), &index).unwrap();

    assert!(
        ratio_at_maturity < 1.0,
        "raw ratio should reflect deflation"
    );
    assert_approx_eq(
        ratio_at_maturity,
        295.0 / 300.0,
        REL_TOL,
        "raw maturity ratio",
    );
    assert!(ratio_before_maturity < 1.0);
}

#[test]
fn test_index_ratio_all_payments_deflation_protection() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.deflation_protection = DeflationProtection::AllPayments;
    ilb.base_index = 300.0;

    // Create deflation scenario index
    let observations = vec![
        (d(2024, 10, 1), 295.0), // Deflation
        (d(2025, 1, 1), 295.0),
    ];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    // index_ratio now returns the RAW ratio (no deflation floor).
    // Deflation protection is applied when building the cashflow schedule.
    let ratio = ilb.index_ratio(d(2025, 1, 1), &index).unwrap();

    assert!(ratio < 1.0, "raw ratio should reflect deflation");
    assert_approx_eq(ratio, 295.0 / 300.0, REL_TOL, "raw deflation ratio");
}

#[test]
fn test_index_ratio_from_curve() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 300.0;
    ilb.base_date = d(2024, 12, 1);

    let (_, curve) = market_context_with_curve();

    // Act - calculate ratio using inflation curve (forward projection)
    let ratio = ilb.index_ratio_from_curve(d(2026, 12, 1), &curve).unwrap();

    // Assert - ratio should reflect 2-year inflation at ~2% p.a.
    assert!(ratio > 1.03); // > 4% total
    assert!(ratio < 1.06); // < 6% total
}

#[test]
fn test_index_ratio_from_curve_official_weighting() {
    // The curve path follows the same official RefCPI weighting as the index
    // path: first-of-month anchors CPI(m−3)/CPI(m−2) weighted by (day−1)/D(m).
    // Query 2025-05-15 → anchors 2025-02-01 and 2025-03-01, weight 14/31.
    let mut ilb = sample_tips();
    ilb.base_index = 300.0;

    let (_, curve) = market_context_with_curve();

    let cpi_feb = curve.cpi_on_date(d(2025, 2, 1)).unwrap();
    let cpi_mar = curve.cpi_on_date(d(2025, 3, 1)).unwrap();
    let expected_ref_cpi = cpi_feb + 14.0 / 31.0 * (cpi_mar - cpi_feb);

    let ratio = ilb.index_ratio_from_curve(d(2025, 5, 15), &curve).unwrap();

    assert_approx_eq(
        ratio,
        expected_ref_cpi / 300.0,
        REL_TOL,
        "curve RefCPI weighting",
    );
}

#[test]
fn test_index_ratio_from_market_routes_to_index() {
    // Arrange
    let ilb = sample_tips();
    let (ctx, index) = market_context_with_index();

    // Act
    let ratio_from_market = ilb.index_ratio_from_market(d(2025, 4, 1), &ctx).unwrap();
    let ratio_from_index = ilb.index_ratio(d(2025, 4, 1), &index).unwrap();

    // Assert - should be identical
    assert_approx_eq(
        ratio_from_market,
        ratio_from_index,
        EPSILON,
        "market routing to index",
    );
}

#[test]
fn test_index_ratio_from_market_routes_to_curve() {
    // Arrange: use a date whose 3-month lagged reference is within the curve range
    let ilb = sample_tips();
    let (ctx, curve) = market_context_with_curve();

    let ratio_from_market = ilb.index_ratio_from_market(d(2025, 7, 1), &ctx).unwrap();
    let ratio_from_curve = ilb.index_ratio_from_curve(d(2025, 7, 1), &curve).unwrap();

    assert_approx_eq(
        ratio_from_market,
        ratio_from_curve,
        EPSILON,
        "market routing to curve",
    );
}

#[test]
fn test_index_ratio_consistency_between_sources() {
    // Arrange: use a date whose 3-month lagged reference falls within both sources
    let ilb = sample_tips();
    let (_ctx_index, index) = market_context_with_index();
    let (_ctx_curve, curve) = market_context_with_curve();

    let ratio_index = ilb.index_ratio(d(2025, 7, 1), &index).unwrap();
    let ratio_curve = ilb.index_ratio_from_curve(d(2025, 7, 1), &curve).unwrap();

    // Assert - should be consistent (within tolerance due to different representations)
    // Index uses observations, curve uses forward projection - can differ significantly
    // due to different data sources and interpolation methods
    assert!(ratio_index > 0.0);
    assert!(ratio_curve > 0.0);
    // Both should be in reasonable range (0.8 to 1.5 for modest inflation)
    assert!(ratio_index > 0.8 && ratio_index < 1.5);
    assert!(ratio_curve > 0.8 && ratio_curve < 1.5);
}

#[test]
fn test_index_ratio_tips_requires_linear_interpolation() {
    // Arrange
    let ilb = sample_tips();
    let observations = vec![(d(2024, 12, 1), 300.0)];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Step); // Wrong for TIPS

    // Act & Assert - should fail validation
    let result = ilb.index_ratio(d(2025, 1, 1), &index);
    assert!(result.is_err());
}

#[test]
fn test_index_ratio_uk_requires_step_interpolation() {
    // Arrange
    let mut ilb = sample_uk_linker();
    ilb.indexation_method = IndexationMethod::UK;

    let observations = vec![(d(2024, 6, 1), 320.0)];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "UK-RPI",
        observations,
        finstack_core::currency::Currency::GBP,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear); // Wrong for UK

    // Act & Assert - should fail validation
    let result = ilb.index_ratio(d(2025, 1, 1), &index);
    assert!(result.is_err());
}

#[test]
fn test_index_ratio_canadian_requires_linear_interpolation() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.indexation_method = IndexationMethod::Canadian;

    let observations = vec![(d(2024, 9, 1), 140.0)];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "CA-CPI",
        observations,
        finstack_core::currency::Currency::CAD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Step); // Wrong for Canadian

    // Act & Assert - should fail validation
    let result = ilb.index_ratio(d(2025, 1, 1), &index);
    assert!(result.is_err());
}

#[test]
fn test_index_ratio_rejects_zero_base_index() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 0.0; // Invalid

    let (_, index) = market_context_with_index();

    // Act & Assert
    let result = ilb.index_ratio(d(2025, 1, 1), &index);
    assert!(result.is_err());
}

#[test]
fn test_index_ratio_rejects_negative_base_index() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = -100.0; // Invalid

    let (_, index) = market_context_with_index();

    // Act & Assert
    let result = ilb.index_ratio(d(2025, 1, 1), &index);
    assert!(result.is_err());
}

#[test]
fn test_index_ratio_extreme_inflation() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 100.0;

    // Create extreme inflation scenario index
    let observations = vec![
        (d(2024, 10, 1), 500.0), // 400% inflation
        (d(2025, 1, 1), 500.0),
    ];
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    // Act
    let ratio = ilb.index_ratio(d(2025, 1, 1), &index).unwrap();

    // Assert
    assert_approx_eq(ratio, 5.0, REL_TOL, "extreme inflation");
}

#[test]
fn test_index_ratio_time_series() {
    // Arrange
    let mut ilb = sample_tips();
    ilb.base_index = 300.0;
    ilb.lag = InflationLag::Months(3);

    // Build a time series with steady 0.5% monthly inflation
    // Extend to 18 months (Jan 2024 to June 2025) to cover lagged query dates
    let mut observations = Vec::new();
    for i in 0..18 {
        let month_date = d(2024, 1, 1).add_months(i);
        let value = 300.0 * (1.005_f64).powi(i);
        observations.push((month_date, value));
    }
    let index = finstack_core::market_data::scalars::InflationIndex::new(
        "US-CPI-U",
        observations,
        finstack_core::currency::Currency::USD,
    )
    .unwrap()
    .with_interpolation(InflationInterpolation::Linear);

    // Act - calculate ratios over time
    // April 2025 with 3mo lag → Jan 2025 (i=12)
    // Sept 2025 with 3mo lag → June 2025 (i=17)
    let ratio_1m = ilb.index_ratio(d(2025, 4, 1), &index).unwrap();
    let ratio_6m = ilb.index_ratio(d(2025, 9, 1), &index).unwrap();

    // Assert - ratios should increase over time
    assert!(ratio_6m > ratio_1m);
    assert!(ratio_1m > 1.0);
}
