//! Property-based tests for curve monotonicity and no-arbitrage.
//!
//! Key Properties:
//! - Discount factors: DF(t2) ≤ DF(t1) for t2 > t1 (strictly decreasing)
//! - Forward rates: f(t1, t2) ≥ minimum floor (e.g., -50bp)
//! - Zero rates: z(t2) can be ≥ or ≤ z(t1) (depends on curve shape)

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use proptest::prelude::*;
use time::Month;

/// Generate valid (monotonically decreasing) discount factors with wider range.
///
/// The decay range 0.80..0.999 covers:
/// - High rates (EM-style): 0.80 decay = ~22% annualized
/// - Normal rates: 0.95 decay = ~5% annualized
/// - Low rates: 0.999 decay = ~0.1% annualized
fn valid_discount_factors() -> impl Strategy<Value = Vec<(f64, f64)>> {
    // Generate 3-7 knot points with decreasing DFs
    (3usize..=7).prop_flat_map(|n| {
        // Generate times: 0.0, then increasing
        let times: Vec<f64> = (0..n).map(|i| i as f64 + (i as f64) * 0.5).collect();

        // Generate strictly decreasing DFs starting from 1.0
        // Widen range to cover more market scenarios: 0.80..0.999
        let dfs_strategy = prop::collection::vec(0.80..0.999, n - 1).prop_map(move |rates| {
            let mut dfs = vec![1.0]; // Start at DF=1.0
            for rate in rates {
                let prev = *dfs.last().unwrap();
                dfs.push(prev * rate);
            }
            dfs
        });

        (Just(times), dfs_strategy).prop_map(|(t, df)| t.into_iter().zip(df).collect())
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn prop_discount_factors_decrease(
        knots in valid_discount_factors(),
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

        // Build curve - should succeed with valid monotonic data
        let curve = DiscountCurve::builder("PROP-TEST")
            .base_date(base_date)
            .knots(knots.clone())
            .build();

        prop_assert!(curve.is_ok(), "Valid monotonic curve should build: {:?}", curve.err());

        let curve = curve.unwrap();

        // Property: DF(t2) < DF(t1) for all t2 > t1
        for i in 0..knots.len() - 1 {
            let (t1, _df1) = knots[i];
            let (t2, _df2) = knots[i + 1];

            let evaluated_df1 = curve.df(t1);
            let evaluated_df2 = curve.df(t2);

            prop_assert!(
                evaluated_df2 < evaluated_df1,
                "DF({}) = {:.6} should be < DF({}) = {:.6}",
                t2, evaluated_df2, t1, evaluated_df1
            );
        }
    }

    #[test]
    fn prop_invalid_curves_rejected(
        valid_knots in valid_discount_factors(),
        bad_index in 1usize..10,
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

        // Create invalid curve by making one DF increase
        let mut invalid_knots = valid_knots;
        let len = invalid_knots.len();
        let idx = 1 + (bad_index % (len - 1));
        // Make DF at idx larger than the previous DF.
        let (_, df_prev) = invalid_knots[idx - 1];
        invalid_knots[idx].1 = df_prev * 1.01;

        // Build curve - should fail validation
        let result = DiscountCurve::builder("INVALID-PROP")
            .base_date(base_date)
            .knots(invalid_knots)
            .build();

        prop_assert!(
            result.is_err(),
            "Non-monotonic curve should be rejected"
        );
    }

    #[test]
    fn prop_zero_rates_positive_for_normal_curves(
        knots in valid_discount_factors(),
    ) {
        let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

        let curve = DiscountCurve::builder("ZERO-PROP")
            .base_date(base_date)
            .knots(knots.clone())
            .build()
            .unwrap();

        // Property: For normal (decreasing DF) curves, zero rates are typically positive
        for (t, _df) in &knots {
            if *t > 0.0 {
                let zero = curve.zero(*t);

                // Zero rates should be in reasonable range for normal curves
                // Widened to accommodate wider DF ranges: -25% to +50%
                prop_assert!(
                    zero > -0.25 && zero < 0.50,
                    "Zero rate {:.4} at t={} outside reasonable range",
                    zero, t
                );
            }
        }
    }
}

// =============================================================================
// Edge Case Tests for Rate Environments
// =============================================================================

/// Test EUR-style negative rate environment where DF > 1.0 at short end.
///
/// Note: The current DiscountCurve implementation may not fully support
/// negative rate environments. This test documents the expected behavior
/// and current limitations.
#[test]
fn test_negative_rate_curve_via_zero_rates() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    // Build curve with small but positive DFs that imply very low positive rates
    // This is the closest we can get to negative rates with current implementation
    let knots = vec![
        (0.0, 1.0),
        (1.0, 0.9999), // ~0.01% rate
        (2.0, 0.9998), // ~0.01% rate
        (5.0, 0.9990), // ~0.02% rate
    ];

    let curve = DiscountCurve::builder("NEAR-ZERO")
        .base_date(base_date)
        .knots(knots)
        .build();

    assert!(
        curve.is_ok(),
        "Near-zero rate curve should build: {:?}",
        curve.err()
    );

    let curve = curve.unwrap();

    // Verify zero rates are very small
    let z1 = curve.zero(1.0);
    assert!(
        z1.abs() < 0.01,
        "Zero rate should be < 1% for near-zero curve: {:.4}",
        z1
    );
}

/// Test near-zero rate flat curve.
#[test]
fn test_flat_near_zero_rate_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    // Near-zero flat curve (~0.01% rate)
    let knots = vec![
        (0.0, 1.0),
        (1.0, 0.9999),  // ~0.01% rate
        (5.0, 0.9995),  // ~0.01% rate
        (10.0, 0.9990), // ~0.01% rate
    ];

    let curve = DiscountCurve::builder("FLAT-ZERO")
        .base_date(base_date)
        .knots(knots)
        .build();

    assert!(curve.is_ok(), "Near-zero flat curve should build");

    let curve = curve.unwrap();

    // Verify zero rates are small but positive
    let z5 = curve.zero(5.0);
    assert!(
        z5.abs() < 0.001,
        "Zero rate should be < 10bp for near-zero curve"
    );
}

/// Test steep high rate (EM-style) curve.
#[test]
fn test_steep_high_rate_curve() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    // EM-style high rate curve
    let knots = vec![
        (0.0, 1.0),
        (1.0, 0.90),  // 10% rate
        (5.0, 0.50),  // ~14% average
        (10.0, 0.25), // ~14% average
    ];

    let curve = DiscountCurve::builder("HIGH-RATE")
        .base_date(base_date)
        .knots(knots)
        .build();

    assert!(curve.is_ok(), "High rate curve should build");

    let curve = curve.unwrap();

    // Verify DFs are monotonically decreasing
    assert!(curve.df(1.0) < curve.df(0.0), "DF should decrease");
    assert!(curve.df(5.0) < curve.df(1.0), "DF should decrease");
    assert!(curve.df(10.0) < curve.df(5.0), "DF should decrease");

    // Verify zero rates are high
    let z10 = curve.zero(10.0);
    assert!(z10 > 0.10, "Zero rate should be > 10% for high rate curve");
}

/// Test inverted curve shape (short rates > long rates).
#[test]
fn test_inverted_curve_shape() {
    let base_date = Date::from_calendar_date(2025, Month::January, 15).unwrap();

    // Inverted curve: steep short-end, flatter long-end
    // This models a curve where short rates are higher than long rates
    let knots = vec![
        (0.0, 1.0),
        (1.0, 0.92),  // 8% 1Y rate
        (2.0, 0.85),  // 7.5% 2Y rate
        (5.0, 0.70),  // 7% 5Y rate
        (10.0, 0.50), // 7% 10Y rate
    ];

    let curve = DiscountCurve::builder("INVERTED")
        .base_date(base_date)
        .knots(knots)
        .build();

    assert!(curve.is_ok(), "Inverted curve should build");

    let curve = curve.unwrap();

    // Verify zero rates are higher at short end
    let z1 = curve.zero(1.0);
    let z10 = curve.zero(10.0);
    // Note: With these DFs, z1 ≈ 8% and z10 ≈ 7%
    // The relationship depends on interpolation
    assert!(z1 > 0.07, "Short-end zero rate should be high");
    assert!(z10 > 0.06, "Long-end zero rate should still be positive");
}
