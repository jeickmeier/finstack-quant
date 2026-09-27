//! Book-rollup and reporting analytics that sit next to the priced book.
//!
//! These paths were previously unit-tested only. Each group uses a
//! representative institutional or synthetic fixture so Criterion measures
//! the user-facing entry point rather than a micro-loop.

#[path = "bench_common.rs"]
mod bench_common;

use bench_common::{create_institutional_portfolio, create_market_context};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::config::FinstackConfig;
use finstack_quant_core::currency::Currency;
use finstack_quant_portfolio::book::Book;
use finstack_quant_portfolio::brinson::{brinson_fachler, carino_link, SectorPeriod};
use finstack_quant_portfolio::excess_return::{
    cell_returns_from_reference, excess_returns, CellConfig, ExcessReturnPosition, ReferenceReturn,
};
use finstack_quant_portfolio::factor_brinson::{factor_brinson_attribution, FactorBrinsonInput};
use finstack_quant_portfolio::fi_attribution::{
    campisi_attribution, FiAttributionConfig, FiPositionSnapshot,
};
use finstack_quant_portfolio::grid_attribution::{grid_attribution, GridPosition};
use finstack_quant_portfolio::grouping::aggregate_by_book;
use finstack_quant_portfolio::primitive_exposures;
use finstack_quant_portfolio::valuation::{
    value_portfolio, PortfolioValuationOptions, RequestedMetrics,
};
use finstack_quant_portfolio::PortfolioBuilder;
use finstack_quant_valuations::metrics::MetricId;
use std::hint::black_box;

fn booked_institutional_portfolio(num_positions: usize) -> finstack_quant_portfolio::Portfolio {
    let source = create_institutional_portfolio(num_positions);
    let n_regions = 4_usize;
    let n_desks = 8_usize;
    let desks_per_region = n_desks / n_regions;

    let mut builder = PortfolioBuilder::new(format!("{}_BOOKS", source.id))
        .base_currency(source.base_currency)
        .as_of(source.as_of)
        .entities(source.entities.values().cloned())
        .book(Book::new("ROOT", Some("Root".into())));

    for region in 0..n_regions {
        builder = builder.book(
            Book::new(format!("REGION_{region}"), Some(format!("Region {region}")))
                .with_parent("ROOT"),
        );
        for desk in 0..desks_per_region {
            let desk_idx = region * desks_per_region + desk;
            builder = builder.book(
                Book::new(format!("DESK_{desk_idx}"), Some(format!("Desk {desk_idx}")))
                    .with_parent(format!("REGION_{region}")),
            );
        }
    }

    for position in source.positions().iter().cloned() {
        builder = builder.position(position);
    }
    for (index, position) in source.positions().iter().enumerate() {
        let desk = format!("DESK_{}", index % n_desks);
        builder = builder
            .add_position_to_book(position.position_id.clone(), desk)
            .expect("bench: book assignment");
    }
    builder.build().expect("bench: booked portfolio")
}

fn bench_book_rollup(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_book_rollup");
    let market = create_market_context();
    let config = FinstackConfig::default();

    for num_positions in [250_usize, 3_000] {
        let portfolio = booked_institutional_portfolio(num_positions);
        let pv_only = PortfolioValuationOptions {
            strict_risk: false,
            metrics: RequestedMetrics::Only(Vec::new()),
        };
        let valuation = value_portfolio(&portfolio, &market, &config, &pv_only)
            .expect("bench: booked valuation");

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{num_positions}pos")),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    aggregate_by_book(
                        black_box(&valuation),
                        black_box(&portfolio.books),
                        black_box(Currency::USD),
                    )
                    .expect("bench: book rollup")
                });
            },
        );
    }
    group.finish();
}

fn bench_primitive_exposure(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_primitive_exposure");
    group.sample_size(10);
    let market = create_market_context();
    let metrics: [MetricId; 0] = [];

    for num_positions in [64_usize, 250] {
        let portfolio = create_institutional_portfolio(num_positions);
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{num_positions}pos")),
            &num_positions,
            |b, _| {
                b.iter(|| {
                    primitive_exposures(
                        black_box(&portfolio),
                        black_box(&market),
                        black_box(metrics.as_slice()),
                    )
                    .expect("bench: primitive exposure")
                });
            },
        );
    }
    group.finish();
}

fn brinson_sectors(n_sectors: usize, tilt: f64) -> Vec<SectorPeriod> {
    let weight = 1.0 / n_sectors as f64;
    (0..n_sectors)
        .map(|index| SectorPeriod {
            sector: format!("S{index}"),
            portfolio_weight: weight,
            benchmark_weight: weight,
            portfolio_return: 0.012 + tilt * (index as f64) * 1e-4,
            benchmark_return: 0.010 + (index as f64) * 1e-4,
        })
        .collect()
}

fn bench_brinson(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_brinson");
    let sectors = brinson_sectors(50, 1.0);
    group.bench_function("brinson_fachler_50_sectors", |b| {
        b.iter(|| brinson_fachler(black_box(&sectors)).expect("bench: brinson"));
    });

    let periods: Vec<_> = (0..252)
        .map(|period| brinson_fachler(&brinson_sectors(20, period as f64)).expect("period"))
        .collect();
    group.bench_function("carino_link_252x20", |b| {
        b.iter(|| carino_link(black_box(&periods)).expect("bench: carino"));
    });
    group.finish();
}

fn campisi_side(n_positions: usize, n_sectors: usize, tilt: f64) -> Vec<FiPositionSnapshot> {
    let weight = 1.0 / n_positions as f64;
    (0..n_positions)
        .map(|index| FiPositionSnapshot {
            sector: format!("S{}", index % n_sectors),
            weight,
            total_return: 0.012 + tilt * (index as f64) * 1e-6,
            yield_annual: 0.04,
            modified_duration: 5.0,
            spread_duration: 4.0,
            spread: 0.01,
            delta_treasury_yield: -0.001,
            delta_spread: 0.0005,
        })
        .collect()
}

fn bench_campisi(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_campisi");
    let config = FiAttributionConfig::new(0.25);
    let portfolio = campisi_side(2_000, 20, 1.0);
    let benchmark = campisi_side(2_000, 20, 0.0);
    group.bench_function("campisi_2000pos_20sectors", |b| {
        b.iter(|| {
            campisi_attribution(
                black_box(&portfolio),
                black_box(&benchmark),
                black_box(&config),
            )
            .expect("bench: campisi")
        });
    });
    group.finish();
}

fn grid_side(n_positions: usize, n_cells: usize, n_sectors: usize, tilt: f64) -> Vec<GridPosition> {
    let weight = 1.0 / n_positions as f64;
    (0..n_positions)
        .map(|index| GridPosition {
            cell: format!("C{}", index % n_cells),
            sector: format!("S{}", index % n_sectors),
            weight,
            total_return: 0.011 + tilt * (index as f64) * 1e-6,
        })
        .collect()
}

fn bench_grid_attribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_grid_attribution");
    let portfolio = grid_side(2_000, 10, 20, 1.0);
    let benchmark = grid_side(2_000, 10, 20, 0.0);
    group.bench_function("grid_2000pos_10x20", |b| {
        b.iter(|| {
            grid_attribution(black_box(&portfolio), black_box(&benchmark)).expect("bench: grid")
        });
    });
    group.finish();
}

fn factor_brinson_case(n_assets: usize, n_factors: usize) -> (FactorBrinsonInput, Vec<f64>) {
    let mut exposures = vec![0.0; n_assets * n_factors];
    let mut asset_returns = Vec::with_capacity(n_assets);
    let mut portfolio_weights = vec![1.0 / n_assets as f64; n_assets];
    let benchmark_weights = vec![1.0 / n_assets as f64; n_assets];
    if n_assets >= 2 {
        portfolio_weights[0] += 0.01;
        portfolio_weights[1] -= 0.01;
    }
    for index in 0..n_assets {
        let factor = index % n_factors;
        exposures[index * n_factors + factor] = 1.0;
        asset_returns.push(0.01 + 0.001 * (factor as f64) + 0.0001 * ((index % 7) as f64));
    }
    let mut numerator = vec![0.0; n_factors];
    let mut denominator = vec![0.0; n_factors];
    for index in 0..n_assets {
        let factor = index % n_factors;
        numerator[factor] += benchmark_weights[index] * asset_returns[index];
        denominator[factor] += benchmark_weights[index];
    }
    let factor_returns = numerator
        .iter()
        .zip(&denominator)
        .map(|(num, den)| num / den)
        .collect();
    let input = FactorBrinsonInput {
        asset_ids: (0..n_assets).map(|index| format!("A{index}")).collect(),
        asset_returns,
        exposures,
        factor_names: (0..n_factors).map(|index| format!("F{index}")).collect(),
        portfolio_weights,
        benchmark_weights,
    };
    (input, factor_returns)
}

fn bench_factor_brinson(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_factor_brinson");
    let (input, factor_returns) = factor_brinson_case(2_000, 20);
    group.bench_function("factor_brinson_2000x20", |b| {
        b.iter(|| {
            factor_brinson_attribution(black_box(&input), black_box(&factor_returns))
                .expect("bench: factor-brinson")
        });
    });
    group.finish();
}

fn bench_excess_return(c: &mut Criterion) {
    let mut group = c.benchmark_group("portfolio_excess_return");
    let reference: Vec<ReferenceReturn> = (0..40)
        .map(|index| ReferenceReturn {
            duration: 0.25 + index as f64 * 0.25,
            total_return: 0.01 + index as f64 * 0.001,
        })
        .collect();
    let table = cell_returns_from_reference(&reference, "UST", &CellConfig { width: 0.5 })
        .expect("bench: duration-cell table");
    let n_positions = 2_000_usize;
    let weight = 1.0 / n_positions as f64;
    let positions: Vec<ExcessReturnPosition> = (0..n_positions)
        .map(|index| ExcessReturnPosition {
            id: format!("P{index}"),
            weight,
            duration: 0.3 + (index % 35) as f64 * 0.25,
            total_return: 0.015 + (index as f64) * 1e-6,
        })
        .collect();

    group.bench_function("excess_returns_2000pos", |b| {
        b.iter(|| excess_returns(black_box(&positions), black_box(&table)).expect("bench: excess"));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_book_rollup,
    bench_primitive_exposure,
    bench_brinson,
    bench_campisi,
    bench_grid_attribution,
    bench_factor_brinson,
    bench_excess_return
);
criterion_main!(benches);
