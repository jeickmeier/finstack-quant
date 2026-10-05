//! Benchmarks that scale the historical position-risk tail accumulation.
//!
//! The production decomposer uses a serial sorted-tail fold. This group
//! still sweeps 400 / 500 / 600 positions so throughput stays visible as
//! `n_tail * n` grows.
//!
//! Related thresholds covered by other benchmark paths:
//!
//! * `liquidity::scoring::PARALLEL_SCORING_THRESHOLD` (512) — would need a
//!   built `Portfolio` and per-position `LiquidityProfile` fixture; defer to
//!   portfolio-scale fixture work if the threshold becomes contentious.
//! * The request-scoped evaluation executor's 64-position position-axis
//!   cutoffs are exercised by the portfolio valuation, workflow, and
//!   selective-repricing benchmark groups.
//!
//! The single bench here is intentionally narrow: it isolates the inner
//! work the threshold gates so the criterion output is dominated by the
//! work the threshold is supposed to optimise.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_models::factor::risk::{
    decompose_from_pnls, DecompositionConfig, PositionRiskDecomposition,
};

fn bench_historical_tail_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("historical_decomp_tail");
    group.sample_size(10);

    // Fix scenarios; sweep n (positions) so n_tail * n scales through 80k–120k.
    // confidence = 0.95 => n_tail = 0.05 * n_scenarios.
    let n_scenarios: usize = 4_000; // n_tail = 200
    let confidence = 0.95;

    for n_positions in [400_usize, 500, 600].iter() {
        let n = *n_positions;
        let n_tail_times_n = (n_scenarios as f64 * (1.0 - confidence)) as usize * n;
        group.throughput(Throughput::Elements(n_tail_times_n as u64));

        // Deterministic synthetic P&L matrix: row-major (n_scenarios, n).
        let total = n_scenarios * n;
        let mut pnls = Vec::with_capacity(total);
        for s in 0..n_scenarios {
            for i in 0..n {
                // Mild scenario/position interaction; finite, stable across seeds.
                let v = ((s as f64 * 0.013) - (i as f64 * 0.007)).sin() * 1_000.0;
                pnls.push(v);
            }
        }
        let ids: Vec<String> = (0..n).map(|i| format!("P{i}")).collect();
        let mut config = DecompositionConfig::historical(confidence);
        config.confidence = confidence;

        group.bench_with_input(
            BenchmarkId::new("decompose_from_pnls", format!("{}p_x_{}sc", n, n_scenarios)),
            &n,
            |b, _| {
                b.iter(|| {
                    let _: PositionRiskDecomposition =
                        decompose_from_pnls(&pnls, &ids, n_scenarios, &config)
                            .expect("bench: decomposition should succeed");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_historical_tail_threshold);
criterion_main!(benches);
