//! Benchmarks for cashflow NPV and discounting operations.
//!
//! Tests performance of:
//! - Curve-based NPV with Money-typed cashflows (single and batch)
//! - Scalar NPV with flat discount rates
//! - Discountable trait dispatch
//! - Neumaier compensated summation at scale

#[path = "support/bench_utils.rs"]
mod bench_utils;

use bench_utils::bench_iter;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::cashflow::{npv, Discountable};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("valid bench date")
}

fn flat_curve(rate: f64) -> DiscountCurve {
    DiscountCurve::flat("BENCH-FLAT", base_date(), rate).expect("valid bench curve")
}

fn shaped_curve() -> DiscountCurve {
    let base = base_date();
    DiscountCurve::builder("USD-OIS")
        .base_date(base)
        .day_count(DayCount::Act365F)
        .knots([
            (0.0, 1.0),
            (0.25, 0.9988),
            (0.5, 0.9975),
            (1.0, 0.9512),
            (2.0, 0.9048),
            (3.0, 0.8607),
            (5.0, 0.7788),
            (7.0, 0.7047),
            (10.0, 0.6065),
            (15.0, 0.4724),
            (20.0, 0.3679),
            (30.0, 0.2231),
        ])
        .build()
        .expect("valid bench curve")
}

fn money_flows(n: usize) -> Vec<(Date, Money)> {
    let base = base_date();
    (1..=n)
        .map(|i| {
            let date = base + time::Duration::days(i as i64 * 91);
            (
                date,
                Money::new(1000.0, Currency::USD).expect("valid money fixture"),
            )
        })
        .collect()
}
fn bench_npv_flat_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_flat_curve");
    let curve = flat_curve(0.05_f64.ln_1p());

    {
        let size = 60;
        let flows = money_flows(size);
        group.bench_with_input(BenchmarkId::new("money", size), &size, |b, _| {
            b.iter(|| {
                let pv = npv(black_box(&curve), base_date(), black_box(&flows)).unwrap();
                black_box(pv);
            })
        });
    }

    group.finish();
}

fn bench_npv_shaped_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_shaped_curve");
    let curve = shaped_curve();

    {
        let size = 60;
        let flows = money_flows(size);
        group.bench_with_input(BenchmarkId::new("money", size), &size, |b, _| {
            b.iter(|| {
                let pv = npv(black_box(&curve), base_date(), black_box(&flows)).unwrap();
                black_box(pv);
            })
        });
    }

    group.finish();
}
fn bench_discountable_trait(c: &mut Criterion) {
    let mut group = c.benchmark_group("discountable_trait");
    let curve = shaped_curve();
    let flows = money_flows(60);

    bench_iter(&mut group, "via_trait", || {
        let pv = flows.npv(black_box(&curve), base_date()).unwrap();
        black_box(pv);
    });

    bench_iter(&mut group, "via_fn", || {
        let pv = npv(black_box(&curve), base_date(), &flows).unwrap();
        black_box(pv);
    });

    group.finish();
}
criterion_group!(
    benches,
    bench_npv_flat_curve,
    bench_npv_shaped_curve,
    bench_discountable_trait,
);
criterion_main!(benches);
