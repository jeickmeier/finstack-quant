//! Benchmarks for FX rate lookups against `FxMatrix`.
//!
//! Lookup cost depends on the path taken: a direct quote hits one mutex read,
//! while reciprocal and pivot-triangulated rates walk the pinned/observed
//! caches and the provider. Measures all four shapes because pricing loops
//! hit different ones per currency pair.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxMatrix, FxQuery, SimpleFxProvider};
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

fn query_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).expect("valid date")
}

fn seeded_matrix() -> FxMatrix {
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()));
    matrix
        .set_quote(Currency::EUR, Currency::USD, 1.10)
        .expect("valid quote");
    matrix
        .set_quote(Currency::GBP, Currency::USD, 1.27)
        .expect("valid quote");
    matrix
}

fn rate(matrix: &FxMatrix, from: Currency, to: Currency) -> f64 {
    matrix
        .rate(FxQuery::new(from, to, query_date(), Default::default()))
        .expect("lookup should succeed")
        .rate
}

fn bench_fx_matrix_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_matrix_rate");
    let matrix = seeded_matrix();

    // Warm direct quote (EUR/USD stored).
    group.bench_with_input(BenchmarkId::new("direct", "eur_usd"), &(), |b, _| {
        b.iter(|| rate(&matrix, black_box(Currency::EUR), black_box(Currency::USD)))
    });

    // Reciprocal of a stored quote (USD/EUR).
    group.bench_with_input(BenchmarkId::new("reciprocal", "usd_eur"), &(), |b, _| {
        b.iter(|| rate(&matrix, black_box(Currency::USD), black_box(Currency::EUR)))
    });

    // Triangulated through the USD pivot (no direct EUR/GBP quote).
    group.bench_with_input(BenchmarkId::new("triangulated", "eur_gbp"), &(), |b, _| {
        b.iter(|| rate(&matrix, black_box(Currency::EUR), black_box(Currency::GBP)))
    });

    // Pinned date/policy-scoped quote lookup.
    let on = query_date();
    matrix
        .set_quote_on(
            Currency::EUR,
            Currency::USD,
            on,
            FxConversionPolicy::CashflowDate,
            1.11,
        )
        .expect("valid pinned quote");
    group.bench_with_input(BenchmarkId::new("pinned", "eur_usd"), &(), |b, _| {
        b.iter(|| {
            matrix
                .rate(FxQuery::new(
                    black_box(Currency::EUR),
                    black_box(Currency::USD),
                    on,
                    FxConversionPolicy::CashflowDate,
                ))
                .expect("lookup should succeed")
                .rate
        })
    });

    group.finish();
}

criterion_group!(benches, bench_fx_matrix_rate);
criterion_main!(benches);
