//! Shared listed/OTC futures-option lattice benchmarks.
//!
//! [`InterestRateFutureOption`] is the public wrapper; the hot path is
//! [`FutureOptionTerms::live_unit_price`]. European Black-76 is closed form.
//! American exercise defaults to a 401-step tree (`O(steps²)`), which is the
//! cost this target is meant to catch.
//!
//! FX, commodity, and vol-index future options share the same lattice, so one
//! representative American case covers that family.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_valuations::instruments::{
    ExerciseStyle, FutureOptionTerms, Instrument, InterestRateFutureOption,
};
use std::hint::black_box;
use time::macros::date;

fn as_of() -> Date {
    date!(2026 - 01 - 01)
}

fn market() -> MarketContext {
    MarketContext::new().insert(
        DiscountCurve::builder("USD-OIS")
            .base_date(as_of())
            .knots([(0.0, 1.0), (2.0, 0.90)])
            .build()
            .unwrap(),
    )
}

fn european_option() -> InterestRateFutureOption {
    InterestRateFutureOption::example().unwrap()
}

fn american_option(steps: usize) -> InterestRateFutureOption {
    let mut option = InterestRateFutureOption::example().unwrap();
    option.terms.exercise_style = ExerciseStyle::American;
    option.instrument_pricing_overrides =
        option.instrument_pricing_overrides.with_tree_steps(steps);
    option
}

fn bench_future_option_european(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_european");
    let market = market();
    let as_of = as_of();
    let option = european_option();
    group.bench_function("black76", |b| {
        b.iter(|| {
            black_box(&option)
                .value(black_box(&market), black_box(as_of))
                .unwrap()
                .amount()
        });
    });
    group.finish();
}

fn bench_future_option_american(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_american");
    let market = market();
    let as_of = as_of();
    for steps in [201_usize, 401] {
        group.throughput(Throughput::Elements(steps as u64));
        let option = american_option(steps);
        group.bench_with_input(BenchmarkId::from_parameter(steps), &steps, |b, _| {
            b.iter(|| {
                black_box(&option)
                    .value(black_box(&market), black_box(as_of))
                    .unwrap()
                    .amount()
            });
        });
    }
    group.finish();
}

fn bench_future_option_american_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_american_greeks");
    let market = market();
    let as_of = as_of();
    let option = american_option(201);
    group.bench_function("cash_delta_default", |b| {
        b.iter(|| {
            option
                .terms
                .cash_delta(
                    black_box(None),
                    black_box(Some(0.20)),
                    black_box(&market),
                    black_box(as_of),
                )
                .unwrap()
        });
    });
    group.bench_function("cash_delta_401", |b| {
        b.iter(|| {
            option
                .terms
                .cash_delta(
                    black_box(Some(401)),
                    black_box(Some(0.20)),
                    black_box(&market),
                    black_box(as_of),
                )
                .unwrap()
        });
    });
    group.finish();
}

fn bench_future_option_terms_lattice(c: &mut Criterion) {
    let mut group = c.benchmark_group("future_option_terms_lattice");
    let market = market();
    let as_of = as_of();
    let mut terms = FutureOptionTerms::example().unwrap();
    terms.exercise_style = ExerciseStyle::American;
    terms.underlying_price_change_per_bp = Some(-0.01);
    group.throughput(Throughput::Elements(401));
    group.bench_function("american_401_default", |b| {
        b.iter(|| {
            terms
                .npv_raw(
                    black_box(&InstrumentId::new("FOP-BENCH")),
                    black_box(None),
                    black_box(Some(0.20)),
                    black_box(&market),
                    black_box(as_of),
                )
                .unwrap()
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_future_option_european,
    bench_future_option_american,
    bench_future_option_american_greeks,
    bench_future_option_terms_lattice,
);
criterion_main!(benches);
