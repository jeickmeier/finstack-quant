//! Portfolio cashflow aggregation benchmarks.
//!
//! Measures the full cashflow ladder pipeline for realistic institutional portfolios.
//! Cashflow aggregation touches per-position schedule generation, O(E log E) event
//! sort, nested IndexMap accumulation, and (optionally) FX conversion of every
//! distinct payment date.

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{create_institutional_portfolio, create_market_context};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_portfolio::cashflows::aggregate_full_cashflows;
use std::hint::black_box;

// ============================================================================
// aggregate_full_cashflows — date × currency × CFKind ladder
// ============================================================================

fn bench_aggregate_full_cashflows(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_cashflows_full");
    let market = create_market_context();

    {
        let num_positions = 250usize;
        let portfolio = create_institutional_portfolio(num_positions);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    aggregate_full_cashflows(black_box(&portfolio), black_box(&market)).unwrap()
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_aggregate_full_cashflows,);
criterion_main!(benches);
