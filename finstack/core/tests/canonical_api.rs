//! Tests for canonical API behavior.
//!
//! These tests verify consistent results and proper error handling
//! for core financial computation APIs.
//!
//! # Test Structure
//!
//! Tests are organized by module:
//! - **NPV**: Tests for `npv()` with various discount curves
//! - **IRR**: Tests for `irr()` and `irr_with_daycount()`
//! - **Quadrature**: Tests for `GaussHermiteQuadrature::new()`

use finstack_core::cashflow::{irr, npv, npv_with_options, xirr_with_daycount, NpvOptions};
use finstack_core::currency::Currency;
use finstack_core::dates::DayCountContext;
use finstack_core::dates::{Date, DayCount};
use finstack_core::market_data::term_structures::FlatCurve;
use finstack_core::math::GaussHermiteQuadrature;
use finstack_core::money::Money;
use time::Month;

/// Tolerance for floating-point comparisons.
const TOLERANCE: f64 = 1e-12;

/// Tolerance for IRR comparisons (solver tolerance).
const IRR_TOLERANCE: f64 = 1e-6;

/// Helper to create dates.
fn d(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, Month::try_from(month).unwrap(), day).unwrap()
}

// =============================================================================
// NPV Tests (Canonical API)
// =============================================================================

mod npv_tests {
    use super::*;

    /// Test NPV with FlatCurve (the canonical approach).
    #[test]
    fn npv_with_flat_curve() {
        let base = d(2024, 1, 1);
        let flows = vec![
            (base, Money::new(-100_000.0, Currency::USD)),
            (d(2025, 1, 1), Money::new(110_000.0, Currency::USD)),
        ];
        let rate: f64 = 0.05;
        let dc = DayCount::Act365F;

        // Convert annual rate to continuous rate
        let continuous_rate = (1.0 + rate).ln();
        let curve = FlatCurve::new(continuous_rate, base, dc, "TEST");
        // Investment-NPV view: the day-0 outlay is intentional, so opt back in
        // (the default now excludes flows on/before the valuation date — see
        // ).
        let pv = npv_with_options(
            &curve,
            base,
            Some(dc),
            DayCountContext::default(),
            NpvOptions::default().include_past_flows(true),
            &flows,
        )
        .unwrap();

        // NPV should be approximately 110000/1.05 - 100000 ≈ 4761.90
        assert!(
            pv.amount() > 4700.0 && pv.amount() < 4800.0,
            "NPV at 5% should be ~4761.90, got {}",
            pv.amount()
        );
    }

    /// Test NPV across various discount rates.
    #[test]
    fn npv_various_rates() {
        let base = d(2024, 1, 1);
        let flows = vec![
            (base, Money::new(-50_000.0, Currency::USD)),
            (d(2024, 7, 1), Money::new(25_000.0, Currency::USD)),
            (d(2025, 1, 1), Money::new(30_000.0, Currency::USD)),
        ];

        for rate in [0.0_f64, 0.01, 0.05, 0.10, 0.25] {
            let dc = DayCount::Act365F;
            let continuous_rate = (1.0 + rate).ln();
            let curve = FlatCurve::new(continuous_rate, base, dc, "TEST");
            // Investment-NPV view: include the day-0 outlay explicitly (the
            // default excludes flows on/before the valuation date).
            let pv = npv_with_options(
                &curve,
                base,
                Some(dc),
                DayCountContext::default(),
                NpvOptions::default().include_past_flows(true),
                &flows,
            )
            .unwrap();

            // At 0% rate, NPV should be sum of flows = 5000
            if rate == 0.0 {
                assert!(
                    (pv.amount() - 5000.0).abs() < TOLERANCE,
                    "At 0% rate, NPV should be 5000, got {}",
                    pv.amount()
                );
            }
        }
    }

    /// Test NPV with different day count conventions.
    #[test]
    fn npv_different_day_counts() {
        let base = d(2024, 1, 1);
        let flows = vec![
            (base, Money::new(-100.0, Currency::USD)),
            (d(2024, 7, 1), Money::new(105.0, Currency::USD)),
        ];
        let rate: f64 = 0.05;

        let mut results = Vec::new();
        for dc in [DayCount::Act365F, DayCount::Act360, DayCount::Thirty360] {
            let continuous_rate = (1.0 + rate).ln();
            let curve = FlatCurve::new(continuous_rate, base, dc, "TEST");
            let pv = npv(&curve, base, Some(dc), &flows).unwrap();
            results.push((dc, pv.amount()));
        }

        // Different day counts should produce different results
        assert!(
            (results[0].1 - results[1].1).abs() > 1e-4,
            "Act365F and Act360 should produce different NPVs"
        );
    }
}

// =============================================================================
// IRR Tests (Canonical API)
// =============================================================================

mod irr_tests {
    use super::*;

    /// Test that periodic IRR works correctly for evenly-spaced flows.
    #[test]
    fn periodic_irr_is_canonical() {
        let flows = [-100.0, 10.0, 10.0, 10.0, 110.0];

        let irr1 = irr(&flows, None).unwrap();
        let irr2 = irr(&flows, Some(0.05)).unwrap();

        // Different initial guesses should converge to the same rate
        assert!(
            (irr1 - irr2).abs() < IRR_TOLERANCE,
            "Periodic IRR should be guess-independent: irr(None)={}, irr(Some(0.05))={}",
            irr1,
            irr2
        );
    }

    /// Test IRR with various cashflow patterns.
    #[test]
    fn dated_irr_various_patterns() {
        let test_cases = vec![
            // Simple: invest, receive
            vec![(d(2024, 1, 1), -100_000.0), (d(2025, 1, 1), 110_000.0)],
            // Multiple distributions
            vec![
                (d(2024, 1, 1), -100_000.0),
                (d(2024, 6, 1), 30_000.0),
                (d(2025, 1, 1), 80_000.0),
            ],
            // High return
            vec![(d(2024, 1, 1), -50_000.0), (d(2025, 1, 1), 150_000.0)],
            // Low return
            vec![(d(2024, 1, 1), -100_000.0), (d(2025, 1, 1), 101_000.0)],
        ];

        for flows in test_cases {
            let rate = xirr_with_daycount(&flows, DayCount::Act365F, None).unwrap();
            // All test cases have positive returns
            assert!(rate > 0.0, "IRR should be positive for these flows");
        }
    }

    /// Test that different day counts produce different IRR results.
    #[test]
    fn irr_day_count_affects_result() {
        let flows: Vec<(Date, f64)> = vec![(d(2024, 1, 1), -100_000.0), (d(2024, 7, 1), 102_500.0)];

        let irr_365f = xirr_with_daycount(&flows, DayCount::Act365F, None).unwrap();
        let irr_360 = xirr_with_daycount(&flows, DayCount::Act360, None).unwrap();

        // Different day counts should produce different rates
        assert!(
            (irr_365f - irr_360).abs() > 1e-4,
            "Different day counts should produce different IRRs"
        );

        // Both should be positive for this profitable trade
        assert!(irr_365f > 0.0);
        assert!(irr_360 > 0.0);
    }
}

// =============================================================================
// Gauss-Hermite Quadrature Tests (Canonical API)
// =============================================================================

mod quadrature_tests {
    use super::*;

    /// Test creating quadrature with all supported orders.
    #[test]
    fn new_supported_orders() {
        for order in [5, 7, 10, 15, 20] {
            let quad = GaussHermiteQuadrature::new(order).expect("valid order");
            assert_eq!(
                quad.points.len(),
                order,
                "Order {} should have {} points",
                order,
                order
            );
        }
    }

    /// Test integration correctness with known integrals.
    #[test]
    fn quadrature_integration_correctness() {
        // E[X²] = 1 for standard normal
        let expected_x2 = 1.0;
        // E[X⁴] = 3 for standard normal
        let expected_x4 = 3.0;

        for order in [5, 7, 10, 15, 20] {
            let quad = GaussHermiteQuadrature::new(order).expect("valid order");

            let result_x2 = quad.integrate(|x| x * x);
            let result_x4 = quad.integrate(|x| x.powi(4));

            // All supported orders are exact for x² (degree 2) and x⁴ (degree 4)
            // because n-point GH is exact for polynomials of degree ≤ 2n-1, and our
            // minimum supported order is 5 (exact up to degree 9).
            let tolerance_x2 = 1e-10;
            let tolerance_x4 = 1e-10;

            assert!(
                (result_x2 - expected_x2).abs() < tolerance_x2,
                "Order {}: E[X²] should be ~1.0, got {}",
                order,
                result_x2
            );
            assert!(
                (result_x4 - expected_x4).abs() < tolerance_x4,
                "Order {}: E[X⁴] should be ~3.0, got {}",
                order,
                result_x4
            );
        }
    }

    /// Test that serialization/deserialization preserves behavior.
    #[test]
    fn quadrature_serde_equivalence() {
        for order in [5, 7, 10, 15, 20] {
            let original = GaussHermiteQuadrature::new(order).expect("valid order");

            // Serialize and deserialize
            let json = serde_json::to_string(&original).unwrap();
            let restored: GaussHermiteQuadrature = serde_json::from_str(&json).unwrap();

            // Test with integration
            let f = |x: f64| x * x;
            let result_original = original.integrate(f);
            let result_restored = restored.integrate(f);

            assert!(
                (result_original - result_restored).abs() < 1e-12,
                "Serde roundtrip should preserve behavior for order {}: original={}, restored={}",
                order,
                result_original,
                result_restored
            );
        }
    }

    /// Test that `new()` returns `Err` for unsupported orders.
    #[test]
    fn new_rejects_unsupported_orders() {
        let unsupported = [
            0, 1, 2, 3, 4, 6, 8, 9, 11, 12, 13, 14, 16, 17, 18, 19, 21, 100,
        ];

        for order in unsupported {
            assert!(
                GaussHermiteQuadrature::new(order).is_err(),
                "Order {} should be unsupported",
                order
            );
        }
    }
}
