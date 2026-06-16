//! Benchmarks that span the parallel-vs-serial cutoff in the historical
//! position-risk decomposer.
//!
//! Threshold covered:
//!
//! * `factor_model::position_risk::PARALLEL_TAIL_THRESHOLD` (100_000) — the
//!   `n_tail * n` cutoff that switches the tail-component-ES accumulation
//!   from a serial loop to a position-axis-sharded Rayon fan-out.
//!
//! Related thresholds covered by other benchmark paths:
//!
//! * `liquidity::scoring::PARALLEL_SCORING_THRESHOLD` (512) — would need a
//!   built `Portfolio` and per-position `LiquidityProfile` fixture; defer to
//!   `portfolio_metrics`-style bench infrastructure.
//! * `valuation::REVALUE_AFFECTED_PARALLEL_MIN_AFFECTED` (64) — already
//!   exercised indirectly by `portfolio_valuation` benches with a varying
//!   `affected_indices` slice; would only need a dedicated bench if the
//!   threshold itself becomes contentious.
//!
//! The single bench here is intentionally narrow: it isolates the inner
//! work the threshold gates so the criterion output is dominated by the
//! work the threshold is supposed to optimise.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_portfolio::factor_model::{
    DecompositionConfig, HistoricalPositionDecomposer, PositionRiskDecomposition,
};
use finstack_quant_portfolio::types::PositionId;

// ---------------------------------------------------------------------------
// Historical position-risk decomposer: PARALLEL_TAIL_THRESHOLD = 100_000
// ---------------------------------------------------------------------------

fn bench_historical_tail_threshold(c: &mut Criterion) {
    let mut group = c.benchmark_group("historical_decomp_tail");
    group.sample_size(10);

    // Fix scenarios; sweep n (positions) so n_tail * n straddles 100_000.
    // confidence = 0.95 => n_tail = 0.05 * n_scenarios.
    let n_scenarios: usize = 4_000; // n_tail = 200
    let confidence = 0.95;

    for n_positions in [50_usize, 250, 500, 1_000].iter() {
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
        let ids: Vec<PositionId> = (0..n).map(|i| PositionId::new(format!("P{i}"))).collect();
        let mut config = DecompositionConfig::historical(confidence);
        config.confidence = confidence;
        let decomposer = HistoricalPositionDecomposer;

        group.bench_with_input(
            BenchmarkId::new("decompose_from_pnls", format!("{}p_x_{}sc", n, n_scenarios)),
            &n,
            |b, _| {
                b.iter(|| {
                    let _: PositionRiskDecomposition = decomposer
                        .decompose_from_pnls(&pnls, &ids, n_scenarios, &config)
                        .expect("bench: decomposition should succeed");
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_historical_tail_threshold);
criterion_main!(benches);
