//! Benchmarks for Brownian bridge path construction.
//!
//! `construct_path` runs once per Monte Carlo path, so its per-call cost is a
//! direct multiplier on path-dependent pricers. These kernel cases include
//! 64/256-step stress grids beyond the current 40-dimensional Sobol pricer.
//! Construction and checked preflight are excluded; each timed call retains
//! grid validation, path initialization and the complete path calculation.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::math::random::BrownianBridge;
use std::hint::black_box;

fn shocks(num_steps: usize) -> Vec<f64> {
    (0..num_steps)
        .map(|i| ((i % 17) as f64 * 0.031) - 0.25)
        .collect()
}

/// Direct recursive construction, independent of the stored bridge-step table.
fn reference_path(z: &[f64], times: &[f64]) -> Vec<f64> {
    fn fill_midpoints(
        left: usize,
        right: usize,
        z: &[f64],
        times: &[f64],
        out: &mut [f64],
        next_shock: &mut usize,
    ) {
        if right - left <= 1 {
            return;
        }
        let mid = (left + right) / 2;
        let alpha = (times[mid] - times[left]) / (times[right] - times[left]);
        let mean = out[left] + alpha * (out[right] - out[left]);
        let variance = ((times[mid] - times[left]) * (times[right] - times[mid]))
            / (times[right] - times[left]);
        out[mid] = mean + variance.sqrt() * z[*next_shock];
        *next_shock += 1;
        fill_midpoints(left, mid, z, times, out, next_shock);
        fill_midpoints(mid, right, z, times, out, next_shock);
    }

    let n = z.len();
    let mut out = vec![0.0; n + 1];
    if n != 0 {
        out[n] = z[0] * times[n].sqrt();
        let mut next_shock = 1;
        fill_midpoints(0, n, z, times, &mut out, &mut next_shock);
        assert_eq!(next_shock, n);
    }
    out
}

fn bench_construct_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("brownian_bridge_construct_path");
    for &num_steps in &[1usize, 8, 16, 32, 40, 64, 256] {
        group.bench_with_input(
            BenchmarkId::new("construct_path", num_steps),
            &num_steps,
            |b, _| {
                // Criterion selects the case before constructing its fixture.
                let bridge = BrownianBridge::new(num_steps);
                let z = shocks(num_steps);
                let times: Vec<f64> = (0..=num_steps)
                    .map(|i| i as f64 / num_steps as f64)
                    .collect();
                let mut w = vec![0.0; num_steps + 1];
                bridge
                    .construct_path(&z, &mut w, &times)
                    .expect("valid bridge preflight");
                assert_eq!(w.len(), num_steps + 1);
                assert!(w.iter().all(|value| value.is_finite()));
                let reference = reference_path(&z, &times);
                for (point, (actual, expected)) in w.iter().zip(&reference).enumerate() {
                    assert_eq!(actual.to_bits(), expected.to_bits(), "path point {point}");
                }
                b.iter(|| {
                    bridge
                        .construct_path(black_box(&z), &mut w, black_box(&times))
                        .expect("valid bridge inputs");
                    black_box(w.as_slice());
                });
            },
        );
    }
    let num_steps = 17;
    group.bench_with_input(
        BenchmarkId::new("irregular", num_steps),
        &num_steps,
        |b, &num_steps| {
            let bridge = BrownianBridge::new(num_steps);
            let z = shocks(num_steps);
            let times: Vec<f64> = (0..=num_steps)
                .map(|i| (i * i) as f64 / (num_steps * num_steps) as f64)
                .collect();
            let mut w = vec![0.0; num_steps + 1];
            bridge
                .construct_path(&z, &mut w, &times)
                .expect("valid irregular bridge preflight");
            let reference = reference_path(&z, &times);
            assert_eq!(w.len(), num_steps + 1);
            assert!(w.iter().all(|value| value.is_finite()));
            for (point, (actual, expected)) in w.iter().zip(&reference).enumerate() {
                assert_eq!(actual.to_bits(), expected.to_bits(), "path point {point}");
            }
            b.iter(|| {
                bridge
                    .construct_path(black_box(&z), &mut w, black_box(&times))
                    .expect("valid irregular bridge inputs");
                black_box(w.as_slice());
            });
        },
    );
    group.finish();
}

criterion_group!(benches, bench_construct_path);
criterion_main!(benches);
