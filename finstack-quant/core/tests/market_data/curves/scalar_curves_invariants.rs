//! Public-API invariants for scalar `PriceCurve` (price and vol-index kinds).
//!
//! These tests previously lived in `finstack-quant-analytics/tests/api_invariants.rs`
//! but they only cover `finstack_quant_core` curve types, so they now belong here
//! as `finstack-quant-core` integration coverage.

use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::{PriceCurve, PriceCurveKind};
use time::{Duration, Month};

const TOL: f64 = 1e-12;

mod scalar_curve_invariants {
    use super::{Date, Duration, Month, PriceCurve, PriceCurveKind, TOL};
    use finstack_quant_core::market_data::bumps::{
        BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable,
    };

    /// Parallel price shock in native price units.
    fn par(value: f64) -> BumpSpec {
        BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value,
            bump_type: BumpType::Parallel,
        }
    }

    /// Triangular key-rate price shock in native price units.
    fn key_rate(
        prev_bucket: Option<f64>,
        target_bucket: f64,
        next_bucket: Option<f64>,
        value: f64,
    ) -> BumpSpec {
        BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value,
            bump_type: BumpType::TriangularKeyRate {
                prev_bucket,
                target_bucket,
                next_bucket,
            },
        }
    }
    use finstack_quant_core::math::interp::InterpStyle;

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

    fn vol_curve() -> PriceCurve {
        PriceCurve::builder("VIX")
            .kind(PriceCurveKind::VolIndex)
            .base_date(base_date())
            .spot_price(20.0)
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

        let bumped_pc = pc.apply_bump(par(bump)).unwrap();
        let bumped_vc = vc.apply_bump(par(bump)).unwrap();

        assert!((bumped_pc.spot_price() - (pc.spot_price() + bump)).abs() < TOL);
        assert!((bumped_vc.spot_price() - (vc.spot_price() + bump)).abs() < TOL);

        for (orig, bumped) in pc.prices().iter().zip(bumped_pc.prices().iter()) {
            assert!((bumped - orig - bump).abs() < TOL);
        }
        for (orig, bumped) in vc.prices().iter().zip(bumped_vc.prices().iter()) {
            assert!((bumped - orig - bump).abs() < TOL);
        }
    }

    #[test]
    fn parallel_bump_zero_is_identity() {
        let pc = price_curve();
        let vc = vol_curve();

        let bumped_pc = pc.apply_bump(par(0.0)).unwrap();
        let bumped_vc = vc.apply_bump(par(0.0)).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0] {
            assert!((bumped_pc.price(t).unwrap() - pc.price(t).unwrap()).abs() < TOL);
            assert!((bumped_vc.price(t).unwrap() - vc.price(t).unwrap()).abs() < TOL);
        }
    }

    #[test]
    fn parallel_bumps_are_additive() {
        let pc = price_curve();
        let vc = vol_curve();

        let double_bump_pc = pc
            .apply_bump(par(1.0))
            .unwrap()
            .apply_bump(par(2.0))
            .unwrap();
        let single_bump_pc = pc.apply_bump(par(3.0)).unwrap();
        let double_bump_vc = vc
            .apply_bump(par(1.0))
            .unwrap()
            .apply_bump(par(2.0))
            .unwrap();
        let single_bump_vc = vc.apply_bump(par(3.0)).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0] {
            assert!(
                (double_bump_pc.price(t).unwrap() - single_bump_pc.price(t).unwrap()).abs() < TOL
            );
            assert!(
                (double_bump_vc.price(t).unwrap() - single_bump_vc.price(t).unwrap()).abs() < TOL
            );
        }
    }

    #[test]
    fn triangular_key_rate_bump_does_not_shift_spot() {
        let pc = price_curve();
        let vc = vol_curve();

        let bumped_pc = pc
            .apply_bump(key_rate(Some(0.25), 0.5, Some(1.0), 3.0))
            .unwrap();
        let bumped_vc = vc
            .apply_bump(key_rate(Some(0.25), 0.5, Some(1.0), 3.0))
            .unwrap();

        assert!((bumped_pc.spot_price() - pc.spot_price()).abs() < TOL);
        assert!((bumped_vc.spot_price() - vc.spot_price()).abs() < TOL);
    }

    #[test]
    fn triangular_key_rate_bump_full_at_peak() {
        let pc = price_curve();
        let vc = vol_curve();

        let bumped_pc = pc
            .apply_bump(key_rate(Some(0.25), 0.5, Some(1.0), 3.0))
            .unwrap();
        let bumped_vc = vc
            .apply_bump(key_rate(Some(0.25), 0.5, Some(1.0), 3.0))
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
        assert!((bumped_vc.prices()[idx_vc] - vc.prices()[idx_vc] - 3.0).abs() < TOL);
    }

    #[test]
    fn context_condition_keeps_price_and_vol_curves_frozen() {
        use finstack_quant_core::market_data::context::MarketContext;

        let pc = price_curve();
        let vc = vol_curve();
        let horizon = base_date() + Duration::days(90);

        let conditioned = MarketContext::new()
            .insert(pc.clone())
            .insert(vc.clone())
            .condition(horizon)
            .unwrap();

        let kept_pc = conditioned.get_price_curve(pc.id().as_str()).unwrap();
        let kept_vc = conditioned.get_vol_index_curve(vc.id().as_str()).unwrap();

        assert_eq!(kept_pc.base_date(), pc.base_date());
        assert_eq!(kept_vc.base_date(), vc.base_date());
        assert!((kept_pc.spot_price() - pc.spot_price()).abs() < TOL);
        assert!((kept_vc.spot_price() - vc.spot_price()).abs() < TOL);
    }

    #[test]
    fn price_curve_serde_roundtrip() {
        let pc = price_curve();
        let restored: PriceCurve =
            serde_json::from_str(&serde_json::to_string(&pc).unwrap()).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0, 2.0] {
            assert!((pc.price(t).unwrap() - restored.price(t).unwrap()).abs() < TOL);
        }
    }

    #[test]
    fn vol_curve_serde_roundtrip() {
        let vc = vol_curve();
        let restored: PriceCurve =
            serde_json::from_str(&serde_json::to_string(&vc).unwrap()).unwrap();

        for t in [0.0, 0.25, 0.5, 1.0, 2.0] {
            assert!((vc.price(t).unwrap() - restored.price(t).unwrap()).abs() < TOL);
        }
    }

    #[test]
    fn spot_inference_consistency_across_curve_types() {
        let pc_inferred = PriceCurve::builder("WTI-INF")
            .base_date(base_date())
            .knots([(0.0, 75.0), (0.25, 76.0), (0.5, 77.0), (1.0, 78.0)])
            .build()
            .unwrap();
        let vc_inferred = PriceCurve::builder("VIX-INF")
            .kind(PriceCurveKind::VolIndex)
            .base_date(base_date())
            .knots([(0.0, 18.0), (0.25, 19.0), (0.5, 20.0), (1.0, 21.0)])
            .build()
            .unwrap();

        assert!((pc_inferred.spot_price() - 75.0).abs() < TOL);
        assert!((vc_inferred.spot_price() - 18.0).abs() < TOL);
        assert!((pc_inferred.price(0.0).unwrap() - pc_inferred.spot_price()).abs() < TOL);
        assert!((vc_inferred.price(0.0).unwrap() - vc_inferred.spot_price()).abs() < TOL);
    }
}
