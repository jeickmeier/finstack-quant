//! Strict bond metric and multi-instrument metric computation benchmarks.

#![allow(clippy::unwrap_used)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::Instrument;
use finstack_quant_valuations::instruments::fixed_income::bond::Bond;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::results::ValuationResult;
mod support;
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
    let notional = Money::new(1_000_000.0, Currency::USD).expect("valid money fixture");
    let coupon_rate = 0.05;
    let maturity_date = base_date + time::Duration::days(365 * 5);

    Bond::fixed(
        "BOND-5Y",
        notional,
        finstack_quant_core::types::Rate::from_decimal(coupon_rate).expect("valid rate fixture"),
        base_date,
        maturity_date,
        finstack_quant_core::dates::StubKind::ShortFront,
        "USD-OIS",
    )
    .expect("Bond::fixed should succeed with valid parameters")
}

fn validate_metrics(result: &ValuationResult, metrics: &[MetricId]) {
    assert_eq!(result.value.currency(), Currency::USD);
    assert!(result.value.amount().is_finite() && result.value.amount() > 0.0);
    assert!(result.measures.values().all(|value| value.is_finite()));
    for metric in metrics {
        assert!(result.measures.contains_key(metric), "missing {metric}");
    }
}

/// Benchmark metric computation with standard bond metrics
///
/// Note: The instrument's `price_with_metrics` method internally uses
/// the metrics registry. In Phase 1.2, we made strict mode the default.
fn bench_metrics_bond_standard(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

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
        let market = create_benchmark_market();
        let bond = create_benchmark_bond(base_date);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&market),
                    black_box(base_date),
                    black_box(&metric_ids),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("standard bond metrics")
            },
            |result| validate_metrics(result, &metric_ids),
        );
    });
}

/// Benchmark subset of metrics (pricing only)
fn bench_metrics_pricing_only(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

    // Just pricing metrics (no greeks/risk)
    let metric_ids = vec![
        MetricId::CleanPrice,
        MetricId::DirtyPrice,
        MetricId::Accrued,
    ];

    c.bench_function("metrics_bond_3_pricing_metrics", |b| {
        let market = create_benchmark_market();
        let bond = create_benchmark_bond(base_date);
        support::bench(
            b,
            || {
                bond.price_with_metrics(
                    black_box(&market),
                    black_box(base_date),
                    black_box(&metric_ids),
                    finstack_quant_valuations::instruments::PricingOptions::default(),
                )
                .expect("bond pricing metrics")
            },
            |result| validate_metrics(result, &metric_ids),
        );
    });
}

/// Benchmark metric computation with varying number of metrics
fn bench_metrics_scaling(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();

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
                let market = create_benchmark_market();
                let bond = create_benchmark_bond(base_date);
                assert_eq!(ids.len(), num_metrics);
                support::bench(
                    b,
                    || {
                        bond.price_with_metrics(
                            black_box(&market),
                            black_box(base_date),
                            black_box(ids),
                            finstack_quant_valuations::instruments::PricingOptions::default(),
                        )
                        .expect("bond metric scaling")
                    },
                    |result| validate_metrics(result, ids),
                );
            },
        );
    }

    group.finish();
}

/// Benchmark metric computation for multiple instruments (portfolio-like)
fn bench_metrics_portfolio(c: &mut Criterion) {
    let base_date = Date::from_calendar_date(2025, Month::January, 1).unwrap();
    let metric_ids = vec![MetricId::Dv01, MetricId::Convexity, MetricId::DurationMod];

    c.bench_function("metrics_portfolio_5_bonds_3_metrics", |b| {
        let market = create_benchmark_market();
        let maturities = [2, 3, 5, 7, 10];
        let bonds: Vec<Bond> = maturities
            .iter()
            .map(|years| {
                Bond::fixed(
                    format!("BOND-{}Y", years),
                    Money::new(1_000_000.0, Currency::USD).expect("valid money fixture"),
                    finstack_quant_core::types::Rate::from_decimal(0.05)
                        .expect("valid rate fixture"),
                    base_date,
                    base_date + time::Duration::days(365 * years),
                    finstack_quant_core::dates::StubKind::ShortFront,
                    "USD-OIS",
                )
                .expect("valid portfolio bond")
            })
            .collect();
        support::bench(
            b,
            || {
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
                    .expect("all portfolio bond metrics")
            },
            |results| {
                assert_eq!(results.len(), maturities.len());
                for (result, years) in results.iter().zip(maturities) {
                    assert_eq!(result.instrument_id, format!("BOND-{years}Y"));
                    validate_metrics(result, &metric_ids);
                }
            },
        );
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
