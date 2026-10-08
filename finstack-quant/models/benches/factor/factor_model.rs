//! Hot-path benchmarks for `finstack-quant-models::factor`.
//!
//! Fixed-size cost of every public inner loop at one representative size.
//! Size sweeps live in `factor_model_scale.rs`.
//!
//! Legacy groups (`covariance_*`, `mapping_table_matcher`,
//! `hierarchical_matcher`, `cascade_matcher`) keep their Criterion ids so
//! `--baseline` compares stay valid.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/bench_utils.rs"]
mod bench_utils;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use bench_utils::bench_iter;
use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use finstack_quant_core::types::Attributes;
use finstack_quant_models::factor::credit::calibration::CovarianceStrategy;
use finstack_quant_models::factor::credit::decomposition::{decompose_levels, decompose_period};
use finstack_quant_models::factor::matching::{
    CascadeMatcher, CreditHierarchicalMatcher, DependencyFilter, FactorMatcher, FactorNode,
    HierarchicalMatcher, MappingTableMatcher,
};
use finstack_quant_models::factor::{
    CurveType, FactorCovarianceMatrix, FactorId, MarketDependency, SensitivityMatrix,
};
use fixtures::{
    credit_dependency, credit_hierarchical_config, factor_ids, known_issuer_attrs, mapping_rules,
    psd_matrix, unknown_issuer_attrs, zero_sensitivity_matrix, CreditBook,
};

fn bench_covariance_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("covariance_construction");

    {
        let n = 50;
        let ids = factor_ids(n);
        let data = psd_matrix(n);
        group.bench_with_input(BenchmarkId::new("validated", n), &n, |b, _| {
            b.iter(|| {
                let m =
                    FactorCovarianceMatrix::new(black_box(ids.clone()), black_box(data.clone()))
                        .unwrap();
                black_box(m);
            })
        });
    }

    group.finish();
}

fn bench_covariance_lookups(c: &mut Criterion) {
    let mut group = c.benchmark_group("covariance_lookups");

    {
        let n = 50;
        let ids = factor_ids(n);
        let data = psd_matrix(n);
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

    {
        let n = 50;
        let data = psd_matrix(n);
        let matrix = FactorCovarianceMatrix::new(factor_ids(n), data).unwrap();

        group.bench_with_input(BenchmarkId::new("all_variances", n), &n, |b, _| {
            b.iter(|| {
                let vars: Vec<f64> = (0..n).map(|i| matrix.variance_at(i)).collect();
                black_box(vars);
            })
        });

        group.bench_with_input(BenchmarkId::new("all_correlations", n), &n, |b, _| {
            b.iter(|| {
                let mut corrs = Vec::with_capacity(n * (n - 1) / 2);
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

fn bench_mapping_table_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("mapping_table_matcher");

    {
        let n = 50;
        let rules = mapping_rules(n);
        let matcher = MappingTableMatcher::new(rules);
        let attrs = Attributes::default();

        let first_dep = MarketDependency::Curve {
            id: finstack_quant_core::types::CurveId::new("CURVE-0"),
            curve_type: CurveType::Discount,
        };
        group.bench_with_input(BenchmarkId::new("hit_first", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(black_box(&first_dep), &attrs)))
        });

        let last_dep = MarketDependency::Curve {
            id: finstack_quant_core::types::CurveId::new(format!("CURVE-{}", n - 1)),
            curve_type: CurveType::Discount,
        };
        group.bench_with_input(BenchmarkId::new("hit_last", n), &n, |b, _| {
            b.iter(|| black_box(matcher.match_factor(black_box(&last_dep), &attrs)))
        });

        let miss_dep = MarketDependency::Curve {
            id: finstack_quant_core::types::CurveId::new("CURVE-MISSING"),
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
                    filter: finstack_quant_models::factor::AttributeFilter::default(),
                    children: vec![],
                };
            }
            let children = (0..branching)
                .map(|i| {
                    let child_prefix = format!("{prefix}-{i}");
                    let filter = finstack_quant_models::factor::AttributeFilter {
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
                filter: finstack_quant_models::factor::AttributeFilter::default(),
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
        id: finstack_quant_core::types::CurveId::new("USD-OIS"),
        curve_type: CurveType::Discount,
    };

    for (name, depth, branching) in configs {
        let root = build_tree(depth, branching);
        let matcher = HierarchicalMatcher::new(DependencyFilter::default(), root);

        let attrs_hit = Attributes::default().with_tag("sector-0");

        bench_iter(&mut group, format!("{name}_hit"), || {
            let _ = black_box(matcher.match_factor(black_box(&dep), &attrs_hit));
        });

        let attrs_miss = Attributes::default().with_tag("nonexistent");

        bench_iter(&mut group, format!("{name}_fallback"), || {
            let _ = black_box(matcher.match_factor(black_box(&dep), &attrs_miss));
        });
    }

    group.finish();
}

fn bench_cascade_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("cascade_matcher");

    let exact_rules = vec![finstack_quant_models::factor::matching::MappingRule {
        dependency_filter: finstack_quant_models::factor::DependencyFilter {
            dependency_type: Some(finstack_quant_models::factor::DependencyType::Credit),
            curve_type: None,
            id: Some("ACME-HAZARD".into()),
        },
        attribute_filter: finstack_quant_models::factor::AttributeFilter::default(),
        factor_id: FactorId::new("ACME-Specific"),
    }];

    let fallback_rules = vec![finstack_quant_models::factor::matching::MappingRule {
        dependency_filter: finstack_quant_models::factor::DependencyFilter {
            dependency_type: Some(finstack_quant_models::factor::DependencyType::Credit),
            curve_type: None,
            id: None,
        },
        attribute_filter: finstack_quant_models::factor::AttributeFilter::default(),
        factor_id: FactorId::new("Generic-Credit"),
    }];

    let cascade = CascadeMatcher::new(vec![
        Box::new(MappingTableMatcher::new(exact_rules)),
        Box::new(MappingTableMatcher::new(fallback_rules)),
    ]);
    let attrs = Attributes::default();

    let exact_dep = MarketDependency::CreditCurve {
        id: finstack_quant_core::types::CurveId::new("ACME-HAZARD"),
    };
    bench_iter(&mut group, "hit_first_stage", || {
        let _ = black_box(cascade.match_factor(black_box(&exact_dep), &attrs));
    });

    let fallback_dep = MarketDependency::CreditCurve {
        id: finstack_quant_core::types::CurveId::new("OTHER-HAZARD"),
    };
    bench_iter(&mut group, "hit_second_stage", || {
        let _ = black_box(cascade.match_factor(black_box(&fallback_dep), &attrs));
    });

    let miss_dep = MarketDependency::Spot {
        id: "EQUITY".into(),
    };
    bench_iter(&mut group, "miss_all_stages", || {
        let _ = black_box(cascade.match_factor(black_box(&miss_dep), &attrs));
    });

    group.finish();
}

fn bench_credit_calibration(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_calibration");
    group.sample_size(10);

    let book = CreditBook::representative();
    group.bench_function("globally_off_full_sample/50x36x3", |b| {
        b.iter_batched(
            || book.inputs.clone(),
            |inputs| {
                black_box(
                    finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                        book.config.clone(),
                    )
                    .calibrate(inputs)
                    .unwrap(),
                )
            },
            BatchSize::SmallInput,
        )
    });

    let diagonal = book.clone().with_strategy(CovarianceStrategy::Diagonal);
    group.bench_function("globally_off_diagonal/50x36x3", |b| {
        b.iter_batched(
            || diagonal.inputs.clone(),
            |inputs| {
                black_box(
                    finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                        diagonal.config.clone(),
                    )
                    .calibrate(inputs)
                    .unwrap(),
                )
            },
            BatchSize::SmallInput,
        )
    });

    let ledoit = book.clone().with_strategy(CovarianceStrategy::LedoitWolf);
    group.bench_function("globally_off_ledoit_wolf/50x36x3", |b| {
        b.iter_batched(
            || ledoit.inputs.clone(),
            |inputs| {
                black_box(
                    finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                        ledoit.config.clone(),
                    )
                    .calibrate(inputs)
                    .unwrap(),
                )
            },
            BatchSize::SmallInput,
        )
    });

    let dts = book.clone().with_dts();
    group.bench_function("globally_off_dts/50x36x3", |b| {
        b.iter_batched(
            || dts.inputs.clone(),
            |inputs| {
                black_box(
                    finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                        dts.config.clone(),
                    )
                    .calibrate(inputs)
                    .unwrap(),
                )
            },
            BatchSize::SmallInput,
        )
    });

    let issuer_beta = book.clone().with_issuer_beta();
    group.bench_function("issuer_beta_full_sample/50x36x3", |b| {
        b.iter_batched(
            || issuer_beta.inputs.clone(),
            |inputs| {
                black_box(
                    finstack_quant_models::factor::credit::calibration::CreditCalibrator::new(
                        issuer_beta.config.clone(),
                    )
                    .calibrate(inputs)
                    .unwrap(),
                )
            },
            BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_credit_decomposition(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_decomposition");
    group.sample_size(20);

    let book = CreditBook::representative();
    let model = book.calibrate();
    let as_of = book.inputs.as_of;
    let generic = *book.inputs.generic_factor.values.last().unwrap();
    let spreads = &book.inputs.as_of_spreads;

    group.bench_function("decompose_levels/50", |b| {
        b.iter(|| {
            black_box(decompose_levels(&model, black_box(spreads), generic, as_of, None).unwrap())
        })
    });

    let t0 = decompose_levels(&model, spreads, generic, as_of, None).unwrap();
    let mut spreads_t1 = spreads.clone();
    for value in spreads_t1.values_mut() {
        *value += 0.0005;
    }
    let t1_date = as_of.next_day().expect("next day");
    let t1 = decompose_levels(&model, &spreads_t1, generic + 0.0002, t1_date, None).unwrap();

    group.bench_function("decompose_period/50", |b| {
        b.iter(|| black_box(decompose_period(black_box(&t0), black_box(&t1)).unwrap()))
    });

    group.finish();
}

fn bench_credit_hierarchical_matcher(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_hierarchical_matcher");

    let matcher = CreditHierarchicalMatcher::new(credit_hierarchical_config(200));
    let known_dep = credit_dependency("ISSUER-0000-HAZARD");
    let known_attrs = known_issuer_attrs(0);
    group.bench_function("known_issuer/200", |b| {
        b.iter(|| {
            black_box(
                matcher
                    .match_factor(black_box(&known_dep), black_box(&known_attrs))
                    .unwrap(),
            )
        })
    });

    let last_dep = credit_dependency("ISSUER-0199-HAZARD");
    let last_attrs = known_issuer_attrs(199);
    group.bench_function("known_issuer_last/200", |b| {
        b.iter(|| {
            black_box(
                matcher
                    .match_factor(black_box(&last_dep), black_box(&last_attrs))
                    .unwrap(),
            )
        })
    });

    let unknown_dep = credit_dependency("NEWCO-HAZARD");
    let unknown_attrs = unknown_issuer_attrs();
    group.bench_function("unknown_bucket_only/200", |b| {
        b.iter(|| {
            black_box(
                matcher
                    .match_factor(black_box(&unknown_dep), black_box(&unknown_attrs))
                    .unwrap(),
            )
        })
    });

    let miss_dep = MarketDependency::Spot {
        id: "EQUITY".into(),
    };
    group.bench_function("non_credit_miss/200", |b| {
        b.iter(|| {
            black_box(
                matcher
                    .match_factor(black_box(&miss_dep), black_box(&known_attrs))
                    .unwrap(),
            )
        })
    });

    let config = credit_hierarchical_config(200);
    group.bench_function("enumerate_factor_ids/200", |b| {
        b.iter(|| black_box(config.enumerate_factor_ids()))
    });

    group.finish();
}

fn bench_sensitivity_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("sensitivity_matrix");

    let n_pos = 1_000;
    let n_fac = 50;
    let position_ids: Vec<String> = (0..n_pos).map(|i| format!("P{i}")).collect();
    let ids = factor_ids(n_fac);

    group.bench_function("zeros/1000x50", |b| {
        b.iter(|| black_box(SensitivityMatrix::zeros(position_ids.clone(), ids.clone())))
    });

    let mut matrix = zero_sensitivity_matrix(n_pos, n_fac);
    group.bench_function("fill_set_delta/1000x50", |b| {
        b.iter(|| {
            for p in 0..n_pos {
                for f in 0..n_fac {
                    matrix.set_delta(p, f, (p + f) as f64 * 0.01);
                }
            }
            black_box(matrix.as_slice()[0]);
        })
    });

    group.bench_function("all_position_deltas/1000x50", |b| {
        b.iter(|| {
            let mut acc = 0.0;
            for p in 0..n_pos {
                acc += black_box(matrix.position_deltas(p).unwrap())
                    .iter()
                    .sum::<f64>();
            }
            black_box(acc);
        })
    });

    group.bench_function("all_factor_deltas/1000x50", |b| {
        b.iter(|| {
            let cols: Vec<Vec<f64>> = (0..n_fac)
                .map(|f| matrix.factor_deltas(f).unwrap())
                .collect();
            black_box(cols);
        })
    });

    group.finish();
}

fn bench_credit_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_validate");
    let model = CreditBook::representative().calibrate();
    group.bench_function("model_validate/50", |b| {
        b.iter(|| {
            model.validate().unwrap();
            black_box(());
        })
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
    bench_credit_calibration,
    bench_credit_decomposition,
    bench_credit_hierarchical_matcher,
    bench_sensitivity_matrix,
    bench_credit_validate,
);
criterion_main!(benches);
