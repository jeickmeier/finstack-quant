//! Metrics computation benchmarks (Phase 1).
//!
//! This benchmark suite tests the performance of strict vs best-effort
//! metric computation modes introduced in Phase 1.2.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;
use time::Month;

/// Create a simple market with discount curve for benchmarking
fn create_benchmark_market() -> MarketContext {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Create discount curve with builder pattern
    let discount_curve = DiscountCurve::builder("USD-OIS")
        .base_date(base_date)
        .knots([
            (0.0, 1.0),
            (0.08, 0.9985), // ~30 days
            (0.25, 0.9950), // ~90 days
            (0.5, 0.9890),  // ~180 days
            (1.0, 0.9750),
            (2.0, 0.9500),
            (3.0, 0.9250),
            (5.0, 0.8800),
            (7.0, 0.8400),
            (10.0, 0.7800),
        ])
        .interp(InterpStyle::LogLinear)
        .build()
        .unwrap();

    MarketContext::new().insert(discount_curve)
}

/// Create a bond instrument for benchmarking
fn create_benchmark_bond(base_date: Date) -> Bond {
    let notional = Money::new(1_000_000.0, Currency::USD);
    let coupon_rate = 0.05;
    let maturity_date = base_date + time::Duration::days(365 * 5);

    Bond::fixed(
        "BOND-5Y",
        notional,
        coupon_rate,
        base_date,
        maturity_date,
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters")
}

/// Benchmark metric computation with standard bond metrics
///
/// Note: The instrument's `price_with_metrics` method internally uses
/// the metrics registry. In Phase 1.2, we made strict mode the default.
fn bench_metrics_bond_standard(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_benchmark_market();
    let bond = create_benchmark_bond(base_date);

    // Standard bond metrics
    let metric_ids = vec![
        MetricId::Dv01,
        MetricId::Convexity,
        MetricId::DurationMod,
        MetricId::DurationMac,
        MetricId::Ytm,
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::Theta,
    ];

    c.bench_function("metrics_bond_9_standard_metrics", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&market),
                black_box(base_date),
                black_box(&metric_ids),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        })
    });
}

/// Benchmark subset of metrics (pricing only)
fn bench_metrics_pricing_only(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_benchmark_market();
    let bond = create_benchmark_bond(base_date);

    // Just pricing metrics (no greeks/risk)
    let metric_ids = vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
    ];

    c.bench_function("metrics_bond_3_pricing_metrics", |b| {
        b.iter(|| {
            bond.price_with_metrics(
                black_box(&market),
                black_box(base_date),
                black_box(&metric_ids),
                finstack_quant_valuations::instruments::PricingOptions::default(),
            )
        })
    });
}

/// Benchmark metric computation with varying number of metrics
fn bench_metrics_scaling(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_benchmark_market();
    let bond = create_benchmark_bond(base_date);

    // Test with different numbers of metrics
    let all_metrics = vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
        MetricId::Ytm,
        MetricId::DurationMod,
        MetricId::DurationMac,
        MetricId::Convexity,
        MetricId::Dv01,
        MetricId::Theta,
        MetricId::ZSpread,
    ];

    let mut group = c.benchmark_group("metrics_scaling");

    {
        let num_metrics = 5;
        let metric_ids: Vec<MetricId> = all_metrics.iter().take(num_metrics).cloned().collect();

        group.bench_with_input(
            BenchmarkId::new("metrics", num_metrics),
            &metric_ids,
            |b, ids| {
                b.iter(|| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(base_date),
                        black_box(ids),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                })
            },
        );
    }

    group.finish();
}

/// Benchmark metric computation for multiple instruments (portfolio-like)
fn bench_metrics_portfolio(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let market = create_benchmark_market();

    // Create a portfolio of bonds with different maturities
    let maturities = [2, 3, 5, 7, 10]; // years
    let bonds: Vec<Bond> = maturities
        .iter()
        .map(|years| {
            let notional = Money::new(1_000_000.0, Currency::USD);
            let maturity_date = base_date + time::Duration::days(365 * years);
            Bond::fixed(
                format!("BOND-{}Y", years),
                notional,
                0.05,
                base_date,
                maturity_date,
                "USD-OIS",
            )
            .expect("Bond::fixed should succeed with valid parameters")
        })
        .collect();

    let metric_ids = vec![MetricId::Dv01, MetricId::Convexity, MetricId::DurationMod];

    c.bench_function("metrics_portfolio_5_bonds_3_metrics", |b| {
        b.iter(|| {
            bonds
                .iter()
                .map(|bond| {
                    bond.price_with_metrics(
                        black_box(&market),
                        black_box(base_date),
                        black_box(&metric_ids),
                        finstack_quant_valuations::instruments::PricingOptions::default(),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
    });
}

criterion_group!(
    benches,
    bench_metrics_bond_standard,
    bench_metrics_pricing_only,
    bench_metrics_scaling,
    bench_metrics_portfolio
);
criterion_main!(benches);
