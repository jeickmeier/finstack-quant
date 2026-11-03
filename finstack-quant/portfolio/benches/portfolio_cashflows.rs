//! Portfolio cashflow aggregation benchmarks.
//!
//! Measures the full cashflow ladder pipeline for realistic institutional portfolios.
//! Cashflow aggregation touches per-position schedule generation, O(E log E) event
//! sort, nested IndexMap accumulation, and (optionally) FX conversion of every
//! distinct payment date.

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{base_date, create_institutional_portfolio, create_market_context};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::types::CurveId;
use finstack_quant_portfolio::cashflows::aggregate_full_cashflows;
use std::collections::HashMap;
use std::hint::black_box;

fn collapse_discount_curves() -> HashMap<Currency, CurveId> {
    let mut curves = HashMap::new();
    curves.insert(Currency::USD, CurveId::new("USD-OIS"));
    curves.insert(Currency::EUR, CurveId::new("EUR-OIS"));
    curves.insert(Currency::GBP, CurveId::new("GBP-OIS"));
    curves.insert(Currency::JPY, CurveId::new("JPY-OIS"));
    curves
}

fn xl_benchmarks_enabled() -> bool {
    std::env::var("FINSTACK_PORTFOLIO_BENCH_XL").is_ok_and(|value| value == "1")
}

// aggregate_full_cashflows — date × currency × CFKind ladder

fn bench_aggregate_full_cashflows(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_cashflows_full");
    let market = create_market_context();
    let mut sizes = vec![63_usize, 64, 250];
    if xl_benchmarks_enabled() {
        sizes.push(3_000);
    }

    for num_positions in sizes {
        let portfolio = create_institutional_portfolio(num_positions);

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    aggregate_full_cashflows(
                        black_box(&portfolio),
                        black_box(&market),
                        &Default::default(),
                    )
                    .unwrap()
                });
            },
        );
    }
    group.finish();
}

fn bench_collapse_cashflows_to_base(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_cashflows_collapse_base");
    let market = create_market_context();
    let as_of = base_date();
    let portfolio = create_institutional_portfolio(250);
    let ladder = aggregate_full_cashflows(&portfolio, &market, &Default::default()).unwrap();
    let discount_curves = collapse_discount_curves();

    group.bench_function("collapse_to_base_250pos", |b| {
        b.iter(|| {
            ladder
                .collapse_to_base_by_date_kind(
                    black_box(&market),
                    black_box(Currency::USD),
                    black_box(as_of),
                    Some(black_box(&discount_curves)),
                )
                .unwrap()
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_aggregate_full_cashflows,
    bench_collapse_cashflows_to_base
);
criterion_main!(benches);
