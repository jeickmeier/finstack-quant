//! Benchmarks for money types and operations.
//!
//! Tests performance of:
//! - Money creation and conversions
//! - Arithmetic operations (add, subtract, multiply, divide)
//! - Currency conversions via FX providers
//! - Formatting operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::fx::{FxConversionPolicy, FxProvider};
use finstack_quant_core::money::Money;
use std::hint::black_box;
use time::Month;

// Simple FX provider for benchmarking
struct SimpleFxProvider {
    eur_usd: f64,
    gbp_usd: f64,
    jpy_usd: f64,
}

impl FxProvider for SimpleFxProvider {
    fn rate(
        &self,
        from: Currency,
        to: Currency,
        _on: Date,
        _policy: FxConversionPolicy,
    ) -> finstack_quant_core::Result<f64> {
        if from == to {
            return Ok(1.0);
        }

        let from_to_usd = match from {
            Currency::EUR => self.eur_usd,
            Currency::GBP => self.gbp_usd,
            Currency::JPY => self.jpy_usd,
            _ => 1.0,
        };

        let to_from_usd = match to {
            Currency::EUR => 1.0 / self.eur_usd,
            Currency::GBP => 1.0 / self.gbp_usd,
            Currency::JPY => 1.0 / self.jpy_usd,
            _ => 1.0,
        };

        Ok(from_to_usd * to_from_usd)
    }
}

fn bench_money_creation(c: &mut Criterion) {
    c.bench_function("money_creation", |b| {
        b.iter(|| {
            let m = Money::new(black_box(1_000_000.50), Currency::USD);
            black_box(m);
        })
    });
}

fn bench_money_arithmetic(c: &mut Criterion) {
    let m1 = Money::new(1_000_000.0, Currency::USD);
    let m2 = Money::new(50_000.0, Currency::USD);

    c.bench_function("money_add", |b| {
        b.iter(|| {
            let result = black_box(m1).checked_add(black_box(m2)).unwrap();
            black_box(result);
        })
    });

    c.bench_function("money_sub", |b| {
        b.iter(|| {
            let result = black_box(m1).checked_sub(black_box(m2)).unwrap();
            black_box(result);
        })
    });

    c.bench_function("money_mul_scalar", |b| {
        b.iter(|| {
            let result = black_box(m1) * black_box(1.05);
            black_box(result);
        })
    });

    c.bench_function("money_div_scalar", |b| {
        b.iter(|| {
            let result = black_box(m1) / black_box(2.0);
            black_box(result);
        })
    });
}

fn bench_money_conversions(c: &mut Criterion) {
    let provider = SimpleFxProvider {
        eur_usd: 1.1,
        gbp_usd: 1.3,
        jpy_usd: 0.0067,
    };

    let date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    let conversions = [
        ("EUR_to_USD", Currency::EUR, Currency::USD),
        ("USD_to_EUR", Currency::USD, Currency::EUR),
        ("GBP_to_USD", Currency::GBP, Currency::USD),
        ("USD_to_JPY", Currency::USD, Currency::JPY),
        ("EUR_to_GBP", Currency::EUR, Currency::GBP),
    ];

    for (name, from_ccy, to_ccy) in conversions {
        c.bench_function(&format!("money_convert_{}", name), |b| {
            let amount = Money::new(1_000_000.0, from_ccy);
            b.iter(|| {
                let result = black_box(amount)
                    .convert(to_ccy, date, &provider, FxConversionPolicy::CashflowDate)
                    .unwrap();
                black_box(result);
            })
        });
    }
}

fn bench_money_batch_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("money_batch_operations");

    for size in [10, 50, 100, 500, 1000] {
        group.bench_with_input(BenchmarkId::new("sum", size), &size, |b, &size| {
            let amounts: Vec<_> = (0..size)
                .map(|i| Money::new((i as f64) * 1000.0, Currency::USD))
                .collect();
            b.iter(|| {
                let mut total = Money::new(0.0, Currency::USD);
                for amt in &amounts {
                    total = total.checked_add(*amt).unwrap();
                }
                black_box(total);
            })
        });

        group.bench_with_input(BenchmarkId::new("scale", size), &size, |b, &size| {
            let amounts: Vec<_> = (0..size)
                .map(|i| Money::new((i as f64) * 1000.0, Currency::USD))
                .collect();
            b.iter(|| {
                let scaled: Vec<_> = amounts.iter().map(|&amt| amt * 1.05).collect();
                black_box(scaled);
            })
        });
    }

    group.finish();
}

fn bench_money_formatting(c: &mut Criterion) {
    let amounts = [
        ("small", Money::new(12.34, Currency::USD)),
        ("medium", Money::new(1_234.56, Currency::USD)),
        ("large", Money::new(1_234_567.89, Currency::USD)),
    ];

    for (name, amount) in amounts {
        c.bench_function(&format!("money_format_{}", name), |b| {
            b.iter(|| {
                let s = format!("{}", black_box(amount));
                black_box(s);
            })
        });
    }
}

criterion_group!(
    benches,
    bench_money_creation,
    bench_money_arithmetic,
    bench_money_conversions,
    bench_money_batch_operations,
    bench_money_formatting,
);
criterion_main!(benches);
