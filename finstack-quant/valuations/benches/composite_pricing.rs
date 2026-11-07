//! Scaling benchmarks for fixed-quantity composite valuation and decomposition.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::composite::{
    CompositeInstrument, CompositeLegSpec, CompositeSpec, RebalanceRule, WeightingMethod,
};
use finstack_quant_valuations::instruments::equity::spot::Equity;
use finstack_quant_valuations::instruments::json_loader::InstrumentJson;
use finstack_quant_valuations::instruments::{Instrument, PricingOptions};
use std::hint::black_box;
use time::macros::date;
mod support;

fn composite_with_legs(count: usize) -> CompositeInstrument {
    let legs = (0..count)
        .map(|index| {
            let id = format!("LEG-{index}");
            let price = 75.0 + index as f64;
            CompositeLegSpec::new(
                id.clone(),
                InstrumentJson::Equity(
                    Equity::new(id.clone(), id, Currency::USD)
                        .with_quantity(1.0)
                        .with_quoted_spot(price),
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
    let as_of = date!(2025 - 01 - 02);
    let mut group = c.benchmark_group("composite_fixed_quantity");
    for count in [2usize, 8, 32, 64] {
        group.bench_with_input(BenchmarkId::new("value", count), &count, |b, &count| {
            let market = MarketContext::new();
            let instrument = composite_with_legs(count);
            support::bench(
                b,
                || {
                    instrument
                        .value(black_box(&market), black_box(as_of))
                        .expect("benchmark valuation must succeed")
                },
                |value| {
                    assert_eq!(value.currency(), Currency::USD);
                    assert_eq!(value.amount(), -(count as f64) / 2.0);
                },
            );
        });
        group.bench_with_input(
            BenchmarkId::new("primitive_exposure", count),
            &count,
            |b, &count| {
                let market = MarketContext::new();
                let instrument = composite_with_legs(count);
                support::bench(
                    b,
                    || {
                        instrument
                            .primitive_exposures(
                                black_box(&market),
                                black_box(as_of),
                                black_box(&[]),
                                PricingOptions::default(),
                            )
                            .expect("benchmark decomposition must succeed")
                    },
                    |report| {
                        assert_eq!(report.reporting_currency, Currency::USD);
                        assert_eq!(report.paths.len(), count);
                        assert_eq!(report.aggregates.len(), count);
                        for (index, path) in report.paths.iter().enumerate() {
                            let id = format!("LEG-{index}");
                            let quantity = if index % 2 == 0 { 1.0 } else { -1.0 };
                            assert_eq!(path.instrument_id.as_str(), id);
                            assert_eq!(path.quantity, quantity);
                            assert_eq!(path.value.currency(), Currency::USD);
                            assert_eq!(path.value.amount(), quantity * (75.0 + index as f64));
                            assert!(path.instrument.is_some());
                            assert!(path.measures.values().all(|value| value.is_finite()));
                        }
                        for aggregate in &report.aggregates {
                            assert_eq!(aggregate.net_quantity.abs(), 1.0);
                            assert_eq!(aggregate.gross_quantity, 1.0);
                            assert_eq!(aggregate.net_value.currency(), Currency::USD);
                            assert_eq!(aggregate.gross_value.currency(), Currency::USD);
                            assert!(aggregate.net_value.amount().is_finite());
                            assert_eq!(
                                aggregate.gross_value.amount(),
                                aggregate.net_value.amount().abs()
                            );
                        }
                        let net_value: f64 = report
                            .aggregates
                            .iter()
                            .map(|aggregate| aggregate.net_value.amount())
                            .sum();
                        assert_eq!(net_value, -(count as f64) / 2.0);
                    },
                );
            },
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark_composite_pricing);
criterion_main!(benches);
