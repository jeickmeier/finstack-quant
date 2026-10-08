//! Scaling guards for `finstack-quant-models::factor`.
//!
//! Complements `factor_model.rs` (absolute cost at one size) by measuring how
//! cost grows with issuer count, factor dimension, or rule count. Read
//! ns-per-element across sizes: flat is linear; rising means a super-linear
//! term is back.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::types::Attributes;
use finstack_quant_models::factor::credit::calibration::{CovarianceStrategy, CreditCalibrator};
use finstack_quant_models::factor::credit::decomposition::decompose_levels;
use finstack_quant_models::factor::matching::{
    CreditHierarchicalMatcher, FactorMatcher, MappingTableMatcher,
};
use finstack_quant_models::factor::{CurveType, FactorCovarianceMatrix, MarketDependency};
use fixtures::{
    credit_dependency, credit_hierarchical_config, factor_ids, known_issuer_attrs, mapping_rules,
    psd_matrix, zero_sensitivity_matrix, CreditBook,
};

fn scaling_covariance_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_covariance_construction");
    for n in [20_usize, 50, 100, 200] {
        let ids = factor_ids(n);
        let data = psd_matrix(n);
        group.throughput(Throughput::Elements((n * n) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter_batched(
                || (ids.clone(), data.clone()),
                |(ids, data)| black_box(FactorCovarianceMatrix::new(ids, data).unwrap()),
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn scaling_mapping_table(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_mapping_table");
    let attrs = Attributes::default();
    for n in [50_usize, 200, 500] {
        let matcher = MappingTableMatcher::new(mapping_rules(n));
        let last_dep = MarketDependency::Curve {
            id: finstack_quant_core::types::CurveId::new(format!("CURVE-{}", n - 1)),
            curve_type: CurveType::Discount,
        };
        let miss_dep = MarketDependency::Curve {
            id: finstack_quant_core::types::CurveId::new("CURVE-MISSING"),
            curve_type: CurveType::Discount,
        };
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("hit_last", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(&last_dep, &attrs)))
        });
        group.bench_with_input(BenchmarkId::new("miss", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(&miss_dep, &attrs)))
        });
    }
    group.finish();
}

fn scaling_credit_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_credit_matcher");
    for n in [50_usize, 200, 1_000] {
        let config = credit_hierarchical_config(n);
        let matcher = CreditHierarchicalMatcher::new(config.clone());
        let dep = credit_dependency(&format!("ISSUER-{:04}-HAZARD", n - 1));
        let attrs = known_issuer_attrs(n - 1);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("known_last", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(&dep, &attrs).unwrap()))
        });
        group.bench_with_input(BenchmarkId::new("enumerate", n), &n, |b, _| {
            b.iter(|| black_box(config.enumerate_factor_ids()))
        });
    }
    group.finish();
}

fn scaling_calibration(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_calibration");
    group.sample_size(10);

    for n in [25_usize, 50, 100] {
        let book = CreditBook::new(n, 36, 3).with_strategy(CovarianceStrategy::Diagonal);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("diagonal_globally_off", n), &n, |b, _| {
            b.iter_batched(
                || book.inputs.clone(),
                |inputs| {
                    black_box(
                        CreditCalibrator::new(book.config.clone())
                            .calibrate(inputs)
                            .unwrap(),
                    )
                },
                BatchSize::SmallInput,
            )
        });
    }

    for n in [25_usize, 50] {
        let book = CreditBook::new(n, 36, 3);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("full_sample_globally_off", n),
            &n,
            |b, _| {
                b.iter_batched(
                    || book.inputs.clone(),
                    |inputs| {
                        black_box(
                            CreditCalibrator::new(book.config.clone())
                                .calibrate(inputs)
                                .unwrap(),
                        )
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    for n in [25_usize, 50] {
        let book = CreditBook::new(n, 36, 3).with_issuer_beta();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(
            BenchmarkId::new("full_sample_issuer_beta", n),
            &n,
            |b, _| {
                b.iter_batched(
                    || book.inputs.clone(),
                    |inputs| {
                        black_box(
                            CreditCalibrator::new(book.config.clone())
                                .calibrate(inputs)
                                .unwrap(),
                        )
                    },
                    BatchSize::SmallInput,
                )
            },
        );
    }

    group.finish();
}

fn scaling_decompose_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_decompose_levels");
    group.sample_size(15);

    for n in [50_usize, 200, 500] {
        let book = CreditBook::new(n, 24, 3).with_strategy(CovarianceStrategy::Diagonal);
        let model = book.calibrate();
        let spreads = book.inputs.as_of_spreads.clone();
        let generic = *book.inputs.generic_factor.values.last().unwrap();
        let as_of = book.inputs.as_of;
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(decompose_levels(&model, &spreads, generic, as_of, None).unwrap()))
        });
    }

    group.finish();
}

fn scaling_sensitivity_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_sensitivity_matrix");
    for (n_pos, n_fac) in [(100_usize, 10_usize), (1_000, 50), (5_000, 50)] {
        let elements = (n_pos * n_fac) as u64;
        group.throughput(Throughput::Elements(elements));
        group.bench_with_input(
            BenchmarkId::new("zeros", format!("{n_pos}x{n_fac}")),
            &(n_pos, n_fac),
            |b, &(n_pos, n_fac)| b.iter(|| black_box(zero_sensitivity_matrix(n_pos, n_fac))),
        );
        let matrix = zero_sensitivity_matrix(n_pos, n_fac);
        group.bench_with_input(
            BenchmarkId::new("factor_deltas", format!("{n_pos}x{n_fac}")),
            &(n_pos, n_fac),
            |b, &(_, n_fac)| {
                b.iter(|| {
                    let cols: Vec<Vec<f64>> = (0..n_fac)
                        .map(|f| matrix.factor_deltas(f).unwrap())
                        .collect();
                    black_box(cols);
                })
            },
        );
    }
    group.finish();
}

fn scaling_covariance_batch_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_covariance_batch_lookups");
    for n in [20_usize, 50, 100] {
        let matrix = FactorCovarianceMatrix::new(factor_ids(n), psd_matrix(n)).unwrap();
        let pairs = n * (n - 1) / 2;
        group.throughput(Throughput::Elements(pairs as u64));
        group.bench_with_input(BenchmarkId::new("all_correlations", n), &n, |b, _| {
            b.iter(|| {
                let mut corrs = Vec::with_capacity(pairs);
                for i in 0..n {
                    for j in (i + 1)..n {
                        corrs.push(matrix.correlation_at(i, j));
                    }
                }
                black_box(corrs);
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    scaling_covariance_construction,
    scaling_mapping_table,
    scaling_credit_matcher,
    scaling_calibration,
    scaling_decompose_levels,
    scaling_sensitivity_matrix,
    scaling_covariance_batch_lookups,
);
criterion_main!(benches);
