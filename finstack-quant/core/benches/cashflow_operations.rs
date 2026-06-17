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
use finstack_quant_core::cashflow::{npv, npv_amounts, Discountable};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::{DiscountCurve, FlatCurve};
use finstack_quant_core::money::Money;
use std::hint::black_box;
use time::Month;

fn base_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).expect("valid bench date")
}

fn flat_curve(rate: f64) -> FlatCurve {
    FlatCurve::new(rate, base_date(), DayCount::Act365F, "BENCH-FLAT")
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
            (date, Money::new(1000.0, Currency::USD))
        })
        .collect()
}

fn scalar_flows(n: usize) -> Vec<(Date, f64)> {
    let base = base_date();
    (1..=n)
        .map(|i| {
            let date = base + time::Duration::days(i as i64 * 91);
            (date, 1000.0)
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
                let pv = npv(black_box(&curve), base_date(), None, black_box(&flows)).unwrap();
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
                let pv = npv(black_box(&curve), base_date(), None, black_box(&flows)).unwrap();
                black_box(pv);
            })
        });
    }

    group.finish();
}

fn bench_npv_amounts(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_amounts_scalar");

    {
        let size = 60;
        let flows = scalar_flows(size);
        group.bench_with_input(BenchmarkId::new("scalar", size), &size, |b, _| {
            b.iter(|| {
                let pv = npv_amounts(
                    black_box(&flows),
                    black_box(0.05),
                    Some(base_date()),
                    Some(DayCount::Act365F),
                )
                .unwrap();
                black_box(pv);
            })
        });
    }

    group.finish();
}

fn bench_npv_day_count_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_day_count_variants");
    let curve = flat_curve(0.05_f64.ln_1p());
    let flows = money_flows(60);

    let day_counts = [
        ("Act365F", DayCount::Act365F),
        ("Act360", DayCount::Act360),
        ("Thirty360", DayCount::Thirty360),
    ];

    for (name, dc) in day_counts {
        bench_iter(&mut group, name, || {
            let pv = npv(&curve, base_date(), Some(dc), &flows).unwrap();
            black_box(pv);
        });
    }

    group.finish();
}

fn bench_discountable_trait(c: &mut Criterion) {
    let mut group = c.benchmark_group("discountable_trait");
    let curve = shaped_curve();
    let flows = money_flows(60);

    bench_iter(&mut group, "via_trait", || {
        let pv = flows.npv(black_box(&curve), base_date(), None).unwrap();
        black_box(pv);
    });

    bench_iter(&mut group, "via_fn", || {
        let pv = npv(black_box(&curve), base_date(), None, &flows).unwrap();
        black_box(pv);
    });

    group.finish();
}

fn bench_npv_investment_profile(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_investment_profile");
    let base = base_date();

    let bond_flows: Vec<(Date, f64)> = {
        let mut flows = vec![(base, -100_000.0)];
        for i in 1..=20 {
            flows.push((base + time::Duration::days(i * 182), 2_500.0));
        }
        let last = flows.last().unwrap().0;
        flows.push((last, 100_000.0));
        flows
    };

    bench_iter(&mut group, "bond_20_coupons", || {
        let pv = npv_amounts(&bond_flows, 0.04, Some(base), Some(DayCount::Act365F)).unwrap();
        black_box(pv);
    });

    let swap_flows: Vec<(Date, f64)> = (1..=40)
        .map(|i| {
            let date = base + time::Duration::days(i * 91);
            let amount = if i % 2 == 0 { 500.0 } else { -480.0 };
            (date, amount)
        })
        .collect();

    bench_iter(&mut group, "swap_40_netted", || {
        let pv = npv_amounts(&swap_flows, 0.03, Some(base), Some(DayCount::Act360)).unwrap();
        black_box(pv);
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_npv_flat_curve,
    bench_npv_shaped_curve,
    bench_npv_amounts,
    bench_npv_day_count_comparison,
    bench_discountable_trait,
    bench_npv_investment_profile,
);
criterion_main!(benches);
