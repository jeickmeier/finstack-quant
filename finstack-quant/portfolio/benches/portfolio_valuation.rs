//! Portfolio valuation benchmarks.
//!
//! Measures performance of portfolio operations for large institutional portfolios:
//! - Mixed-instrument portfolio valuation
//! - Scaling with portfolio size
//! - Multi-currency aggregation
//! - Entity-level aggregation
//! - Attribute grouping and filtering
//!
//! Simulates realistic institutional portfolios with:
//! - Multiple entities (funds, accounts)
//! - The instrument mix documented in `bench_common.rs`
//! - Cross-currency positions
//! - Various position sizes

#[path = "bench_common.rs"]
mod bench_common;

#[path = "support/validity.rs"]
mod validity;

use bench_common::{base_date, create_institutional_portfolio, create_market_context, maturity_2y};
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::DayCount;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::Entity;
use finstack_quant_portfolio::valuation::{
    PortfolioValuationOptions, RequestedMetrics, revalue_affected, value_portfolio,
};
use finstack_quant_portfolio::{Portfolio, dependencies::MarketFactorKey};
use finstack_quant_valuations::instruments::RatesCurveKind;
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::metrics::MetricId;
use rust_decimal_macros::dec;
use std::hint::black_box;
use std::sync::Arc;

fn xl_benchmarks_enabled() -> bool {
    std::env::var("FINSTACK_PORTFOLIO_BENCH_XL").is_ok_and(|value| value == "1")
}

/// Uniform-cost fixture used to isolate selective invalidation and scheduling.
///
/// The headline valuation and attribution groups remain mixed/credit-bearing.
/// This fixture intentionally assigns exact dependency bands so the benchmark
/// names correspond to measured 3%, 25%, 50%, and 100% dirty sets.
fn create_selective_benchmark_portfolio(
    num_positions: usize,
) -> finstack_quant_portfolio::Portfolio {
    let sparse_end = num_positions * 3 / 100;
    let quarter_end = num_positions / 4;
    let half_end = num_positions / 2;
    let mut builder = Portfolio::builder("SELECTIVE_BENCHMARK")
        .base_currency(Currency::USD)
        .as_of(base_date())
        .entity(Entity::new("SELECTIVE_ENTITY"));

    for index in 0..num_positions {
        let (curve_id, currency) = if index < sparse_end {
            ("SELECTIVE_SPARSE", Currency::USD)
        } else if index < quarter_end {
            ("SELECTIVE_TO_25", Currency::USD)
        } else if index < half_end {
            ("SELECTIVE_TO_50", Currency::USD)
        } else {
            ("SELECTIVE_TO_100", Currency::EUR)
        };
        let instrument_id = format!("SELECTIVE_DEPOSIT_{index:05}");
        let deposit = Deposit::builder()
            .id(instrument_id.clone().into())
            .notional(Money::new(100_000.0, currency).expect("valid money fixture"))
            .start_date(base_date())
            .maturity(maturity_2y())
            .day_count(DayCount::Act360)
            .discount_curve_id(curve_id.into())
            .fixed_rate_opt(Some(dec!(0.04)))
            .build()
            .unwrap();
        builder = builder.position(
            Position::new(
                format!("SELECTIVE_POSITION_{index:05}"),
                "SELECTIVE_ENTITY",
                instrument_id,
                Arc::new(deposit),
                1.0,
                PositionUnit::Units,
            )
            .unwrap(),
        );
    }
    builder.build().unwrap()
}

fn create_selective_benchmark_market() -> MarketContext {
    [
        "SELECTIVE_SPARSE",
        "SELECTIVE_TO_25",
        "SELECTIVE_TO_50",
        "SELECTIVE_TO_100",
    ]
    .into_iter()
    .fold(create_market_context(), |market, curve_id| {
        market.insert(
            DiscountCurve::builder(curve_id)
                .base_date(base_date())
                .knots([(0.0, 1.0), (1.0, 0.96), (5.0, 0.80)])
                .interp(InterpStyle::Linear)
                .build()
                .unwrap(),
        )
    })
}

// Portfolio Valuation Benchmarks

fn bench_portfolio_valuation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_valuation");
    group.bench_with_input(
        BenchmarkId::from_parameter("250pos"),
        &250usize,
        |b, &count| {
            let market = create_market_context();
            let config = FinstackConfig::default();
            let options = PortfolioValuationOptions::default();
            let portfolio = create_institutional_portfolio(count);
            validity::bench(
                b,
                || {
                    value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &options,
                    )
                    .expect("complete standard-risk portfolio valuation")
                },
                |result| validity::valuation(&portfolio, result, &[MetricId::Dv01]),
            );
        },
    );
    group.finish();
}

// Entity Aggregation Benchmarks

fn bench_entity_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_entity_aggregation");
    group.bench_with_input(
        BenchmarkId::from_parameter("250pos"),
        &250usize,
        |b, &count| {
            let market = create_market_context();
            let config = FinstackConfig::default();
            let options = PortfolioValuationOptions::default();
            let portfolio = create_institutional_portfolio(count);
            validity::bench(
                b,
                || {
                    let valuation = value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &options,
                    )
                    .expect("complete standard-risk entity valuation");
                    for i in 1..=5 {
                        black_box(
                            valuation
                                .get_entity_value(&format!("FUND_{i}"))
                                .expect("benchmark entity aggregate"),
                        );
                    }
                    valuation
                },
                |result| {
                    validity::valuation(&portfolio, result, &[MetricId::Dv01]);
                    assert_eq!(result.by_entity.len(), 5);
                },
            );
        },
    );
    group.finish();
}

// Multi-Currency Aggregation Benchmarks

fn bench_multicurrency_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_multicurrency");
    group.bench_function("100pos_multicurrency", |b| {
        let market = create_market_context();
        let config = FinstackConfig::default();
        let options = PortfolioValuationOptions::default();
        let portfolio = create_institutional_portfolio(100);
        validity::bench(
            b,
            || {
                value_portfolio(
                    black_box(&portfolio),
                    black_box(&market),
                    black_box(&config),
                    &options,
                )
                .expect("complete multicurrency standard-risk valuation")
            },
            |result| {
                validity::valuation(&portfolio, result, &[MetricId::Dv01]);
                assert!(
                    result
                        .position_values
                        .values()
                        .any(|value| value.fx_rate.is_some())
                );
            },
        );
    });
    group.finish();
}

// Position Filtering Benchmarks

fn bench_position_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_filtering");
    group.bench_function("filter_by_entity", |b| {
        let portfolio = create_institutional_portfolio(250);
        let expected = portfolio
            .positions()
            .iter()
            .filter(|position| position.entity_id.as_str() == "FUND_1")
            .count();
        assert!(expected > 0);
        validity::bench(
            b,
            || {
                portfolio
                    .positions()
                    .iter()
                    .filter(|position| position.entity_id.as_str() == black_box("FUND_1"))
                    .collect::<Vec<_>>()
            },
            |matched| assert_eq!(matched.len(), expected),
        );
    });
    group.bench_function("position_count", |b| {
        let portfolio = create_institutional_portfolio(250);
        validity::bench(
            b,
            || black_box(&portfolio).positions().len(),
            |count| assert_eq!(*count, 250),
        );
    });
    group.finish();
}

// Scaling Benchmarks

fn bench_portfolio_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_pv_scaling");
    let mut position_counts = vec![63usize, 64, 250, 3_000];
    if xl_benchmarks_enabled() {
        position_counts.push(25_000);
    }
    for num_positions in position_counts {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{num_positions}pos")),
            &num_positions,
            |b, &count| {
                let market = create_market_context();
                let config = FinstackConfig::default();
                let options = PortfolioValuationOptions {
                    strict_risk: true,
                    metrics: RequestedMetrics::Only(Vec::new()),
                };
                let portfolio = create_institutional_portfolio(count);
                validity::bench(
                    b,
                    || {
                        value_portfolio(
                            black_box(&portfolio),
                            black_box(&market),
                            black_box(&config),
                            &options,
                        )
                        .expect("successful canonical portfolio PV")
                    },
                    |result| validity::valuation(&portfolio, result, &[]),
                );
            },
        );
    }
    group.finish();
}

fn bench_selective_repricing_shapes(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_selective_repricing_3000");
    let cases = [
        (
            "dirty_03pct",
            vec![MarketFactorKey::curve(
                "SELECTIVE_SPARSE".into(),
                RatesCurveKind::Discount,
            )],
        ),
        (
            "dirty_25pct",
            vec![
                MarketFactorKey::curve("SELECTIVE_SPARSE".into(), RatesCurveKind::Discount),
                MarketFactorKey::curve("SELECTIVE_TO_25".into(), RatesCurveKind::Discount),
            ],
        ),
        (
            "dirty_50pct",
            vec![
                MarketFactorKey::curve("SELECTIVE_SPARSE".into(), RatesCurveKind::Discount),
                MarketFactorKey::curve("SELECTIVE_TO_25".into(), RatesCurveKind::Discount),
                MarketFactorKey::curve("SELECTIVE_TO_50".into(), RatesCurveKind::Discount),
            ],
        ),
        (
            "dirty_100pct",
            vec![
                MarketFactorKey::curve("SELECTIVE_SPARSE".into(), RatesCurveKind::Discount),
                MarketFactorKey::curve("SELECTIVE_TO_25".into(), RatesCurveKind::Discount),
                MarketFactorKey::curve("SELECTIVE_TO_50".into(), RatesCurveKind::Discount),
                MarketFactorKey::curve("SELECTIVE_TO_100".into(), RatesCurveKind::Discount),
            ],
        ),
        (
            "fx_conversion_refresh",
            vec![MarketFactorKey::fx(Currency::EUR, Currency::USD)],
        ),
    ];
    group.bench_function("full_control", |b| {
        let market = create_selective_benchmark_market();
        let portfolio = create_selective_benchmark_portfolio(3_000);
        let config = FinstackConfig::default();
        let options = PortfolioValuationOptions {
            strict_risk: true,
            metrics: RequestedMetrics::Only(Vec::new()),
        };
        validity::bench(
            b,
            || {
                value_portfolio(
                    black_box(&portfolio),
                    black_box(&market),
                    black_box(&config),
                    &options,
                )
                .expect("full selective control")
            },
            |result| validity::valuation(&portfolio, result, &[]),
        );
    });
    for (label, changed) in &cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(*label),
            changed,
            |b, changed| {
                let market = create_selective_benchmark_market();
                let portfolio = create_selective_benchmark_portfolio(3_000);
                let config = FinstackConfig::default();
                let options = PortfolioValuationOptions {
                    strict_risk: true,
                    metrics: RequestedMetrics::Only(Vec::new()),
                };
                let prior = value_portfolio(&portfolio, &market, &config, &options)
                    .expect("selective prior");
                validity::valuation(&portfolio, &prior, &[]);
                validity::bench(
                    b,
                    || {
                        revalue_affected(
                            black_box(&portfolio),
                            black_box(&market),
                            black_box(&config),
                            &options,
                            black_box(&prior),
                            black_box(changed),
                        )
                        .expect("selective shape valuation")
                    },
                    |result| validity::valuation(&portfolio, result, &[]),
                );
            },
        );
    }
    group.finish();
}

// Selective Repricing Benchmarks

/// Measure broad rate, equity-spot and hazard-curve invalidation on the mixed book.
fn bench_revalue_affected(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_revalue_affected");
    let scenarios = [
        (
            "broad_curve",
            vec![MarketFactorKey::curve(
                "USD-OIS".into(),
                RatesCurveKind::Discount,
            )],
        ),
        ("equity_spot", vec![MarketFactorKey::spot("AAPL")]),
        (
            "hazard_curve",
            vec![MarketFactorKey::curve(
                "CORP-HAZARD".into(),
                RatesCurveKind::Credit,
            )],
        ),
    ];
    for (label, changed) in &scenarios {
        group.bench_with_input(BenchmarkId::new("250pos", label), changed, |b, changed| {
            let market = create_market_context();
            let config = FinstackConfig::default();
            let options = PortfolioValuationOptions::default();
            let portfolio = create_institutional_portfolio(250);
            let prior = value_portfolio(&portfolio, &market, &config, &options)
                .expect("complete standard-risk prior");
            validity::valuation(&portfolio, &prior, &[MetricId::Dv01]);
            validity::bench(
                b,
                || {
                    revalue_affected(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        &options,
                        black_box(&prior),
                        black_box(changed),
                    )
                    .expect("complete standard-risk selective valuation")
                },
                |result| validity::valuation(&portfolio, result, &[MetricId::Dv01]),
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_portfolio_valuation,
    bench_entity_aggregation,
    bench_multicurrency_aggregation,
    bench_position_filtering,
    bench_portfolio_scaling,
    bench_selective_repricing_shapes,
    bench_revalue_affected,
);
criterion_main!(benches);
