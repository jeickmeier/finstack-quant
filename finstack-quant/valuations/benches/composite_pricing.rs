//! Scaling benchmarks for fixed-quantity composite valuation and decomposition.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{
    CompositeInstrument, CompositeLegSpec, CompositeSpec, Equity, Instrument, InstrumentJson,
    RebalanceRule, WeightingMethod,
};
use std::hint::black_box;
use time::macros::date;

fn composite_with_legs(count: usize) -> CompositeInstrument {
    let legs = (0..count)
        .map(|index| {
            let id = format!("LEG-{index}");
            let price = 75.0 + index as f64;
            CompositeLegSpec::new(
                id.clone(),
                InstrumentJson::Equity(
                    Equity::new(id.clone(), id, Currency::USD)
                        .with_shares(1.0)
                        .with_price(price),
                ),
                if index % 2 == 0 { 1.0 } else { -1.0 },
            )
        })
        .collect();
    CompositeSpec::new(
        format!("COMPOSITE-{count}"),
        Currency::USD,
        Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
        legs,
        WeightingMethod::FixedQuantity,
        RebalanceRule::Manual,
    )
    .initialize_fixed(date!(2025 - 01 - 02))
    .expect("benchmark composite must be valid")
    .instrument
}

fn benchmark_composite_pricing(c: &mut Criterion) {
    let market = MarketContext::new();
    let as_of = date!(2025 - 01 - 02);
    let mut group = c.benchmark_group("composite_fixed_quantity");
    for count in [2usize, 8, 32, 64] {
        let composite = composite_with_legs(count);
        group.bench_with_input(
            BenchmarkId::new("value", count),
            &composite,
            |b, instrument| {
                b.iter(|| {
                    black_box(
                        instrument
                            .value(black_box(&market), black_box(as_of))
                            .expect("benchmark valuation must succeed"),
                    )
                });
            },
        );
        group.bench_with_input(
            BenchmarkId::new("primitive_exposure", count),
            &composite,
            |b, instrument| {
                b.iter(|| {
                    black_box(
                        instrument
                            .primitive_exposure_report(
                                black_box(&market),
                                black_box(as_of),
                                black_box(&[]),
                            )
                            .expect("benchmark decomposition must succeed"),
                    )
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark_composite_pricing);
criterion_main!(benches);
