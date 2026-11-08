//! Benchmarks for cashflow NPV and discounting operations.
//!
//! Tests performance of:
//! - Curve-based NPV with Money-typed cashflows on a flat curve
//! - Curve-based NPV with Money-typed cashflows on a shaped curve
//!
//! Each selected case constructs its own fixture and checks dated USD flows
//! and the complete Money result before timing. Full NPV validation, date
//! mapping, curve queries and Decimal arithmetic remain inside timing.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::cashflow::npv;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::money::Money;
use std::hint::black_box;
use time::Month;

const SHAPED_KNOTS: [(f64, f64); 12] = [
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
];

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
        .knots(SHAPED_KNOTS)
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

fn checked_npv(curve: &DiscountCurve, flows: &[(Date, Money)], continuous_rate: Option<f64>) {
    let base = base_date();
    assert_eq!(flows.len(), 60);
    assert_eq!(curve.day_count(), DayCount::Act365F);
    assert_eq!(curve.df(0.0), 1.0);
    let mut expected = 0.0;
    for (index, &(date, amount)) in flows.iter().enumerate() {
        assert_eq!(date, base + time::Duration::days((index as i64 + 1) * 91));
        assert!(date > base);
        assert_eq!(amount.currency(), Currency::USD);
        assert_eq!(amount.amount(), 1000.0);
        let years = (date - base).whole_days() as f64 / 365.0;
        // The shaped reference retains the curve's monotone-convex query, but
        // maps dates and accumulates scalar amounts independently of npv and
        // Money arithmetic. The flat case also has an analytic DF reference.
        let df = continuous_rate.map_or_else(|| curve.df(years), |rate| (-rate * years).exp());
        assert!(df.is_finite() && df > 0.0);
        expected += amount.amount() * df;
    }
    let actual = npv(curve, base, flows).expect("valid benchmark cashflows");
    assert_eq!(actual.currency(), Currency::USD);
    assert!(actual.amount().is_finite() && expected.is_finite());
    assert!(
        (actual.amount() - expected).abs() <= 1e-8,
        "NPV {} differs from independent scalar total {expected}",
        actual.amount()
    );
}

fn bench_npv_flat_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_flat_curve");

    {
        let size = 60;
        group.bench_with_input(BenchmarkId::new("money", size), &size, |b, _| {
            let rate = 0.05_f64.ln_1p();
            let curve = flat_curve(rate);
            let flows = money_flows(size);
            checked_npv(&curve, &flows, Some(rate));
            b.iter(|| {
                let pv = npv(black_box(&curve), base_date(), black_box(&flows))
                    .expect("valid benchmark cashflows");
                black_box(pv);
            })
        });
    }

    group.finish();
}

fn bench_npv_shaped_curve(c: &mut Criterion) {
    let mut group = c.benchmark_group("npv_shaped_curve");

    {
        let size = 60;
        group.bench_with_input(BenchmarkId::new("money", size), &size, |b, _| {
            let curve = shaped_curve();
            let flows = money_flows(size);
            for (years, expected_df) in SHAPED_KNOTS {
                let actual_df = curve.df(years);
                assert!(actual_df.is_finite());
                assert!((actual_df - expected_df).abs() <= 1e-12);
            }
            checked_npv(&curve, &flows, None);
            b.iter(|| {
                let pv = npv(black_box(&curve), base_date(), black_box(&flows))
                    .expect("valid benchmark cashflows");
                black_box(pv);
            })
        });
    }

    group.finish();
}
criterion_group!(benches, bench_npv_flat_curve, bench_npv_shaped_curve);
criterion_main!(benches);
