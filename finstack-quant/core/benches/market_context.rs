//! Benchmarks for MarketContext lookups and bump operations.
//!
//! Tests performance of:
//! - Curve lookups (discount, forward, hazard)
//! - Surface lookups
//! - Context bump operations (parallel, key-rate)
//! - Context cloning

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::bumps::MarketBump;
use finstack_quant_core::market_data::context::{BumpSpec, MarketContext};
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::market_data::term_structures::CreditIndexData;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::market_data::term_structures::ForwardCurve;
use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_core::types::CurveId;
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

fn create_base_date() -> Date {
    Date::from_calendar_date(2024, Month::January, 1).expect("Valid date")
}

fn create_discount_curve(id: &str, num_points: usize) -> DiscountCurve {
    let base_date = create_base_date();
    let knots: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64) * 0.5;
            let df = (-0.04 * t).exp();
            (t, df)
        })
        .collect();

    DiscountCurve::builder(id)
        .base_date(base_date)
        .knots(knots)
        .build()
        .expect("DiscountCurve builder should succeed")
}

fn create_forward_curve(id: &str, num_points: usize) -> ForwardCurve {
    let base_date = create_base_date();
    let knots: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64) * 0.5;
            let fwd = 0.04 + 0.001 * t; // Slightly upward sloping
            (t, fwd)
        })
        .collect();

    ForwardCurve::builder(id, 0.25) // 3M = 0.25 years
        .base_date(base_date)
        .knots(knots)
        .build()
        .expect("ForwardCurve builder should succeed")
}

fn create_hazard_curve(id: &str, num_points: usize) -> HazardCurve {
    let base_date = create_base_date();
    let knots: Vec<(f64, f64)> = (0..num_points)
        .map(|i| {
            let t = (i as f64) * 0.5;
            let hazard = 0.01 + 0.002 * t;
            (t, hazard)
        })
        .collect();

    HazardCurve::builder(id)
        .base_date(base_date)
        .knots(knots)
        .build()
        .expect("HazardCurve builder should succeed")
}

fn create_vol_surface(id: &str, n_expiries: usize, n_strikes: usize) -> VolSurface {
    let expiries: Vec<f64> = (1..=n_expiries).map(|i| i as f64 * 0.5).collect();
    let strikes: Vec<f64> = (0..n_strikes).map(|i| 80.0 + (i as f64) * 5.0).collect();

    let mut builder = VolSurface::builder(id)
        .expiries(&expiries)
        .strikes(&strikes);

    for _ in 0..n_expiries {
        let row: Vec<f64> = (0..n_strikes).map(|j| 0.2 + 0.01 * j as f64).collect();
        builder = builder.row(&row);
    }

    builder.build().expect("VolSurface builder should succeed")
}

fn create_base_correlation_curve(id: &str) -> BaseCorrelationCurve {
    BaseCorrelationCurve::builder(id)
        .knots([(3.0, 0.25), (7.0, 0.40), (10.0, 0.55)])
        .build()
        .expect("BaseCorrelationCurve builder should succeed")
}

fn create_populated_context(num_curves: usize, points_per_curve: usize) -> MarketContext {
    let mut ctx = MarketContext::new();

    for i in 0..num_curves {
        let discount = create_discount_curve(&format!("DISC-{}", i), points_per_curve);
        ctx = ctx.insert(discount);

        let forward = create_forward_curve(&format!("FWD-{}", i), points_per_curve);
        ctx = ctx.insert(forward);

        let hazard = create_hazard_curve(&format!("HAZ-{}", i), points_per_curve);
        ctx = ctx.insert(hazard);
    }

    // Add some vol surfaces
    for i in 0..5 {
        let surface = create_vol_surface(&format!("VOL-{}", i), 10, 10);
        ctx = ctx.insert_surface(surface);
    }

    ctx
}

fn create_credit_index_context(count: usize, points_per_curve: usize) -> MarketContext {
    let mut ctx = MarketContext::new();

    for i in 0..count {
        let hazard = create_hazard_curve(&format!("CDX-HAZ-{}", i), points_per_curve);
        let base_corr = create_base_correlation_curve(&format!("CDX-BC-{}", i));
        let credit_index = CreditIndexData::builder()
            .num_constituents(125)
            .recovery_rate(0.4)
            .index_credit_curve(Arc::new(hazard.clone()))
            .base_correlation_curve(Arc::new(base_corr.clone()))
            .build()
            .expect("CreditIndexData builder should succeed");

        ctx = ctx
            .insert(hazard)
            .insert(base_corr)
            .insert_credit_index(format!("CDX-{}", i), credit_index);
    }

    ctx
}

fn bench_context_curve_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_context_lookup");

    // Small context (10 curves of each type)
    let ctx_small = create_populated_context(10, 20);

    // Medium context (50 curves of each type)
    let ctx_medium = create_populated_context(50, 20);

    // Large context (100 curves of each type)
    let ctx_large = create_populated_context(100, 20);

    // Lookup benchmarks
    group.bench_function("discount_lookup_small", |b| {
        b.iter(|| {
            let curve = black_box(&ctx_small)
                .get_discount(black_box("DISC-5"))
                .expect("Curve should exist");
            black_box(curve);
        })
    });

    group.bench_function("discount_lookup_medium", |b| {
        b.iter(|| {
            let curve = black_box(&ctx_medium)
                .get_discount(black_box("DISC-25"))
                .expect("Curve should exist");
            black_box(curve);
        })
    });

    group.bench_function("discount_lookup_large", |b| {
        b.iter(|| {
            let curve = black_box(&ctx_large)
                .get_discount(black_box("DISC-50"))
                .expect("Curve should exist");
            black_box(curve);
        })
    });

    group.bench_function("forward_lookup", |b| {
        b.iter(|| {
            let curve = black_box(&ctx_medium)
                .get_forward(black_box("FWD-25"))
                .expect("Curve should exist");
            black_box(curve);
        })
    });

    group.bench_function("hazard_lookup", |b| {
        b.iter(|| {
            let curve = black_box(&ctx_medium)
                .get_hazard(black_box("HAZ-25"))
                .expect("Curve should exist");
            black_box(curve);
        })
    });

    group.bench_function("surface_lookup", |b| {
        b.iter(|| {
            let surface = black_box(&ctx_medium)
                .get_surface(black_box("VOL-2"))
                .expect("Surface should exist");
            black_box(surface);
        })
    });

    group.bench_function("discount_ref_lookup", |b| {
        b.iter(|| {
            let curve = black_box(&ctx_medium)
                .get_discount(black_box("DISC-25"))
                .expect("Curve should exist");
            black_box(curve);
        })
    });

    group.finish();
}

fn bench_context_bump_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_context_bump");

    let ctx = create_populated_context(20, 20);

    // Single curve parallel bump
    group.bench_function("parallel_bump_single", |b| {
        let bumps = vec![MarketBump::Curve {
            id: CurveId::new("DISC-5"),
            spec: BumpSpec::parallel_bp(10.0),
        }];
        b.iter(|| {
            let bumped = black_box(&ctx)
                .bump(black_box(bumps.clone()))
                .expect("Bump should succeed");
            black_box(bumped);
        })
    });

    // Multiple curves parallel bump
    group.bench_function("parallel_bump_multiple", |b| {
        let bumps: Vec<MarketBump> = (0..5)
            .map(|i| MarketBump::Curve {
                id: CurveId::new(format!("DISC-{}", i)),
                spec: BumpSpec::parallel_bp(10.0),
            })
            .collect();
        b.iter(|| {
            let bumped = black_box(&ctx)
                .bump(black_box(bumps.clone()))
                .expect("Bump should succeed");
            black_box(bumped);
        })
    });

    // Bump all discount curves
    group.bench_function("parallel_bump_all_discount", |b| {
        let bumps: Vec<MarketBump> = (0..20)
            .map(|i| MarketBump::Curve {
                id: CurveId::new(format!("DISC-{}", i)),
                spec: BumpSpec::parallel_bp(10.0),
            })
            .collect();
        b.iter(|| {
            let bumped = black_box(&ctx)
                .bump(black_box(bumps.clone()))
                .expect("Bump should succeed");
            black_box(bumped);
        })
    });

    group.finish();
}

fn bench_context_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_context_clone");

    for size in [10, 50, 100] {
        group.bench_with_input(BenchmarkId::new("clone", size), &size, |b, &size| {
            let ctx = create_populated_context(size, 20);
            b.iter(|| {
                let cloned = black_box(&ctx).clone();
                black_box(cloned);
            })
        });
    }

    group.finish();
}

fn bench_batch_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_context_batch_lookup");

    let ctx = create_populated_context(100, 20);

    // Batch of 10 lookups
    group.bench_function("batch_10_lookups", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(10);
            for i in 0..10 {
                let curve = ctx
                    .get_discount(format!("DISC-{}", i))
                    .expect("Curve should exist");
                results.push(curve);
            }
            black_box(results);
        })
    });

    // Batch of 50 lookups
    group.bench_function("batch_50_lookups", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(50);
            for i in 0..50 {
                let curve = ctx
                    .get_discount(format!("DISC-{}", i))
                    .expect("Curve should exist");
                results.push(curve);
            }
            black_box(results);
        })
    });

    group.finish();
}

fn bench_credit_index_rebinding(c: &mut Criterion) {
    let mut group = c.benchmark_group("market_context_credit_rebinding");
    let ctx = create_credit_index_context(20, 20);

    group.bench_function("bump_hazard_curves_with_credit_indices", |b| {
        let bumps: Vec<MarketBump> = (0..10)
            .map(|i| MarketBump::Curve {
                id: CurveId::new(format!("CDX-HAZ-{}", i)),
                spec: BumpSpec::parallel_bp(10.0),
            })
            .collect();

        b.iter(|| {
            let bumped = black_box(&ctx)
                .bump(black_box(bumps.clone()))
                .expect("hazard bumps should succeed");
            black_box(bumped);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_context_curve_lookup,
    bench_context_bump_operations,
    bench_context_clone,
    bench_batch_lookups,
    bench_credit_index_rebinding,
);
criterion_main!(benches);
