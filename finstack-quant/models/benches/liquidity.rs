//! Liquidity estimator benchmarks.

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_models::liquidity::{amihud_illiquidity, roll_effective_spread};
use std::hint::black_box;

fn bench_liquidity_estimators(c: &mut Criterion) {
    let mut group = c.benchmark_group("models_liquidity_estimators");
    group.throughput(Throughput::Elements(16_384));

    group.bench_function("roll_effective_spread_16384", |b| {
        let returns: Vec<f64> = (0..16_384)
            .map(|index| if index % 2 == 0 { 0.01 } else { -0.01 })
            .collect();
        let spread = roll_effective_spread(&returns).expect("valid bid-ask bounce fixture");
        // Even alternating +/- 1% returns have zero mean and covariance -0.01².
        assert!(spread.is_finite() && (spread - 0.02).abs() <= 1e-12);
        b.iter(|| roll_effective_spread(black_box(&returns)).expect("bench: roll spread"));
    });
    group.bench_function("amihud_illiquidity_16384", |b| {
        let returns: Vec<f64> = (0..16_384)
            .map(|index| if index % 2 == 0 { 0.01 } else { -0.01 })
            .collect();
        let volumes: Vec<f64> = (0..16_384)
            .map(|index| 1_000_000.0 + index as f64)
            .collect();
        let expected = (0..16_384)
            .map(|index| 0.01 / (1_000_000.0 + index as f64))
            .sum::<f64>()
            / 16_384.0;
        let illiquidity = amihud_illiquidity(&returns, &volumes).expect("valid Amihud fixture");
        assert!(illiquidity.is_finite() && illiquidity > 0.0);
        assert!((illiquidity - expected).abs() <= expected * 1e-12);
        b.iter(|| {
            amihud_illiquidity(black_box(&returns), black_box(&volumes)).expect("bench: amihud")
        });
    });
    group.finish();
}

criterion_group!(benches, bench_liquidity_estimators);
criterion_main!(benches);
