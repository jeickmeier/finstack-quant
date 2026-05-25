//! Public-API invariants for scalar `PriceCurve` and `VolatilityIndexCurve`.
//!
//! These tests previously lived in `finstack-analytics/tests/api_invariants.rs`
//! but they only cover `finstack_core` curve types, so they now belong here
//! as `finstack-core` integration coverage.

use finstack_core::dates::{Date, Duration, Month};
use finstack_core::market_data::term_structures::{PriceCurve, VolatilityIndexCurve};

const TOL: f64 = 1e-12;

mod scalar_curve_invariants {
    use super::{Date, Duration, Month, PriceCurve, VolatilityIndexCurve, TOL};
    use finstack_core::math::interp::InterpStyle;

    fn base_date() -> Date {
        Date::from_calendar_date(2025, Month::January, 1).unwrap()
    }

    fn price_curve() -> PriceCurve {
        PriceCurve::builder("WTI")
            .base_date(base_date())
            .spot_price(80.0)
            .knots([
                (0.0, 80.0),
                (0.25, 81.0),
                (0.5, 82.0),
                (1.0, 83.0),
                (2.0, 84.0),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap()
    }

    fn vol_curve() -> VolatilityIndexCurve {
        VolatilityIndexCurve::builder("VIX")
            .base_date(base_date())
            .spot_level(20.0)
            .knots([
                (0.0, 20.0),
                (0.25, 21.0),
                (0.5, 22.0),
                (1.0, 23.0),
                (2.0, 24.0),
            ])
            .interp(InterpStyle::Linear)
            .build()
            .unwrap()
    }

    #[test]
    fn parallel_bump_shifts_all_knots_by_bump_amount() {
        let pc = price_curve();
        let vc = vol_curve();
        let bump = 2.5;

        let bumped_pc = pc.with_parallel_bump(bump).unwrap();
        let bumped_vc = vc.with_parallel_bump(bump).unwrap();

        assert!((bumped_pc.spot_price() - (pc.spot_price() + bump)).abs() < TOL);
        assert!((bumped_vc.spot_level() - (vc.spot_level() + bump)).abs() < TOL);

        for (orig, bumped) in pc.prices().iter().zip(bumped_pc.prices().iter()) {
            assert!((bumped - orig - bump).abs() < TOL);
        }
        for (orig, bumped) in vc.levels().iter().zip(bumped_vc.levels().iter()) {
            assert!((bumped - orig - bump).abs() < TOL);
        }
    }

    #[test]
    fn parallel_bump_zero_is_identity() {
        let pc = price_curve();
        let vc = vol_curve();

        let bumped_pc = pc.with_parallel_bump(0.0).unwrap();
        let bumped_vc = vc.with_parallel_bump(0.0).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0] {
            assert!((bumped_pc.price(t) - pc.price(t)).abs() < TOL);
            assert!((bumped_vc.forward_level(t) - vc.forward_level(t)).abs() < TOL);
        }
    }

    #[test]
    fn parallel_bumps_are_additive() {
        let pc = price_curve();
        let vc = vol_curve();

        let double_bump_pc = pc
            .with_parallel_bump(1.0)
            .unwrap()
            .with_parallel_bump(2.0)
            .unwrap();
        let single_bump_pc = pc.with_parallel_bump(3.0).unwrap();
        let double_bump_vc = vc
            .with_parallel_bump(1.0)
            .unwrap()
            .with_parallel_bump(2.0)
            .unwrap();
        let single_bump_vc = vc.with_parallel_bump(3.0).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0] {
            assert!((double_bump_pc.price(t) - single_bump_pc.price(t)).abs() < TOL);
            assert!(
                (double_bump_vc.forward_level(t) - single_bump_vc.forward_level(t)).abs() < TOL
            );
        }
    }

    #[test]
    fn triangular_key_rate_bump_does_not_shift_spot() {
        let pc = price_curve();
        let vc = vol_curve();

        let bumped_pc = pc
            .with_triangular_key_rate_bump_neighbors(Some(0.25), 0.5, Some(1.0), 3.0)
            .unwrap();
        let bumped_vc = vc
            .with_triangular_key_rate_bump_neighbors(Some(0.25), 0.5, Some(1.0), 3.0)
            .unwrap();

        assert!((bumped_pc.spot_price() - pc.spot_price()).abs() < TOL);
        assert!((bumped_vc.spot_level() - vc.spot_level()).abs() < TOL);
    }

    #[test]
    fn triangular_key_rate_bump_full_at_peak() {
        let pc = price_curve();
        let vc = vol_curve();

        let bumped_pc = pc
            .with_triangular_key_rate_bump_neighbors(Some(0.25), 0.5, Some(1.0), 3.0)
            .unwrap();
        let bumped_vc = vc
            .with_triangular_key_rate_bump_neighbors(Some(0.25), 0.5, Some(1.0), 3.0)
            .unwrap();

        let idx_pc = pc
            .knots()
            .iter()
            .position(|&t| (t - 0.5).abs() < 1e-9)
            .unwrap();
        let idx_vc = vc
            .knots()
            .iter()
            .position(|&t| (t - 0.5).abs() < 1e-9)
            .unwrap();

        assert!((bumped_pc.prices()[idx_pc] - pc.prices()[idx_pc] - 3.0).abs() < TOL);
        assert!((bumped_vc.levels()[idx_vc] - vc.levels()[idx_vc] - 3.0).abs() < TOL);
    }

    #[test]
    fn roll_forward_advances_base_date_and_new_spot() {
        let pc = price_curve();
        let vc = vol_curve();
        let days = 90_i64;

        let rolled_pc = pc.roll_forward(days).unwrap();
        let rolled_vc = vc.roll_forward(days).unwrap();

        let expected_base = base_date() + Duration::days(days);
        assert_eq!(rolled_pc.base_date(), expected_base);
        assert_eq!(rolled_vc.base_date(), expected_base);

        let dt_years = {
            use finstack_core::dates::{DayCount, DayCountContext};
            DayCount::Act365F
                .year_fraction(base_date(), expected_base, DayCountContext::default())
                .unwrap()
        };

        assert!((rolled_pc.spot_price() - pc.price(dt_years)).abs() < TOL);
        assert!((rolled_vc.spot_level() - vc.forward_level(dt_years)).abs() < TOL);
    }

    #[test]
    fn roll_forward_preserves_far_tenor_levels() {
        let rolled_pc = price_curve().roll_forward(30).unwrap();
        let rolled_vc = vol_curve().roll_forward(30).unwrap();

        assert!(rolled_pc.price(1.0) > 0.0);
        assert!(rolled_vc.forward_level(1.0) > 0.0);
    }

    #[test]
    fn price_curve_serde_roundtrip() {
        let pc = price_curve();
        let restored: PriceCurve =
            serde_json::from_str(&serde_json::to_string(&pc).unwrap()).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0, 2.0] {
            assert!((pc.price(t) - restored.price(t)).abs() < TOL);
        }
    }

    #[test]
    fn vol_curve_serde_roundtrip() {
        let vc = vol_curve();
        let restored: VolatilityIndexCurve =
            serde_json::from_str(&serde_json::to_string(&vc).unwrap()).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0, 2.0] {
            assert!((vc.forward_level(t) - restored.forward_level(t)).abs() < TOL);
        }
    }

    #[test]
    fn spot_inference_consistency_across_curve_types() {
        let pc_inferred = PriceCurve::builder("WTI-INF")
            .base_date(base_date())
            .knots([(0.0, 75.0), (0.25, 76.0), (0.5, 77.0), (1.0, 78.0)])
            .build()
            .unwrap();
        let vc_inferred = VolatilityIndexCurve::builder("VIX-INF")
            .base_date(base_date())
            .knots([(0.0, 18.0), (0.25, 19.0), (0.5, 20.0), (1.0, 21.0)])
            .build()
            .unwrap();

        assert!((pc_inferred.spot_price() - 75.0).abs() < TOL);
        assert!((vc_inferred.spot_level() - 18.0).abs() < TOL);
        assert!((pc_inferred.price(0.0) - pc_inferred.spot_price()).abs() < TOL);
        assert!((vc_inferred.forward_level(0.0) - vc_inferred.spot_level()).abs() < TOL);
    }
}
