//! Tests for InflationCurve functionality.
//!
//! This module covers:
//! - Builder validation and construction
//! - CPI interpolation
//! - Inflation rate calculations
//! - Clone safety (panic-free)
//! - Serialization roundtrips

use finstack_core::market_data::term_structures::InflationCurve;
use finstack_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use time::{Date, Month};

fn _test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

// =============================================================================
// Clone Safety Tests
// =============================================================================

/// Verifies that cloning an InflationCurve is infallible and produces identical results.
#[test]
fn clone_is_panic_free_and_equivalent() {
    let original = InflationCurve::builder("US-CPI")
        .base_cpi(300.0)
        .base_date(_test_date())
        .knots([
            (0.0, 300.0),
            (1.0, 306.0),
            (2.0, 312.5),
            (5.0, 330.0),
            (10.0, 360.0),
        ])
        .interp(InterpStyle::Linear)
        .build()
        .unwrap();

    // Clone should not panic
    let cloned = original.clone();

    // Verify structural equality
    assert_eq!(original.id(), cloned.id());
    assert_eq!(original.base_cpi(), cloned.base_cpi());
    assert_eq!(original.knots(), cloned.knots());
    assert_eq!(original.cpi_levels(), cloned.cpi_levels());

    // Verify interpolation produces identical results
    for t in [0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0] {
        let orig_cpi = original.cpi(t);
        let cloned_cpi = cloned.cpi(t);
        assert!(
            (orig_cpi - cloned_cpi).abs() < 1e-14,
            "CPI mismatch after clone at t={}: {} vs {}",
            t,
            orig_cpi,
            cloned_cpi
        );
    }
}

/// Verifies clone works correctly for all interpolation styles.
#[test]
fn clone_works_for_all_interp_styles() {
    let interp_styles = [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::CubicHermite,
    ];

    for style in interp_styles {
        let curve = InflationCurve::builder("TEST-CPI")
            .base_cpi(100.0)
            .base_date(_test_date())
            .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0)])
            .interp(style)
            .build()
            .unwrap();

        // Clone should not panic for any interpolation style
        let cloned = curve.clone();

        // Verify CPI values match
        for t in [0.0, 0.5, 1.0, 3.0, 5.0] {
            let orig_cpi = curve.cpi(t);
            let cloned_cpi = cloned.cpi(t);
            assert!(
                (orig_cpi - cloned_cpi).abs() < 1e-14,
                "Clone CPI mismatch for {:?} at t={}: {} vs {}",
                style,
                t,
                orig_cpi,
                cloned_cpi
            );
        }
    }
}

/// Verifies clone works correctly with extrapolated values.
///
/// Note: InflationCurveBuilder uses default extrapolation policy.
/// This test verifies cloning preserves extrapolation behavior.
#[test]
fn clone_works_with_extrapolation() {
    let curve = InflationCurve::builder("TEST-CPI")
        .base_cpi(100.0)
        .base_date(_test_date())
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0)])
        .build()
        .unwrap();

    // Clone should not panic
    let cloned = curve.clone();

    // Verify extrapolated CPI values match beyond the knot range
    for t in [0.0, 2.5, 5.0, 10.0, 20.0] {
        let orig_cpi = curve.cpi(t);
        let cloned_cpi = cloned.cpi(t);
        assert!(
            (orig_cpi - cloned_cpi).abs() < 1e-14,
            "Clone CPI mismatch at t={}: {} vs {}",
            t,
            orig_cpi,
            cloned_cpi
        );
    }
}

#[test]
fn builder_requires_explicit_base_date() {
    let result = InflationCurve::builder("US-CPI")
        .base_cpi(100.0)
        .knots([(0.0, 100.0), (1.0, 102.0)])
        .build();
    assert!(
        result.is_err(),
        "inflation curve should require an explicit base date"
    );
}

// =============================================================================
// Serialization Tests
// =============================================================================

mod serde_tests {
    use super::*;

    #[test]
    fn roundtrip_with_all_fields() {
        let original = InflationCurve::builder("US-CPI")
            .base_cpi(300.0)
            .base_date(_test_date())
            .knots([
                (0.0, 300.0),
                (1.0, 306.0),
                (2.0, 312.5),
                (5.0, 330.0),
                (10.0, 360.0),
            ])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: InflationCurve = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id(), deserialized.id());
        assert_eq!(original.base_cpi(), deserialized.base_cpi());
        assert_eq!(original.knots(), deserialized.knots());
        assert_eq!(original.cpi_levels(), deserialized.cpi_levels());
        assert_eq!(original.extrapolation(), deserialized.extrapolation());

        // Test CPI interpolation
        for t in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.5, 10.0] {
            let original_cpi = original.cpi(t);
            let deserialized_cpi = deserialized.cpi(t);
            assert!(
                (original_cpi - deserialized_cpi).abs() < 1e-10,
                "CPI mismatch at t={}: {} vs {}",
                t,
                original_cpi,
                deserialized_cpi
            );
        }

        // Test inflation rate calculation
        for (t1, t2) in [(0.0, 1.0), (1.0, 2.0), (2.0, 5.0), (5.0, 10.0)] {
            let original_rate = original.inflation_rate(t1, t2);
            let deserialized_rate = deserialized.inflation_rate(t1, t2);
            assert!(
                (original_rate - deserialized_rate).abs() < 1e-12,
                "Inflation rate mismatch for period {}-{}: {} vs {}",
                t1,
                t2,
                original_rate,
                deserialized_rate
            );
        }
    }

    #[test]
    fn builder_extrapolation_controls_cpi_beyond_last_knot() {
        let flat_zero = InflationCurve::builder("US-CPI")
            .base_cpi(300.0)
            .base_date(_test_date())
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatZero)
            .build()
            .unwrap();
        let flat_forward = InflationCurve::builder("US-CPI")
            .base_cpi(300.0)
            .base_date(_test_date())
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::FlatForward)
            .build()
            .unwrap();

        assert_eq!(flat_zero.extrapolation(), ExtrapolationPolicy::FlatZero);
        assert_eq!(
            flat_forward.extrapolation(),
            ExtrapolationPolicy::FlatForward
        );
        assert!(
            flat_forward.cpi(2.0) > flat_zero.cpi(2.0),
            "flat-forward CPI should continue the positive inflation trend"
        );
    }

    #[test]
    fn roundtrip_all_interp_styles() {
        // Note: MonotoneConvex interprets values as discount factors (Hagan-West
        // forward-rate construction), so it's not suitable for CPI levels
        let interp_styles = [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::CubicHermite,
        ];

        for style in interp_styles {
            let original = InflationCurve::builder("EUR-HICP")
                .base_cpi(100.0)
                .base_date(_test_date())
                .knots([(0.0, 100.0), (1.0, 102.0), (3.0, 106.5), (5.0, 111.0)])
                .interp(style)
                .build()
                .unwrap();

            let json = serde_json::to_string(&original).unwrap();
            let deserialized: InflationCurve = serde_json::from_str(&json).unwrap();

            // Test CPI accuracy for each style
            for t in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
                let original_cpi = original.cpi(t);
                let deserialized_cpi = deserialized.cpi(t);
                assert!(
                    (original_cpi - deserialized_cpi).abs() < 1e-10,
                    "CPI mismatch for {:?} at t={}: {} vs {}",
                    style,
                    t,
                    original_cpi,
                    deserialized_cpi
                );
            }
        }
    }
}
