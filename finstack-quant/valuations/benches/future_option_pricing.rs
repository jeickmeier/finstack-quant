//! Shared listed/OTC futures-option lattice benchmarks.
//!
//! [`InterestRateFutureOption`] is the public wrapper; the hot path is
//! [`FutureOptionTerms::live_unit_price`]. European Black-76 is closed form.
//! American future delivery uses an explicitly sized tree (`O(steps²)`).
//! American fixed-date cash settlement is a separate closed-form control.
//!
//! FX, commodity, and vol-index future options share the same lattice, so one
//! representative American case covers that family.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::types::InstrumentId;
use finstack_quant_models::types::ExerciseStyle;
use finstack_quant_valuations::instruments::rates::ir_future_option::InterestRateFutureOption;
use finstack_quant_valuations::instruments::{
    FutureOptionModel, FutureOptionPremiumStyle, FutureOptionSettlement, FutureOptionTerms,
    Instrument,
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
    option.terms.settlement = FutureOptionSettlement::Future {
        underlying_last_trading_date: date!(2026 - 12 - 16),
        underlying_settlement_date: date!(2026 - 12 - 17),
        underlying_settlement_price: None,
    };
    option.instrument_pricing_overrides =
        option.instrument_pricing_overrides.with_tree_steps(steps);
    option
}

fn assert_lattice_terms(terms: &FutureOptionTerms, as_of: Date) {
    assert_eq!(terms.exercise_style, ExerciseStyle::American);
    assert!(matches!(
        terms.settlement,
        FutureOptionSettlement::Future { .. }
    ));
    assert_eq!(terms.model, FutureOptionModel::Black76);
    assert_eq!(terms.premium_style, FutureOptionPremiumStyle::PremiumPaid);
    assert_eq!(terms.currency, Currency::USD);
    assert!(as_of < terms.expiry);
    assert!(terms.exercise.is_none());
}

fn bench_future_option_european(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_european");
    group.bench_function("black76", |b| {
        let market = market();
        let as_of = as_of();
        let option = european_option();
        assert_eq!(option.terms.exercise_style, ExerciseStyle::European);
        assert_eq!(option.terms.model, FutureOptionModel::Black76);
        assert!(as_of < option.terms.expiry);
        support::bench(
            b,
            || {
                black_box(&option)
                    .value(black_box(&market), black_box(as_of))
                    .expect("European Black-76 PV")
            },
            |pv| {
                assert!(pv.amount().is_finite());
                assert_eq!(pv.currency(), Currency::USD);
            },
        );
    });
    group.finish();
}

fn bench_future_option_american(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_american");
    for steps in [201_usize, 401] {
        group.throughput(Throughput::Elements(steps as u64));
        group.bench_with_input(BenchmarkId::from_parameter(steps), &steps, |b, _| {
            let market = market();
            let as_of = as_of();
            let option = american_option(steps);
            assert_lattice_terms(&option.terms, as_of);
            assert_eq!(
                option.instrument_pricing_overrides.model_config.tree_steps,
                Some(steps)
            );
            assert_eq!(
                option
                    .instrument_pricing_overrides
                    .market_quotes
                    .implied_volatility,
                Some(0.20)
            );
            support::bench(
                b,
                || {
                    black_box(&option)
                        .value(black_box(&market), black_box(as_of))
                        .expect("American future-delivery lattice PV")
                },
                |pv| {
                    assert!(pv.amount().is_finite());
                    assert_eq!(pv.currency(), Currency::USD);
                },
            );
        });
    }
    group.finish();
}

fn bench_future_option_american_greeks(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_american_greeks");
    for steps in [201_usize, 401] {
        group.bench_function(format!("cash_delta_{steps}"), |b| {
            let market = market();
            let as_of = as_of();
            let option = american_option(steps);
            assert_lattice_terms(&option.terms, as_of);
            support::bench(
                b,
                || {
                    option
                        .terms
                        .cash_delta(
                            black_box(Some(steps)),
                            black_box(Some(0.20)),
                            black_box(&market),
                            black_box(as_of),
                        )
                        .expect("American future-delivery lattice delta")
                },
                |delta| assert!(delta.is_finite()),
            );
        });
    }
    group.finish();
}

fn bench_future_option_terms_lattice(c: &mut Criterion) {
    let mut group = c.benchmark_group("future_option_terms_lattice");
    group.throughput(Throughput::Elements(401));
    group.bench_function("american_401", |b| {
        let market = market();
        let as_of = as_of();
        let terms = american_option(401).terms;
        assert_lattice_terms(&terms, as_of);
        support::bench(
            b,
            || {
                terms
                    .npv_raw(
                        black_box(&InstrumentId::new("FOP-BENCH")),
                        black_box(Some(401)),
                        black_box(Some(0.20)),
                        black_box(&market),
                        black_box(as_of),
                    )
                    .expect("American future-delivery terms PV")
            },
            |pv| {
                assert!(pv.is_finite());
                let finer = terms
                    .npv_raw(
                        &InstrumentId::new("FOP-BENCH"),
                        Some(801),
                        Some(0.20),
                        &market,
                        as_of,
                    )
                    .expect("801-step convergence reference");
                assert!(finer.is_finite());
                // Fixed before measurement: this smooth ATM fixture must agree
                // within 0.1% when the lattice resolution is doubled.
                assert!(
                    (pv - finer).abs() <= 1.0e-3 * finer.abs().max(1.0),
                    "401/801-step lattice convergence: {pv} vs {finer}"
                );
            },
        );
    });
    group.finish();
}

fn bench_future_option_american_cash(c: &mut Criterion) {
    let mut group = c.benchmark_group("ir_future_option_american_cash");
    group.bench_function("black76_closed_form", |b| {
        let market = market();
        let as_of = as_of();
        let mut option = european_option();
        option.terms.exercise_style = ExerciseStyle::American;
        assert!(matches!(
            option.terms.settlement,
            FutureOptionSettlement::Cash { .. }
        ));
        assert_eq!(option.terms.model, FutureOptionModel::Black76);
        assert!(as_of < option.terms.expiry);
        support::bench(
            b,
            || {
                option
                    .value(black_box(&market), black_box(as_of))
                    .expect("American fixed-date cash Black-76 PV")
            },
            |pv| {
                assert!(pv.amount().is_finite());
                assert_eq!(pv.currency(), Currency::USD);
                let european = european_option()
                    .value(&market, as_of)
                    .expect("European cash control");
                assert!((pv.amount() - european.amount()).abs() < 1.0e-12);
            },
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_future_option_european,
    bench_future_option_american,
    bench_future_option_american_greeks,
    bench_future_option_terms_lattice,
    bench_future_option_american_cash,
);
criterion_main!(benches);
