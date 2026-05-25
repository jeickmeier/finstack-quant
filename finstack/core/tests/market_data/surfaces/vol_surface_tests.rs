use finstack_core::market_data::bumps::{BumpMode, BumpSpec, BumpType, BumpUnits, Bumpable};
use finstack_core::market_data::surfaces::VolSurface;

#[test]
fn test_vol_surface_builder_basic() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    assert_eq!(surface.id().as_str(), "TEST");
    assert_eq!(surface.expiries(), &[1.0, 2.0]);
    assert_eq!(surface.strikes(), &[90.0, 100.0, 110.0]);
}

#[test]
fn test_vol_surface_value_checked() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    // In-bounds queries
    let v = surface.value_checked(1.5, 100.0).unwrap();
    assert!(v > 0.19 && v < 0.22);

    // Out-of-bounds should error
    assert!(surface.value_checked(0.5, 100.0).is_err());
    assert!(surface.value_checked(3.0, 100.0).is_err());
    assert!(surface.value_checked(1.5, 80.0).is_err());
    assert!(surface.value_checked(1.5, 120.0).is_err());
}

#[test]
fn test_vol_surface_value_clamped() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.2, 0.21, 0.22])
        .row(&[0.19, 0.2, 0.21])
        .build()
        .unwrap();

    // Out-of-bounds queries should clamp
    let v1 = surface.value_clamped(0.5, 100.0); // Clamp expiry to 1.0
    let v2 = surface.value_clamped(1.0, 100.0); // Exact
    assert!((v1 - v2).abs() < 1e-10);

    // Clamp strike to 90.0, then interpolate along expiry:
    // expiry=1.5 is midpoint between rows at strike=90: (0.20 + 0.19) / 2 = 0.195
    let v3 = surface.value_clamped(1.5, 80.0);
    assert!((v3 - 0.195).abs() < 1e-12);
}

#[test]
fn test_vol_surface_strike_interpolation() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.25, 0.30])
        .build()
        .unwrap();

    // Linear interpolation between strikes
    let v = surface.value_checked(1.0, 95.0).unwrap();
    assert!((v - 0.225).abs() < 0.001); // Should be halfway
}

#[test]
fn test_vol_surface_expiry_interpolation() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[100.0])
        .row(&[0.20])
        .row(&[0.30])
        .build()
        .unwrap();

    // Linear interpolation between expiries
    let v = surface.value_checked(1.5, 100.0).unwrap();
    assert!((v - 0.25).abs() < 0.001); // Should be halfway
}

#[test]
fn test_vol_surface_bilinear_interpolation() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 110.0])
        .row(&[0.20, 0.30])
        .row(&[0.25, 0.35])
        .build()
        .unwrap();

    // Bilinear interpolation at center
    let v = surface.value_checked(1.5, 100.0).unwrap();
    // Center point => average of four corners
    // (0.20 + 0.30 + 0.25 + 0.35) / 4 = 0.275
    assert!((v - 0.275).abs() < 1e-12);
}

#[test]
fn test_vol_surface_exact_nodes() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0, 3.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.21, 0.22])
        .row(&[0.19, 0.20, 0.21])
        .row(&[0.18, 0.19, 0.20])
        .build()
        .unwrap();

    // Exact node queries
    assert!((surface.value_checked(1.0, 90.0).unwrap() - 0.20).abs() < 1e-12);
    assert!((surface.value_checked(2.0, 100.0).unwrap() - 0.20).abs() < 1e-12);
    assert!((surface.value_checked(3.0, 110.0).unwrap() - 0.20).abs() < 1e-12);
}

#[test]
fn test_vol_surface_builder_validation() {
    // Empty expiries
    let result = VolSurface::builder("TEST")
        .expiries(&[])
        .strikes(&[100.0])
        .build();
    assert!(result.is_err());

    // Empty strikes
    let result = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[])
        .build();
    assert!(result.is_err());

    // Wrong number of rows
    let result = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[100.0])
        .row(&[0.2])
        // Missing second row
        .build();
    assert!(result.is_err());

    // Wrong row length
    let result = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.2]) // Should have 2 values
        .build();
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_unsorted_expiries() {
    let result = VolSurface::builder("TEST")
        .expiries(&[2.0, 1.0]) // Unsorted
        .strikes(&[100.0])
        .row(&[0.2])
        .row(&[0.2])
        .build();
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_unsorted_strikes() {
    let result = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[110.0, 90.0]) // Unsorted
        .row(&[0.2, 0.2])
        .build();
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_negative_strikes() {
    // Negative strikes are allowed (for certain products like spreads)
    let result = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[-10.0, 0.0, 10.0])
        .row(&[0.2, 0.2, 0.2])
        .build();
    assert!(result.is_ok());

    let surf = result.unwrap();
    // Test interpolation with negative strikes
    assert!((surf.value_clamped(1.0, -10.0) - 0.2).abs() < 1e-12);
    assert!((surf.value_clamped(1.0, 0.0) - 0.2).abs() < 1e-12);
    assert!((surf.value_clamped(1.0, 10.0) - 0.2).abs() < 1e-12);
}

#[test]
fn test_vol_surface_from_grid() {
    let vols = vec![0.2, 0.21, 0.22, 0.19, 0.2, 0.21];
    let surface = VolSurface::from_grid("TEST", &[1.0, 2.0], &[90.0, 100.0, 110.0], &vols).unwrap();

    assert_eq!(surface.id().as_str(), "TEST");
    assert_eq!(surface.expiries(), &[1.0, 2.0]);
    assert_eq!(surface.strikes(), &[90.0, 100.0, 110.0]);
}

#[test]
fn test_vol_surface_from_grid_wrong_size() {
    let vols = vec![0.2, 0.21]; // Should be 6 values (2 * 3)
    let result = VolSurface::from_grid("TEST", &[1.0, 2.0], &[90.0, 100.0, 110.0], &vols);
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_parallel_bump_additive_percent() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    // +1% additive in Percent units => +0.01 absolute vol points (not a 1% relative change)
    let bumped = surface
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: 1.0,
            bump_type: BumpType::Parallel,
        })
        .unwrap();

    for &(e, k) in &[(1.0, 90.0), (1.0, 100.0), (2.0, 90.0), (2.0, 100.0)] {
        let orig = surface.value_checked(e, k).unwrap();
        let b = bumped.value_checked(e, k).unwrap();
        assert!((b - (orig + 0.01)).abs() < 1e-12);
    }
}

#[test]
fn test_vol_surface_parallel_bump_multiplicative_factor() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    let bumped = surface.apply_bump(BumpSpec::multiplier(1.10)).unwrap();
    let orig = surface.value_checked(1.5, 95.0).unwrap();
    let b = bumped.value_checked(1.5, 95.0).unwrap();
    assert!((b - orig * 1.10).abs() < 1e-12);
}

#[test]
fn test_vol_surface_bump_clamps_to_zero() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[0.005])
        .build()
        .unwrap();

    // Large negative additive bump should clamp to zero
    let bumped = surface
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Fraction,
            value: -1.0,
            bump_type: BumpType::Parallel,
        })
        .unwrap();
    assert_eq!(bumped.value_checked(1.0, 100.0).unwrap(), 0.0);
}

#[test]
fn test_vol_surface_rejects_key_rate_bumps() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    let err = surface
        .apply_bump(BumpSpec {
            mode: BumpMode::Additive,
            units: BumpUnits::Percent,
            value: 1.0,
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
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    // Bump only (expiry=2.0, strike=90.0) by +10%
    let bumped = surface
        .apply_bucket_bump(Some(&[2.0]), Some(&[90.0]), 10.0)
        .expect("bucket bump should succeed");

    // Bumped bucket
    assert!((bumped.value_checked(2.0, 90.0).unwrap() - 0.19 * 1.10).abs() < 1e-12);
    // Unchanged buckets
    assert!((bumped.value_checked(1.0, 90.0).unwrap() - 0.20).abs() < 1e-12);
    assert!((bumped.value_checked(1.0, 100.0).unwrap() - 0.21).abs() < 1e-12);
    assert!((bumped.value_checked(2.0, 100.0).unwrap() - 0.20).abs() < 1e-12);
}

#[test]
fn test_vol_surface_single_point() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0])
        .strikes(&[100.0])
        .row(&[0.25])
        .build()
        .unwrap();

    // Exact query
    assert!((surface.value_checked(1.0, 100.0).unwrap() - 0.25).abs() < 1e-12);

    // Clamped queries (should all give same value)
    assert!((surface.value_clamped(0.5, 100.0) - 0.25).abs() < 1e-12);
    assert!((surface.value_clamped(2.0, 100.0) - 0.25).abs() < 1e-12);
    assert!((surface.value_clamped(1.0, 90.0) - 0.25).abs() < 1e-12);
    assert!((surface.value_clamped(1.0, 110.0) - 0.25).abs() < 1e-12);
}

#[test]
fn test_vol_surface_large_grid() {
    let expiries: Vec<f64> = (1..=10).map(|i| i as f64).collect();
    let strikes: Vec<f64> = (80..=120).step_by(5).map(|s| s as f64).collect();

    let mut builder = VolSurface::builder("LARGE");
    builder = builder.expiries(&expiries).strikes(&strikes);

    for _ in 0..expiries.len() {
        let row: Vec<f64> = strikes.iter().map(|_| 0.25).collect();
        builder = builder.row(&row);
    }

    let surface = builder.build().unwrap();

    // Test interpolation in large grid
    let v = surface.value_checked(5.5, 100.0).unwrap();
    assert!((v - 0.25).abs() < 0.001);
}

#[test]
fn test_vol_surface_serde() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.20, 0.21, 0.22])
        .row(&[0.19, 0.20, 0.21])
        .build()
        .unwrap();

    let json = serde_json::to_string(&surface).unwrap();
    let deser: VolSurface = serde_json::from_str(&json).unwrap();

    assert_eq!(deser.id(), surface.id());
    assert_eq!(deser.expiries(), surface.expiries());
    assert_eq!(deser.strikes(), surface.strikes());

    // Check values match
    assert!(
        (deser.value_checked(1.5, 100.0).unwrap() - surface.value_checked(1.5, 100.0).unwrap())
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
        "strikes": [90.0, 100.0],
        "vols_row_major": [0.2, 0.21]
    }"#;
    let result: Result<VolSurface, _> = serde_json::from_str(json);
    assert!(result.is_err());
}

#[test]
fn test_vol_surface_clone() {
    let surface = VolSurface::builder("TEST")
        .expiries(&[1.0, 2.0])
        .strikes(&[90.0, 100.0])
        .row(&[0.20, 0.21])
        .row(&[0.19, 0.20])
        .build()
        .unwrap();

    let cloned = surface.clone();

    assert_eq!(cloned.id(), surface.id());
    assert_eq!(cloned.expiries(), surface.expiries());
    assert_eq!(cloned.strikes(), surface.strikes());
    assert!(
        (cloned.value_checked(1.5, 95.0).unwrap() - surface.value_checked(1.5, 95.0).unwrap())
            .abs()
            < 1e-12
    );
}
