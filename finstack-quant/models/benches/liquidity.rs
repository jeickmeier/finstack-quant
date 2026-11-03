//! Liquidity estimator benchmarks.

use criterion::{Criterion, criterion_group, criterion_main};
use finstack_quant_models::liquidity::{amihud_illiquidity, roll_effective_spread};
use std::hint::black_box;

fn bench_liquidity_estimators(c: &mut Criterion) {
    let mut group = c.benchmark_group("models_liquidity_estimators");
    let returns: Vec<f64> = (0..16_384)
        .map(|index| if index % 2 == 0 { 0.01 } else { -0.01 })
        .collect();
    let volumes: Vec<f64> = (0..16_384)
        .map(|index| 1_000_000.0 + index as f64)
        .collect();

    group.bench_function("roll_effective_spread_16384", |b| {
        b.iter(|| roll_effective_spread(black_box(&returns)).expect("bench: roll spread"));
    });
    group.bench_function("amihud_illiquidity_16384", |b| {
        b.iter(|| {
            amihud_illiquidity(black_box(&returns), black_box(&volumes)).expect("bench: amihud")
        });
    });
    group.finish();
}

criterion_group!(benches, bench_liquidity_estimators);
criterion_main!(benches);
