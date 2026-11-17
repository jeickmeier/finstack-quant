//! Tests for InflationCurve functionality.
//!
//! This module covers:
//! - Builder validation and strict required conventions
//! - CPI interpolation & date queries
//! - Inflation rate calculations
//! - Clone safety (panic-free)
//! - Serialization roundtrips

use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::term_structures::InflationCurve;
use finstack_quant_core::math::interp::{ExtrapolationPolicy, InterpStyle};
use time::{Date, Month};

fn _test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

/// Verifies that cloning an InflationCurve is infallible and produces identical results.
#[test]
fn clone_is_panic_free_and_equivalent() {
    let original = InflationCurve::builder("US-CPI")
        .base_date(_test_date())
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 300.0),
            (1.0, 306.0),
            (2.0, 312.5),
            (5.0, 330.0),
            (10.0, 360.0),
        ])
        .interp(InterpStyle::Linear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    let cloned = original.clone();

    assert_eq!(original.id(), cloned.id());
    assert_eq!(original.base_cpi(), cloned.base_cpi());
    assert_eq!(original.knots(), cloned.knots());
    assert_eq!(original.cpi_levels(), cloned.cpi_levels());

    for t in [0.0, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0] {
        let orig_cpi = original.cpi(t).unwrap();
        let cloned_cpi = cloned.cpi(t).unwrap();
        assert!(
            (orig_cpi - cloned_cpi).abs() < 1e-14,
            "CPI mismatch after clone at t={}: {} vs {}",
            t,
            orig_cpi,
            cloned_cpi
        );
    }
}

/// Verifies clone works correctly for all supported interpolation styles.
#[test]
fn clone_works_for_all_interp_styles() {
    let interp_styles = [
        InterpStyle::Linear,
        InterpStyle::LogLinear,
        InterpStyle::CubicHermite,
    ];

    for style in interp_styles {
        let curve = InflationCurve::builder("TEST-CPI")
            .base_date(_test_date())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0)])
            .interp(style)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();

        let cloned = curve.clone();

        for t in [0.0, 0.5, 1.0, 3.0, 5.0] {
            let orig_cpi = curve.cpi(t).unwrap();
            let cloned_cpi = cloned.cpi(t).unwrap();
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

/// Verifies clone preserves extrapolation behavior and Bus/252 calendar clock.
#[test]
fn clone_works_with_extrapolation_and_bus252_calendar() {
    let curve = InflationCurve::builder("BR-IPCA")
        .base_date(_test_date())
        .day_count(DayCount::Bus252)
        .calendar_id("brbd")
        .knots([(0.0, 100.0), (1.0, 102.0), (5.0, 110.0)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Extend)
        .build()
        .unwrap();

    assert_eq!(curve.calendar_id(), Some("brbd"));
    let cloned = curve.clone();
    assert_eq!(cloned.calendar_id(), Some("brbd"));

    for t in [0.0, 2.5, 5.0, 10.0, 20.0] {
        let orig_cpi = curve.cpi(t).unwrap();
        let cloned_cpi = cloned.cpi(t).unwrap();
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
fn builder_requires_explicit_conventions() {
    let missing_base = InflationCurve::builder("US-CPI")
        .day_count(DayCount::Act365F)
        .knots([(0.0, 100.0), (1.0, 102.0)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .build();
    assert!(missing_base.is_err());

    let missing_dc = InflationCurve::builder("US-CPI")
        .base_date(_test_date())
        .knots([(0.0, 100.0), (1.0, 102.0)])
        .interp(InterpStyle::LogLinear)
        .extrapolation(ExtrapolationPolicy::Flat)
        .build();
    assert!(missing_dc.is_err());

    let missing_interp = InflationCurve::builder("US-CPI")
        .base_date(_test_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 100.0), (1.0, 102.0)])
        .extrapolation(ExtrapolationPolicy::Flat)
        .build();
    assert!(missing_interp.is_err());

    let missing_extrap = InflationCurve::builder("US-CPI")
        .base_date(_test_date())
        .day_count(DayCount::Act365F)
        .knots([(0.0, 100.0), (1.0, 102.0)])
        .interp(InterpStyle::LogLinear)
        .build();
    assert!(missing_extrap.is_err());
}

mod serde_tests {
    use super::*;

    #[test]
    fn roundtrip_with_all_fields() {
        let original = InflationCurve::builder("US-CPI")
            .base_date(_test_date())
            .day_count(DayCount::Act365F)
            .knots([
                (0.0, 300.0),
                (1.0, 306.0),
                (2.0, 312.5),
                (5.0, 330.0),
                (10.0, 360.0),
            ])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: InflationCurve = serde_json::from_str(&json).unwrap();

        assert_eq!(original.id(), deserialized.id());
        assert_eq!(original.base_cpi(), deserialized.base_cpi());
        assert_eq!(original.knots(), deserialized.knots());
        assert_eq!(original.cpi_levels(), deserialized.cpi_levels());
        assert_eq!(original.extrapolation(), deserialized.extrapolation());

        for t in [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 5.0, 7.5, 10.0] {
            let original_cpi = original.cpi(t).unwrap();
            let deserialized_cpi = deserialized.cpi(t).unwrap();
            assert!(
                (original_cpi - deserialized_cpi).abs() < 1e-10,
                "CPI mismatch at t={}: {} vs {}",
                t,
                original_cpi,
                deserialized_cpi
            );
        }

        for (t1, t2) in [(0.0, 1.0), (1.0, 2.0), (2.0, 5.0), (5.0, 10.0)] {
            let original_rate = original.inflation_rate(t1, t2).unwrap();
            let deserialized_rate = deserialized.inflation_rate(t1, t2).unwrap();
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
        let flat = InflationCurve::builder("US-CPI")
            .base_date(_test_date())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Flat)
            .build()
            .unwrap();
        let extend = InflationCurve::builder("US-CPI")
            .base_date(_test_date())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Extend)
            .build()
            .unwrap();
        let err_tail = InflationCurve::builder("US-CPI")
            .base_date(_test_date())
            .day_count(DayCount::Act365F)
            .knots([(0.0, 300.0), (1.0, 306.0)])
            .interp(InterpStyle::LogLinear)
            .extrapolation(ExtrapolationPolicy::Error)
            .build()
            .unwrap();

        assert_eq!(flat.extrapolation(), ExtrapolationPolicy::Flat);
        assert_eq!(extend.extrapolation(), ExtrapolationPolicy::Extend);
        assert!(extend.cpi(2.0).unwrap() > flat.cpi(2.0).unwrap());
        assert!(err_tail.cpi(2.0).is_err());
    }

    #[test]
    fn roundtrip_all_interp_styles() {
        let interp_styles = [
            InterpStyle::Linear,
            InterpStyle::LogLinear,
            InterpStyle::CubicHermite,
        ];

        for style in interp_styles {
            let original = InflationCurve::builder("EUR-HICP")
                .base_date(_test_date())
                .day_count(DayCount::Act365F)
                .knots([(0.0, 100.0), (1.0, 102.0), (3.0, 106.5), (5.0, 111.0)])
                .interp(style)
                .extrapolation(ExtrapolationPolicy::Extend)
                .build()
                .unwrap();

            let json = serde_json::to_string(&original).unwrap();
            let deserialized: InflationCurve = serde_json::from_str(&json).unwrap();

            for t in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0] {
                let original_cpi = original.cpi(t).unwrap();
                let deserialized_cpi = deserialized.cpi(t).unwrap();
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
