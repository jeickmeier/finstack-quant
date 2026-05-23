//! Benchmarks for factor model primitives.
//!
//! Tests performance of:
//! - FactorCovarianceMatrix construction and validation (symmetry + PSD check)
//! - Variance, covariance, and correlation lookups
//! - MappingTableMatcher rule evaluation
//! - HierarchicalMatcher tree traversal
//! - CascadeMatcher chain evaluation
//! - Scaling behavior with increasing factor/rule counts

mod bench_utils;

use bench_utils::bench_iter;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_core::types::{Attributes, CurveId};
use finstack_factor_model::matching::{
    AttributeFilter, CascadeMatcher, DependencyFilter, FactorMatcher, FactorNode,
    HierarchicalMatcher, MappingRule, MappingTableMatcher,
};
use finstack_factor_model::{
    CurveType, DependencyType, FactorCovarianceMatrix, FactorId, MarketDependency,
};
use std::hint::black_box;

fn make_factor_ids(n: usize) -> Vec<FactorId> {
    (0..n).map(|i| FactorId::new(format!("F{i}"))).collect()
}

fn make_psd_matrix(n: usize) -> Vec<f64> {
    let mut data = vec![0.0; n * n];
    for i in 0..n {
        data[i * n + i] = 1.0;
        for j in (i + 1)..n {
            let cov = 0.3 / ((j - i) as f64 + 1.0);
            data[i * n + j] = cov;
            data[j * n + i] = cov;
        }
    }
    data
}

fn bench_covariance_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("covariance_construction");

    for n in [5, 10, 25, 50, 100] {
        let ids = make_factor_ids(n);
        let data = make_psd_matrix(n);
        group.bench_with_input(BenchmarkId::new("validated", n), &n, |b, _| {
            b.iter(|| {
                let m =
                    FactorCovarianceMatrix::new(black_box(ids.clone()), black_box(data.clone()))
                        .unwrap();
                black_box(m);
            })
        });
    }

    for n in [5, 10, 25, 50, 100] {
        let ids = make_factor_ids(n);
        let data = make_psd_matrix(n);
        group.bench_with_input(BenchmarkId::new("unchecked", n), &n, |b, _| {
            b.iter(|| {
                let m = FactorCovarianceMatrix::new_unchecked(
                    black_box(ids.clone()),
                    black_box(data.clone()),
                );
                black_box(m);
            })
        });
    }

    group.finish();
}

fn bench_covariance_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("covariance_lookups");

    for n in [10, 50, 100] {
        let ids = make_factor_ids(n);
        let data = make_psd_matrix(n);
        let matrix = FactorCovarianceMatrix::new(ids.clone(), data).unwrap();
        let id_a = &ids[0];
        let id_b = &ids[n / 2];

        group.bench_with_input(BenchmarkId::new("variance", n), &n, |b, _| {
            b.iter(|| black_box(matrix.variance(black_box(id_a))))
        });

        group.bench_with_input(BenchmarkId::new("covariance", n), &n, |b, _| {
            b.iter(|| black_box(matrix.covariance(black_box(id_a), black_box(id_b))))
        });

        group.bench_with_input(BenchmarkId::new("correlation", n), &n, |b, _| {
            b.iter(|| black_box(matrix.correlation(black_box(id_a), black_box(id_b))))
        });
    }

    group.finish();
}

fn bench_covariance_batch_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("covariance_batch_lookups");

    for n in [10, 50, 100] {
        let ids = make_factor_ids(n);
        let data = make_psd_matrix(n);
        let matrix = FactorCovarianceMatrix::new(ids.clone(), data).unwrap();

        group.bench_with_input(BenchmarkId::new("all_variances", n), &n, |b, _| {
            b.iter(|| {
                let vars: Vec<f64> = ids.iter().map(|id| matrix.variance(id)).collect();
                black_box(vars);
            })
        });

        group.bench_with_input(BenchmarkId::new("all_correlations", n), &n, |b, _| {
            b.iter(|| {
                let mut corrs = Vec::with_capacity(n * (n - 1) / 2);
                for i in 0..ids.len() {
                    for j in (i + 1)..ids.len() {
                        corrs.push(matrix.correlation(&ids[i], &ids[j]));
                    }
                }
                black_box(corrs);
            })
        });
    }

    group.finish();
}

fn make_mapping_rules(n: usize) -> Vec<MappingRule> {
    (0..n)
        .map(|i| MappingRule {
            dependency_filter: DependencyFilter {
                dependency_type: Some(DependencyType::Discount),
                curve_type: Some(CurveType::Discount),
                id: Some(format!("CURVE-{i}")),
            },
            attribute_filter: AttributeFilter::default(),
            factor_id: FactorId::new(format!("Factor-{i}")),
        })
        .collect()
}

fn bench_mapping_table_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("mapping_table_matcher");

    for n in [5, 20, 50, 100] {
        let rules = make_mapping_rules(n);
        let matcher = MappingTableMatcher::new(rules);
        let attrs = Attributes::default();

        let first_dep = MarketDependency::Curve {
            id: CurveId::new("CURVE-0"),
            curve_type: CurveType::Discount,
        };
        group.bench_with_input(BenchmarkId::new("hit_first", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(black_box(&first_dep), &attrs)))
        });

        let last_dep = MarketDependency::Curve {
            id: CurveId::new(format!("CURVE-{}", n - 1)),
            curve_type: CurveType::Discount,
        };
        group.bench_with_input(BenchmarkId::new("hit_last", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(black_box(&last_dep), &attrs)))
        });

        let miss_dep = MarketDependency::Curve {
            id: CurveId::new("CURVE-MISSING"),
            curve_type: CurveType::Discount,
        };
        group.bench_with_input(BenchmarkId::new("miss", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(black_box(&miss_dep), &attrs)))
        });
    }

    group.finish();
}

fn bench_hierarchical_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("hierarchical_matcher");

    let build_tree = |depth: usize, branching: usize| -> FactorNode {
        fn build_level(depth: usize, branching: usize, prefix: &str) -> FactorNode {
            if depth == 0 {
                return FactorNode {
                    factor_id: Some(FactorId::new(format!("{prefix}-leaf"))),
                    filter: AttributeFilter::default(),
                    children: vec![],
                };
            }
            let children = (0..branching)
                .map(|i| {
                    let child_prefix = format!("{prefix}-{i}");
                    let filter = AttributeFilter {
                        tags: vec![format!("sector-{i}")],
                        meta: vec![],
                    };
                    let mut node = build_level(depth - 1, branching, &child_prefix);
                    node.filter = filter;
                    node
                })
                .collect();
            FactorNode {
                factor_id: Some(FactorId::new(format!("{prefix}-node"))),
                filter: AttributeFilter::default(),
                children,
            }
        }
        build_level(depth, branching, "root")
    };

    let configs = [
        ("shallow_2x3", 2, 3),
        ("medium_3x3", 3, 3),
        ("deep_4x2", 4, 2),
    ];

    let dep = MarketDependency::Curve {
        id: CurveId::new("USD-OIS"),
        curve_type: CurveType::Discount,
    };

    for (name, depth, branching) in configs {
        let root = build_tree(depth, branching);
        let matcher = HierarchicalMatcher::new(root);

        let attrs_hit = Attributes::default().with_tag("sector-0");

        bench_iter(&mut group, format!("{name}_hit"), || {
            black_box(matcher.match_factor(black_box(&dep), &attrs_hit));
        });

        let attrs_miss = Attributes::default().with_tag("nonexistent");

        bench_iter(&mut group, format!("{name}_fallback"), || {
            black_box(matcher.match_factor(black_box(&dep), &attrs_miss));
        });
    }

    group.finish();
}

fn bench_cascade_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("cascade_matcher");

    let exact_rules = vec![MappingRule {
        dependency_filter: DependencyFilter {
            dependency_type: Some(DependencyType::Credit),
            curve_type: None,
            id: Some("ACME-HAZARD".into()),
        },
        attribute_filter: AttributeFilter::default(),
        factor_id: FactorId::new("ACME-Specific"),
    }];

    let fallback_rules = vec![MappingRule {
        dependency_filter: DependencyFilter {
            dependency_type: Some(DependencyType::Credit),
            curve_type: None,
            id: None,
        },
        attribute_filter: AttributeFilter::default(),
        factor_id: FactorId::new("Generic-Credit"),
    }];

    let cascade = CascadeMatcher::new(vec![
        Box::new(MappingTableMatcher::new(exact_rules)),
        Box::new(MappingTableMatcher::new(fallback_rules)),
    ]);
    let attrs = Attributes::default();

    let exact_dep = MarketDependency::CreditCurve {
        id: CurveId::new("ACME-HAZARD"),
    };
    bench_iter(&mut group, "hit_first_stage", || {
        black_box(cascade.match_factor(black_box(&exact_dep), &attrs));
    });

    let fallback_dep = MarketDependency::CreditCurve {
        id: CurveId::new("OTHER-HAZARD"),
    };
    bench_iter(&mut group, "hit_second_stage", || {
        black_box(cascade.match_factor(black_box(&fallback_dep), &attrs));
    });

    let miss_dep = MarketDependency::Spot {
        id: "EQUITY".into(),
    };
    bench_iter(&mut group, "miss_all_stages", || {
        black_box(cascade.match_factor(black_box(&miss_dep), &attrs));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_covariance_construction,
    bench_covariance_lookups,
    bench_covariance_batch_lookups,
    bench_mapping_table_matcher,
    bench_hierarchical_matcher,
    bench_cascade_matcher,
);
criterion_main!(benches);
