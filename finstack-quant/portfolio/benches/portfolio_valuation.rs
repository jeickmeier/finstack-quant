//! Portfolio valuation benchmarks.
//!
//! Measures performance of portfolio operations for large institutional portfolios:
//! - Full portfolio valuation (all instrument types)
//! - Scaling with portfolio size
//! - Multi-currency aggregation
//! - Entity-level aggregation
//! - Attribute grouping and filtering
//!
//! Simulates realistic institutional portfolios with:
//! - Multiple entities (funds, accounts)
//! - All major instrument types
//! - Cross-currency positions
//! - Various position sizes

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{base_date, create_institutional_portfolio, create_market_context, maturity_2y};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
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
    revalue_affected, value_portfolio, PortfolioValuationOptions, RequestedMetrics,
};
use finstack_quant_portfolio::{MarketFactorKey, PortfolioBuilder};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::RatesCurveKind;
use rust_decimal_macros::dec;
use std::hint::black_box;
use std::sync::Arc;

fn xl_benchmarks_enabled() -> bool {
    std::env::var("FINSTACK_PORTFOLIO_BENCH_XL").is_ok_and(|value| value == "1")
}

fn best_effort_standard_metrics() -> PortfolioValuationOptions {
    PortfolioValuationOptions {
        strict_risk: false,
        metrics: RequestedMetrics::Standard,
    }
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
    let mut builder = PortfolioBuilder::new("SELECTIVE_BENCHMARK")
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
            .quote_rate_opt(Some(dec!(0.04)))
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
    let market = create_market_context();
    let config = FinstackConfig::default();
    let options = best_effort_standard_metrics();

    {
        let num_positions = &250;
        let portfolio = create_institutional_portfolio(*num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            num_positions,
            |b, _| {
                b.iter(|| {
                    value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        black_box(&options),
                    )
                    .expect("portfolio valuation benchmark")
                });
            },
        );
    }
    group.finish();
}

// Entity Aggregation Benchmarks

fn bench_entity_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_entity_aggregation");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let options = best_effort_standard_metrics();

    {
        let num_positions = &250;
        let portfolio = create_institutional_portfolio(*num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            num_positions,
            |b, _| {
                b.iter(|| {
                    let valuation = value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        black_box(&options),
                    )
                    .unwrap();
                    // Access entity aggregates
                    for i in 1..=5 {
                        let _ = valuation.get_entity_value(&format!("FUND_{}", i));
                    }
                });
            },
        );
    }
    group.finish();
}

// Multi-Currency Aggregation Benchmarks

fn bench_multicurrency_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_multicurrency");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let portfolio = create_institutional_portfolio(100);
    let options = best_effort_standard_metrics();

    group.bench_function("100pos_multicurrency", |b| {
        b.iter(|| {
            value_portfolio(
                black_box(&portfolio),
                black_box(&market),
                black_box(&config),
                black_box(&options),
            )
            .expect("multicurrency portfolio benchmark")
        });
    });
    group.finish();
}

// Position Filtering Benchmarks

fn bench_position_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_filtering");
    let portfolio = create_institutional_portfolio(250);

    group.bench_function("filter_by_entity", |b| {
        b.iter(|| {
            let target = black_box("FUND_1");
            let matched: Vec<_> = portfolio
                .positions()
                .iter()
                .filter(|position| position.entity_id.as_str() == target)
                .collect();
            black_box(matched);
        });
    });

    group.bench_function("iterate_all_positions", |b| {
        b.iter(|| {
            let count = portfolio.positions().len();
            black_box(count);
        });
    });

    group.finish();
}

// Scaling Benchmarks

fn bench_portfolio_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_pv_scaling");
    group.sample_size(10);
    let market = create_market_context();
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions {
        strict_risk: false,
        metrics: RequestedMetrics::Only(Vec::new()),
    };

    let mut position_counts = vec![63usize, 64, 250, 3_000];
    if xl_benchmarks_enabled() {
        position_counts.push(25_000);
    }
    for num_positions in position_counts {
        let portfolio = create_institutional_portfolio(num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}pos", num_positions)),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    value_portfolio(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        black_box(&options),
                    )
                    .expect("portfolio scaling benchmark")
                });
            },
        );
    }
    group.finish();
}

fn bench_selective_repricing_shapes(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_selective_repricing_3000");
    group.sample_size(30);
    let market = create_selective_benchmark_market();
    let portfolio = create_selective_benchmark_portfolio(3_000);
    let config = FinstackConfig::default();
    let options = PortfolioValuationOptions {
        strict_risk: true,
        metrics: RequestedMetrics::Only(Vec::new()),
    };
    let prior =
        value_portfolio(&portfolio, &market, &config, &options).expect("selective benchmark prior");
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

    group.bench_function("full_control", |bencher| {
        bencher.iter(|| {
            value_portfolio(
                black_box(&portfolio),
                black_box(&market),
                black_box(&config),
                black_box(&options),
            )
            .expect("full selective control")
        });
    });
    for (label, changed) in &cases {
        group.bench_with_input(
            BenchmarkId::from_parameter(*label),
            changed,
            |bencher, changed| {
                bencher.iter(|| {
                    revalue_affected(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(&config),
                        black_box(&options),
                        black_box(&prior),
                        black_box(changed),
                    )
                    .expect("selective shape benchmark")
                });
            },
        );
    }
    group.finish();
}

// Selective Repricing Benchmarks

/// Bench `revalue_affected` under three "changed factor" scenarios that exercise
/// different affected-position fractions:
///
/// - `broad_curve`  — USD-OIS discount curve changed → reprices nearly all instruments
/// - `equity_spot`  — AAPL spot changed → reprices only equity / equity-option positions
/// - `hazard_curve` — CORP-HAZARD changed → reprices CDS / CDS-option positions
fn bench_revalue_affected(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_revalue_affected");
    let market = create_market_context();
    let config = FinstackConfig::default();
    let options = best_effort_standard_metrics();

    let broad_curve = vec![MarketFactorKey::curve(
        "USD-OIS".into(),
        RatesCurveKind::Discount,
    )];
    let equity_spot = vec![MarketFactorKey::spot("AAPL")];
    let hazard_curve = vec![MarketFactorKey::curve(
        "CORP-HAZARD".into(),
        RatesCurveKind::Credit,
    )];

    let scenarios: &[(&str, &[MarketFactorKey])] = &[
        ("broad_curve", &broad_curve),
        ("equity_spot", &equity_spot),
        ("hazard_curve", &hazard_curve),
    ];

    for num_positions in [250usize] {
        let portfolio = create_institutional_portfolio(num_positions);
        let prior = value_portfolio(&portfolio, &market, &config, &options).unwrap();

        for (label, changed) in scenarios {
            group.bench_with_input(
                BenchmarkId::new(format!("{}pos", num_positions), label),
                changed,
                |b, changed| {
                    b.iter(|| {
                        revalue_affected(
                            black_box(&portfolio),
                            black_box(&market),
                            black_box(&config),
                            black_box(&options),
                            black_box(&prior),
                            black_box(changed),
                        )
                        .unwrap()
                    });
                },
            );
        }
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
