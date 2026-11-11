//! Checked FX lookups with isolated global, scoped, cold and warm quote states.
//! Fixture construction is untimed. The cold cross uses a fresh matrix for
//! every lookup; the warm cross retains its provider-observed result.

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::{
    FxConversionPolicy, FxMatrix, FxQuery, FxRateResult, SimpleFxProvider,
};
use std::hint::black_box;
use std::sync::Arc;
use time::Month;

fn query_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 15).expect("valid date")
}

fn seeded_matrix() -> FxMatrix {
    // A deliberate USD hub: the cross benchmarks route EUR/GBP through USD.
    let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()), Some(Currency::USD));
    matrix
        .set_quotes(&[
            (Currency::EUR, Currency::USD, 1.10),
            (Currency::GBP, Currency::USD, 1.27),
        ])
        .expect("valid global quotes");
    matrix
}

fn rate(matrix: &FxMatrix, from: Currency, to: Currency) -> FxRateResult {
    matrix
        .rate(FxQuery::new(
            from,
            to,
            query_date(),
            FxConversionPolicy::CashflowDate,
        ))
        .expect("valid benchmark FX lookup must succeed")
}

fn check_rate(result: &FxRateResult, expected: f64, triangulated: bool) {
    assert!(result.rate.is_finite() && result.rate > 0.0);
    assert!((result.rate - expected).abs() <= 1e-14);
    assert_eq!(result.triangulated, triangulated);
}

fn bench_fx_matrix_rate(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_matrix_rate");

    group.bench_with_input(BenchmarkId::new("direct", "eur_usd"), &(), |b, _| {
        let matrix = seeded_matrix();
        check_rate(&rate(&matrix, Currency::EUR, Currency::USD), 1.10, false);
        b.iter(|| {
            black_box(rate(
                &matrix,
                black_box(Currency::EUR),
                black_box(Currency::USD),
            ))
        });
    });

    group.bench_with_input(BenchmarkId::new("reciprocal", "usd_eur"), &(), |b, _| {
        let matrix = seeded_matrix();
        check_rate(
            &rate(&matrix, Currency::USD, Currency::EUR),
            1.0 / 1.10,
            false,
        );
        b.iter(|| {
            black_box(rate(
                &matrix,
                black_box(Currency::USD),
                black_box(Currency::EUR),
            ))
        });
    });

    group.bench_with_input(
        BenchmarkId::new("triangulated_warm", "eur_gbp"),
        &(),
        |b, _| {
            let matrix = seeded_matrix();
            let first = rate(&matrix, Currency::EUR, Currency::GBP);
            let warm = rate(&matrix, Currency::EUR, Currency::GBP);
            check_rate(&first, 1.10 / 1.27, true);
            check_rate(&warm, 1.10 / 1.27, true);
            assert_eq!(first.rate.to_bits(), warm.rate.to_bits());
            b.iter(|| {
                black_box(rate(
                    &matrix,
                    black_box(Currency::EUR),
                    black_box(Currency::GBP),
                ))
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("triangulated_cold_lookup", "eur_gbp"),
        &(),
        |b, _| {
            let matrix = seeded_matrix();
            let state = matrix.get_serializable_state().expect("valid FX state");
            assert_eq!(state.quotes.len(), 2);
            assert!(state.pinned_quotes.is_empty());
            assert!(state.provider_quotes.is_empty());
            check_rate(
                &rate(&matrix, Currency::EUR, Currency::GBP),
                1.10 / 1.27,
                true,
            );
            // Setup and matrix destruction are outside the measured lookup.
            // Bound live setup matrices rather than scaling their count with
            // Criterion's estimate of this short operation's iteration count.
            b.iter_batched_ref(
                seeded_matrix,
                |matrix| {
                    black_box(rate(
                        matrix,
                        black_box(Currency::EUR),
                        black_box(Currency::GBP),
                    ))
                },
                BatchSize::NumIterations(64),
            );
        },
    );

    group.bench_with_input(BenchmarkId::new("pinned", "eur_usd"), &(), |b, _| {
        // No pair-global EUR/USD quote can shadow this scoped fixing.
        let matrix = FxMatrix::new(Arc::new(SimpleFxProvider::new()), None);
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
        let state = matrix.get_serializable_state().expect("valid FX state");
        assert!(state.quotes.is_empty());
        assert_eq!(state.pinned_quotes.len(), 1);
        let query = FxQuery::new(
            Currency::EUR,
            Currency::USD,
            on,
            FxConversionPolicy::CashflowDate,
        );
        check_rate(&matrix.rate(query).expect("pinned lookup"), 1.11, false);
        assert!(
            matrix
                .rate(FxQuery::new(
                    Currency::EUR,
                    Currency::USD,
                    on.next_day().expect("next date"),
                    FxConversionPolicy::CashflowDate,
                ))
                .is_err(),
            "the fixing must remain date-scoped"
        );
        b.iter(|| {
            black_box(
                matrix
                    .rate(black_box(query))
                    .expect("valid pinned benchmark FX lookup must succeed"),
            )
        });
    });

    group.finish();
}

criterion_group!(benches, bench_fx_matrix_rate);
criterion_main!(benches);
