//! Tests for the surrounding crate component and its documented behavior.
//!
use finstack_quant_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use finstack_quant_core::market_data::surfaces::{
    VolGridOpts, VolInterpolationMode, VolQuoteType, VolSurface, VolSurfaceAxis,
};

fn black_vol_opts() -> VolGridOpts {
    VolGridOpts {
        secondary_axis: VolSurfaceAxis::Strike,
        quote_type: VolQuoteType::BlackLognormal,
        interpolation_mode: VolInterpolationMode::Vol,
    }
}

fn black_vol_builder(id: &str) -> finstack_quant_core::market_data::surfaces::VolSurfaceBuilder {
    VolSurface::builder(id)
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::BlackLognormal)
        .interpolation_mode(VolInterpolationMode::Vol)
}

#[test]
fn test_vol_surface_builder_basic() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    assert_eq!(surface.id().as_str(), "TEST");
    assert_eq!(surface.expiries(), &[1.0, 2.0]);
    assert_eq!(surface.secondary_values(), &[90.0, 100.0, 110.0]);
}

#[test]
fn test_vol_surface_value_checked() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    // In-bounds queries
    let v = finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 100.0).unwrap();
    assert!(v > 0.19 && v < 0.22);

    // Out-of-bounds should error
    assert!(finstack_quant_models::volatility::get_surface_vol(&surface, 0.5, 100.0).is_err());
    assert!(finstack_quant_models::volatility::get_surface_vol(&surface, 3.0, 100.0).is_err());
    assert!(finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 80.0).is_err());
    assert!(finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 120.0).is_err());
}

#[test]
fn test_vol_surface_value_clamped() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    // Out-of-bounds queries should clamp
    let v1 = finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 0.5, 100.0); // Clamp expiry to 1.0
    let v2 = finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 1.0, 100.0); // Exact
    assert!((v1 - v2).abs() < 1e-10);

    // Clamp strike to 90.0, then interpolate along expiry:
    // expiry=1.5 is midpoint between rows at strike=90: (0.20 + 0.19) / 2 = 0.195
    let v3 = finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 1.5, 80.0);
    assert!((v3 - 0.195).abs() < 1e-12);
}

#[test]
fn value_clamped_handles_nan_and_singleton_axes() {
    let one_by_one = black_vol_builder("ONE")
        .expiries(&[1.0])
        .secondary_values(&[100.0])
        .row(&[0.25])
        .build()
        .unwrap();
    assert_eq!(
        finstack_quant_models::volatility::get_surface_vol_clamped(&one_by_one, 2.0, 120.0),
        0.25
    );
    assert!(
        finstack_quant_models::volatility::get_surface_vol_clamped(&one_by_one, f64::NAN, 100.0)
            .is_nan()
    );
    assert!(
        finstack_quant_models::volatility::get_surface_vol_clamped(&one_by_one, 1.0, f64::NAN)
            .is_nan()
    );

    let one_by_many = black_vol_builder("ROW")
        .expiries(&[1.0])
        .secondary_values(&[90.0, 110.0])
        .row(&[0.20, 0.30])
        .build()
        .unwrap();
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&one_by_many, 3.0, 100.0)
            - 0.25)
            .abs()
            < 1e-12
    );

    let many_by_one = black_vol_builder("COLUMN")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[100.0])
        .row(&[0.20])
        .row(&[0.30])
        .build()
        .unwrap();
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&many_by_one, 1.5, 120.0)
            - 0.25)
            .abs()
            < 1e-12
    );
}

#[test]
fn test_vol_surface_strike_interpolation() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.25, 0.30])
        .build()
        .unwrap();

    // Linear interpolation between strikes
    let v = finstack_quant_models::volatility::get_surface_vol(&surface, 1.0, 95.0).unwrap();
    assert!((v - 0.225).abs() < 0.001); // Should be halfway
}

#[test]
fn test_vol_surface_expiry_interpolation() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[100.0])
        .row(&[0.20])
        .row(&[0.30])
        .build()
        .unwrap();

    // Linear interpolation between expiries
    let v = finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 100.0).unwrap();
    assert!((v - 0.25).abs() < 0.001); // Should be halfway
}

#[test]
fn test_vol_surface_bilinear_interpolation() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 110.0])
        .row(&[0.20, 0.30])
        .row(&[0.25, 0.35])
        .build()
        .unwrap();

    // Bilinear interpolation at center
    let v = finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 100.0).unwrap();
    // Center point => average of four corners
    // (0.20 + 0.30 + 0.25 + 0.35) / 4 = 0.275
    assert!((v - 0.275).abs() < 1e-12);
}

#[test]
fn test_vol_surface_exact_nodes() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0, 3.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.21, 0.22])
        .row(&[0.19, 0.20, 0.21])
        .row(&[0.18, 0.19, 0.20])
        .build()
        .unwrap();

    // Exact node queries
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&surface, 1.0, 90.0).unwrap() - 0.20)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&surface, 2.0, 100.0).unwrap() - 0.20)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&surface, 3.0, 110.0).unwrap() - 0.20)
            .abs()
            < 1e-12
    );
}

#[test]
fn test_vol_surface_builder_validation() {
    // Empty expiries
    let result = black_vol_builder("TEST")
        .expiries(&[])
        .secondary_values(&[100.0])
        .build();
    assert!(result.is_err());

    // Empty strikes
    let result = black_vol_builder("TEST")
        .expiries(&[1.0])
        .secondary_values(&[])
        .build();
    assert!(result.is_err());

    // Wrong number of rows
    let result = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[100.0])
        .row(&[0.2])
        // Missing second row
        .build();
    assert!(result.is_err());

    // Wrong row length
    let result = black_vol_builder("TEST")
        .expiries(&[1.0])
        .secondary_values(&[90.0, 100.0])
        .row(&[0.2]) // Should have 2 values
        .build();
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_unsorted_expiries() {
    let result = black_vol_builder("TEST")
        .expiries(&[2.0, 1.0]) // Unsorted
        .secondary_values(&[100.0])
        .row(&[0.2])
        .row(&[0.2])
        .build();
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_unsorted_strikes() {
    let result = black_vol_builder("TEST")
        .expiries(&[1.0])
        .secondary_values(&[110.0, 90.0]) // Unsorted
        .row(&[0.2, 0.2])
        .build();
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_negative_strikes() {
    // Unshifted Black lognormal surfaces require strictly positive strikes.
    assert!(
        black_vol_builder("TEST-BLACK")
            .expiries(&[1.0])
            .secondary_values(&[-10.0, 0.0, 10.0])
            .row(&[0.2, 0.2, 0.2])
            .build()
            .is_err()
    );

    // Normal (Bachelier) surfaces allow non-positive strikes (e.g. spreads or negative rates).
    let surf = VolSurface::builder("TEST")
        .secondary_axis(VolSurfaceAxis::Strike)
        .quote_type(VolQuoteType::Normal)
        .interpolation_mode(VolInterpolationMode::Vol)
        .expiries(&[1.0])
        .secondary_values(&[-10.0, 0.0, 10.0])
        .row(&[0.2, 0.2, 0.2])
        .build()
        .expect("normal vol surface allows non-positive strikes");
    // Test interpolation with negative strikes
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surf, 1.0, -10.0) - 0.2).abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surf, 1.0, 0.0) - 0.2).abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surf, 1.0, 10.0) - 0.2).abs()
            < 1e-12
    );
}

#[test]
fn test_vol_surface_from_grid() {
    let vols = vec![0.2, 0.21, 0.22, 0.19, 0.2, 0.21];
    let surface = VolSurface::from_grid(
        "TEST",
        &[1.0, 2.0],
        &[90.0, 100.0, 110.0],
        &vols,
        black_vol_opts(),
        None,
    )
    .unwrap();

    assert_eq!(surface.id().as_str(), "TEST");
    assert_eq!(surface.expiries(), &[1.0, 2.0]);
    assert_eq!(surface.secondary_values(), &[90.0, 100.0, 110.0]);
}

#[test]
fn test_vol_surface_from_grid_wrong_size() {
    let vols = vec![0.2, 0.21]; // Should be 6 values (2 * 3)
    let result = VolSurface::from_grid(
        "TEST",
        &[1.0, 2.0],
        &[90.0, 100.0, 110.0],
        &vols,
        black_vol_opts(),
        None,
    );
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_parallel_bump_additive_fraction_and_rejects_percent() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    // Native VolSurface additive shock uses Fraction (+0.01 absolute vol points).
    // Ambiguous Percent and RateBp units must fail closed.
    assert!(
        surface
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Percent,
                value: 1.0,
                bump_type: BumpType::Parallel,
            })
            .is_err()
    );
    assert!(surface.apply_bump(BumpSpec::parallel_bp(10.0)).is_err());

    let bumped = surface
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: 0.01,
            bump_type: BumpType::Parallel,
        })
        .unwrap();

    for &(e, k) in &[(1.0, 90.0), (1.0, 100.0), (2.0, 90.0), (2.0, 100.0)] {
        let orig = finstack_quant_models::volatility::get_surface_vol(&surface, e, k).unwrap();
        let b = finstack_quant_models::volatility::get_surface_vol(&bumped, e, k).unwrap();
        assert!((b - (orig + 0.01)).abs() < 1e-12);
    }
}

#[test]
fn test_vol_surface_parallel_bump_multiplicative_factor() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    let bumped = surface.apply_bump(BumpSpec::multiplier(1.10)).unwrap();
    let orig = finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 95.0).unwrap();
    let b = finstack_quant_models::volatility::get_surface_vol(&bumped, 1.5, 95.0).unwrap();
    assert!((b - orig * 1.10).abs() < 1e-12);
}

#[test]
fn test_vol_surface_bump_rejects_negative_shocked_vol() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0])
        .secondary_values(&[100.0])
        .row(&[0.005])
        .build()
        .unwrap();

    // Negative additive bump driving grid vol below zero must error instead of clamping.
    assert!(
        surface
            .apply_bump(BumpSpec {
                mode: BumpMode::Additive,
                units: BumpUnits::Fraction,
                value: -1.0,
                bump_type: BumpType::Parallel,
            })
            .is_err()
    );

    let exact_zero = surface
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: -0.005,
            bump_type: BumpType::Parallel,
        })
        .unwrap();
    assert!(
        finstack_quant_models::volatility::get_surface_vol(&exact_zero, 1.0, 100.0)
            .unwrap()
            .abs()
            < 1e-15
    );
}

#[test]
fn test_vol_surface_rejects_key_rate_bumps() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    let err = surface
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: 0.01,
            bump_type: BumpType::TriangularKeyRate {
                prev_bucket: Some(1.0),
                target_bucket: 2.0,
                next_bucket: Some(3.0),
            },
        })
        .expect_err("VolSurface should reject key-rate bumps");
    let msg = format!("{err}");
    assert!(msg.contains("VolSurface") || msg.contains("Parallel"));
}

#[test]
fn test_vol_surface_bucket_bump_filters() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    // Bump only (expiry=2.0, strike=90.0) by +10%
    let bumped = surface
        .apply_bucket_bump(Some(&[2.0]), Some(&[90.0]), BumpSpec::multiplier(1.10))
        .expect("bucket bump should succeed");

    // Bumped bucket
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&bumped, 2.0, 90.0).unwrap()
            - 0.19 * 1.10)
            .abs()
            < 1e-12
    );
    // Unchanged buckets
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&bumped, 1.0, 90.0).unwrap() - 0.20)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&bumped, 1.0, 100.0).unwrap() - 0.21)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&bumped, 2.0, 100.0).unwrap() - 0.20)
            .abs()
            < 1e-12
    );
}

#[test]
fn test_vol_surface_single_point() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0])
        .secondary_values(&[100.0])
        .row(&[0.25])
        .build()
        .unwrap();

    // Exact query
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&surface, 1.0, 100.0).unwrap() - 0.25)
            .abs()
            < 1e-12
    );

    // Clamped queries (should all give same value)
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 0.5, 100.0) - 0.25)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 2.0, 100.0) - 0.25)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 1.0, 90.0) - 0.25)
            .abs()
            < 1e-12
    );
    assert!(
        (finstack_quant_models::volatility::get_surface_vol_clamped(&surface, 1.0, 110.0) - 0.25)
            .abs()
            < 1e-12
    );
}

#[test]
fn test_vol_surface_large_grid() {
    let expiries: Vec<f64> = (1..=10).map(|i| i as f64).collect();
    let strikes: Vec<f64> = (80..=120).step_by(5).map(|s| s as f64).collect();

    let mut builder = black_vol_builder("LARGE");
    builder = builder.expiries(&expiries).secondary_values(&strikes);

    for _ in 0..expiries.len() {
        let row: Vec<f64> = strikes.iter().map(|_| 0.25).collect();
        builder = builder.row(&row);
    }

    let surface = builder.build().unwrap();

    // Test interpolation in large grid
    let v = finstack_quant_models::volatility::get_surface_vol(&surface, 5.5, 100.0).unwrap();
    assert!((v - 0.25).abs() < 0.001);
}

#[test]
fn test_vol_surface_serde() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.21, 0.22])
        .row(&[0.19, 0.20, 0.21])
        .build()
        .unwrap();

    let json = serde_json::to_string(&surface).unwrap();
    let deser: VolSurface = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.id(), surface.id());
    assert_eq!(deser.expiries(), surface.expiries());
    assert_eq!(deser.secondary_values(), surface.secondary_values());

    // Check values match
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&deser, 1.5, 100.0).unwrap()
            - finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 100.0).unwrap())
        .abs()
            < 1e-12
    );
}

#[test]
fn test_vol_surface_serde_invalid() {
    // Mismatched dimensions
    let json = r#"{
        "id": "TEST",
        "expiries": [1.0, 2.0],
        "secondary_values": [90.0, 100.0],
        "secondary_axis": "strike",
        "quote_type": "black_lognormal",
        "interpolation_mode": "vol",
        "vols_row_major": [0.2, 0.21]
    }"#;
    let result: Result<VolSurface, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_clone() {
    let surface = black_vol_builder("TEST")
        .expiries(&[1.0, 2.0])
        .secondary_values(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    let cloned = surface.clone();

    assert_eq!(cloned.id(), surface.id());
    assert_eq!(cloned.expiries(), surface.expiries());
    assert_eq!(cloned.secondary_values(), surface.secondary_values());
    assert!(
        (finstack_quant_models::volatility::get_surface_vol(&cloned, 1.5, 95.0).unwrap()
            - finstack_quant_models::volatility::get_surface_vol(&surface, 1.5, 95.0).unwrap())
        .abs()
            < 1e-12
    );
}

#[test]
fn total_variance_preserves_exact_knots_and_scales_extreme_vols() {
    use finstack_quant_models::volatility::{VolSource, get_surface_vol, get_surface_vol_clamped};
    use std::sync::Arc;

    let knots = VolSurface::from_grid(
        "TV-KNOTS",
        &[0.5, 1.5],
        &[90.0, 110.0],
        &[0.19, 0.23, 0.27, 0.31],
        VolGridOpts {
            secondary_axis: VolSurfaceAxis::Strike,
            quote_type: VolQuoteType::BlackLognormal,
            interpolation_mode: VolInterpolationMode::TotalVariance,
        },
        None,
    )
    .unwrap();
    for (ei, &t) in knots.expiries().iter().enumerate() {
        for (si, &k) in knots.secondary_values().iter().enumerate() {
            let expected = knots.vols()[ei * 2 + si];
            assert_eq!(
                get_surface_vol(&knots, t, k).unwrap().to_bits(),
                expected.to_bits()
            );
        }
    }

    for &extreme in &[1e160_f64, 1e-200_f64] {
        let surf = VolSurface::from_grid(
            "TV-EXTREME",
            &[1.0, 2.0],
            &[90.0, 110.0],
            &[extreme; 4],
            VolGridOpts {
                secondary_axis: VolSurfaceAxis::Strike,
                quote_type: VolQuoteType::BlackLognormal,
                interpolation_mode: VolInterpolationMode::TotalVariance,
            },
            None,
        )
        .unwrap();
        let mid = get_surface_vol(&surf, 1.5, 100.0).unwrap();
        assert!(mid.is_finite() && mid > 0.0, "extreme={extreme}, mid={mid}");
        assert!((mid - extreme).abs() / extreme < 1e-12);
    }

    let source = VolSource::Surface(Arc::new(knots.clone()));
    assert!(source.get_vol_clamped(0.0, 0.0, 100.0).is_err());
    assert!(source.get_vol_clamped(-0.1, 0.0, 100.0).is_err());
    assert!(get_surface_vol_clamped(&knots, 0.0, 100.0).is_nan());
    assert!(get_surface_vol_clamped(&knots, -0.1, 100.0).is_nan());
}
