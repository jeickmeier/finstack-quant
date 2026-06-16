//! Benchmarks for volatility surface operations.
//!
//! Tests performance of:
//! - Surface construction (builder pattern)
//! - Bilinear interpolation (single and batch)
//! - Clamped vs checked lookups
//! - Surface bump operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::market_data::bumps::{BumpSpec, Bumpable};
use finstack_quant_core::market_data::surfaces::VolSurface;
use std::hint::black_box;

fn create_vol_surface(n_expiries: usize, n_strikes: usize) -> VolSurface {
    let expiries: Vec<f64> = (1..=n_expiries).map(|i| i as f64 * 0.5).collect();
    let strikes: Vec<f64> = (0..n_strikes).map(|i| 80.0 + (i as f64) * 5.0).collect();

    let mut builder = VolSurface::builder("TEST-VOL")
        .expiries(&expiries)
        .strikes(&strikes);

    // Create realistic volatility smile
    for _ in 0..n_expiries {
        let row: Vec<f64> = (0..n_strikes)
            .map(|j| {
                let moneyness = (j as f64 - n_strikes as f64 / 2.0) / 10.0;
                0.20 + 0.02 * moneyness * moneyness // Smile shape
            })
            .collect();
        builder = builder.row(&row);
    }

    builder.build().expect("VolSurface builder should succeed")
}

fn bench_vol_surface_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_surface_construction");

    // Small surface: 5x5
    group.bench_function("construct_5x5", |b| {
        let expiries: Vec<f64> = (1..=5).map(|i| i as f64 * 0.5).collect();
        let strikes: Vec<f64> = (0..5).map(|i| 90.0 + (i as f64) * 5.0).collect();
        let vols: Vec<f64> = vec![0.2; 25];

        b.iter(|| {
            let surface = VolSurface::from_grid(
                black_box("TEST"),
                black_box(&expiries),
                black_box(&strikes),
                black_box(&vols),
            )
            .expect("Surface creation should succeed");
            black_box(surface);
        })
    });

    // Medium surface: 10x10
    group.bench_function("construct_10x10", |b| {
        let expiries: Vec<f64> = (1..=10).map(|i| i as f64 * 0.5).collect();
        let strikes: Vec<f64> = (0..10).map(|i| 80.0 + (i as f64) * 5.0).collect();
        let vols: Vec<f64> = vec![0.2; 100];

        b.iter(|| {
            let surface = VolSurface::from_grid(
                black_box("TEST"),
                black_box(&expiries),
                black_box(&strikes),
                black_box(&vols),
            )
            .expect("Surface creation should succeed");
            black_box(surface);
        })
    });

    // Large surface: 20x20
    group.bench_function("construct_20x20", |b| {
        let expiries: Vec<f64> = (1..=20).map(|i| i as f64 * 0.25).collect();
        let strikes: Vec<f64> = (0..20).map(|i| 70.0 + (i as f64) * 3.0).collect();
        let vols: Vec<f64> = vec![0.2; 400];

        b.iter(|| {
            let surface = VolSurface::from_grid(
                black_box("TEST"),
                black_box(&expiries),
                black_box(&strikes),
                black_box(&vols),
            )
            .expect("Surface creation should succeed");
            black_box(surface);
        })
    });

    group.finish();
}

fn bench_vol_surface_single_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_surface_single_lookup");

    let surface = create_vol_surface(10, 10);

    // Lookup at grid point (fast path)
    group.bench_function("lookup_at_grid_point", |b| {
        b.iter(|| {
            let vol = black_box(&surface)
                .value_checked(black_box(1.0), black_box(100.0))
                .expect("Lookup should succeed");
            black_box(vol);
        })
    });

    // Lookup requiring interpolation
    group.bench_function("lookup_interpolated", |b| {
        b.iter(|| {
            let vol = black_box(&surface)
                .value_checked(black_box(1.25), black_box(97.5))
                .expect("Lookup should succeed");
            black_box(vol);
        })
    });

    // Clamped lookup (with extrapolation)
    group.bench_function("lookup_clamped", |b| {
        b.iter(|| {
            let vol = black_box(&surface).value_clamped(black_box(0.1), black_box(75.0));
            black_box(vol);
        })
    });

    group.bench_function("lookup_checked_in_bounds", |b| {
        b.iter(|| {
            let vol = black_box(&surface)
                .value_checked(black_box(1.25), black_box(97.5))
                .expect("benchmark point is in bounds");
            black_box(vol);
        })
    });

    group.finish();
}

fn bench_vol_surface_batch_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_surface_batch_lookup");

    let surface = create_vol_surface(10, 10);

    for batch_size in [10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("checked", batch_size),
            &batch_size,
            |b, &size| {
                let points: Vec<(f64, f64)> = (0..size)
                    .map(|i| {
                        let t = 0.5 + (i as f64 / size as f64) * 4.0;
                        let k = 85.0 + (i as f64 / size as f64) * 40.0;
                        (t, k)
                    })
                    .collect();

                b.iter(|| {
                    let vols: Vec<f64> = points
                        .iter()
                        .map(|&(t, k)| surface.value_checked(t, k).expect("Lookup should succeed"))
                        .collect();
                    black_box(vols);
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("clamped", batch_size),
            &batch_size,
            |b, &size| {
                let points: Vec<(f64, f64)> = (0..size)
                    .map(|i| {
                        let t = 0.1 + (i as f64 / size as f64) * 6.0; // Some out-of-bounds
                        let k = 70.0 + (i as f64 / size as f64) * 60.0;
                        (t, k)
                    })
                    .collect();

                b.iter(|| {
                    let vols: Vec<f64> = points
                        .iter()
                        .map(|&(t, k)| surface.value_clamped(t, k))
                        .collect();
                    black_box(vols);
                })
            },
        );
    }

    group.finish();
}

fn bench_vol_surface_bump(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_surface_bump");

    // Test different surface sizes
    for (n_expiries, n_strikes) in [(5, 5), (10, 10), (20, 20)] {
        let surface = create_vol_surface(n_expiries, n_strikes);
        let label = format!("{}x{}", n_expiries, n_strikes);

        // Parallel bump (all vols)
        group.bench_function(BenchmarkId::new("parallel_bump", &label), |b| {
            b.iter(|| {
                let bumped = black_box(&surface)
                    .apply_bump(BumpSpec::parallel_bp(100.0))
                    .expect("Bump should succeed");
                black_box(bumped);
            })
        });

        // Scaled surface
        group.bench_function(BenchmarkId::new("scaled", &label), |b| {
            b.iter(|| {
                let scaled = black_box(&surface).scaled(black_box(1.01));
                black_box(scaled);
            })
        });

        // Point bump
        group.bench_function(BenchmarkId::new("point_bump", &label), |b| {
            b.iter(|| {
                let bumped = black_box(&surface)
                    .bump_point(black_box(1.5), black_box(100.0), black_box(0.01))
                    .expect("Point bump should succeed");
                black_box(bumped);
            })
        });
    }

    group.finish();
}

fn bench_vol_surface_grid_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_surface_grid_comparison");

    // Compare lookup performance across grid sizes
    for (n_expiries, n_strikes) in [(5, 5), (10, 10), (20, 20), (50, 50)] {
        let surface = create_vol_surface(n_expiries, n_strikes);
        let label = format!("{}x{}", n_expiries, n_strikes);

        group.bench_function(BenchmarkId::new("single_lookup", &label), |b| {
            // Pick a point in the middle of the grid
            let t = (n_expiries as f64) * 0.25;
            let k = 80.0 + (n_strikes as f64) * 2.5;
            b.iter(|| {
                let vol = black_box(&surface)
                    .value_checked(black_box(t), black_box(k))
                    .expect("Lookup should succeed");
                black_box(vol);
            })
        });

        group.bench_function(BenchmarkId::new("100_lookups", &label), |b| {
            let max_expiry = (n_expiries as f64) * 0.5;
            let max_strike = 80.0 + (n_strikes as f64) * 5.0;
            let points: Vec<(f64, f64)> = (0..100)
                .map(|i| {
                    let t = 0.5 + (i as f64 / 100.0) * (max_expiry - 0.5);
                    let k = 85.0 + (i as f64 / 100.0) * (max_strike - 85.0 - 5.0);
                    (t, k)
                })
                .collect();

            b.iter(|| {
                let vols: Vec<f64> = points
                    .iter()
                    .map(|&(t, k)| surface.value_checked(t, k).expect("Lookup should succeed"))
                    .collect();
                black_box(vols);
            })
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_vol_surface_construction,
    bench_vol_surface_single_lookup,
    bench_vol_surface_batch_lookup,
    bench_vol_surface_bump,
    bench_vol_surface_grid_sizes,
);
criterion_main!(benches);
