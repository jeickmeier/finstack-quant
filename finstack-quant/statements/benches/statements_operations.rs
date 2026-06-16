//! Statement operations benchmarks.
//!
//! Measures performance of statement modeling operations:
//! - Model building (periods, nodes, formulas)
//! - Model evaluation (simple to complex models)
//! - DSL parsing and compilation
//! - Forecast methods (forward fill, growth, seasonal, etc.)
//! - Registry operations
//! - Capital structure integration
//! - Results export (table conversion)
//!
//! Covers the main public APIs of finstack-quant-statements to track performance
//! characteristics and regression over time.

use criterion::{criterion_group, criterion_main, Criterion};
use finstack_quant_statements::prelude::*;
use indexmap::IndexMap;
use std::hint::black_box;

// ============================================================================
// Model Building Benchmarks
// ============================================================================

fn bench_model_building(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_building");

    // Simple model with value nodes
    group.bench_function("simple_value_model", |b| {
        b.iter(|| {
            ModelBuilder::new("test")
                .periods("2025Q1..Q4", None)
                .unwrap()
                .value(
                    "revenue",
                    &[
                        (
                            PeriodId::quarter(2025, 1),
                            AmountOrScalar::scalar(100_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 2),
                            AmountOrScalar::scalar(110_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 3),
                            AmountOrScalar::scalar(120_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 4),
                            AmountOrScalar::scalar(130_000.0),
                        ),
                    ],
                )
                .build()
                .unwrap()
        })
    });

    // Model with computed nodes
    group.bench_function("computed_nodes_model", |b| {
        b.iter(|| {
            ModelBuilder::new("test")
                .periods("2025Q1..Q4", None)
                .unwrap()
                .value(
                    "revenue",
                    &[
                        (
                            PeriodId::quarter(2025, 1),
                            AmountOrScalar::scalar(100_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 2),
                            AmountOrScalar::scalar(110_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 3),
                            AmountOrScalar::scalar(120_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 4),
                            AmountOrScalar::scalar(130_000.0),
                        ),
                    ],
                )
                .compute("cogs", "revenue * 0.6")
                .unwrap()
                .compute("gross_profit", "revenue - cogs")
                .unwrap()
                .compute("opex", "revenue * 0.2")
                .unwrap()
                .compute("ebitda", "gross_profit - opex")
                .unwrap()
                .build()
                .unwrap()
        })
    });

    // Large model with many nodes
    group.bench_function("large_model_50_nodes", |b| {
        b.iter(|| {
            let mut builder = ModelBuilder::new("test")
                .periods("2025Q1..Q4", None)
                .unwrap()
                .value(
                    "revenue",
                    &[
                        (
                            PeriodId::quarter(2025, 1),
                            AmountOrScalar::scalar(100_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 2),
                            AmountOrScalar::scalar(110_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 3),
                            AmountOrScalar::scalar(120_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 4),
                            AmountOrScalar::scalar(130_000.0),
                        ),
                    ],
                );

            // Add 49 computed nodes (50 total including revenue)
            for i in 1..50 {
                builder = builder
                    .compute(
                        format!("metric_{}", i),
                        format!("revenue * {}", 0.01 * i as f64),
                    )
                    .unwrap();
            }

            builder.build().unwrap()
        })
    });

    group.finish();
}

// ============================================================================
// Model Evaluation Benchmarks
// ============================================================================

fn bench_model_evaluation(c: &mut Criterion) {
    let mut group = c.benchmark_group("model_evaluation");

    // Simple value-only model
    let simple_model = ModelBuilder::new("simple")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .build()
        .unwrap();

    group.bench_function("evaluate_value_only", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&simple_model).unwrap())
        })
    });

    // Model with calculations
    let calc_model = ModelBuilder::new("calc")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("opex", "revenue * 0.2")
        .unwrap()
        .compute("ebitda", "gross_profit - opex")
        .unwrap()
        .build()
        .unwrap();

    group.bench_function("evaluate_with_calculations", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&calc_model).unwrap())
        })
    });

    // P&L model with time-series functions
    let ts_model = ModelBuilder::new("timeseries")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("opex", "revenue * 0.2")
        .unwrap()
        .compute("ebitda", "gross_profit - opex")
        .unwrap()
        .compute("revenue_growth", "pct_change(revenue)")
        .unwrap()
        .compute("revenue_qoq", "diff(revenue)")
        .unwrap()
        .compute("revenue_ttm", "rolling_sum(revenue, 4)")
        .unwrap()
        .compute("revenue_avg_3q", "rolling_mean(revenue, 3)")
        .unwrap()
        .build()
        .unwrap();

    group.bench_function("evaluate_with_timeseries", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&ts_model).unwrap())
        })
    });

    // Large model
    let mut large_builder = ModelBuilder::new("large")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        );

    for i in 1..50 {
        large_builder = large_builder
            .compute(
                format!("metric_{}", i),
                format!("revenue * {}", 0.01 * i as f64),
            )
            .unwrap();
    }
    let large_model = large_builder.build().unwrap();

    group.bench_function("evaluate_50_nodes", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&large_model).unwrap())
        })
    });

    // Model with many periods
    let mut monthly_values = Vec::new();
    for m in 1..=24 {
        monthly_values.push((
            PeriodId::month(2025 + (m - 1) / 12, (((m - 1) % 12) + 1) as u8),
            AmountOrScalar::scalar(100_000.0 + m as f64 * 1000.0),
        ));
    }

    let monthly_model = ModelBuilder::new("monthly")
        .periods("2025M01..2026M12", None)
        .unwrap()
        .value("revenue", &monthly_values)
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("revenue_ttm", "rolling_sum(revenue, 12)")
        .unwrap()
        .build()
        .unwrap();

    group.bench_function("evaluate_24_periods", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&monthly_model).unwrap())
        })
    });

    group.finish();
}

// ============================================================================
// DSL Parsing and Compilation Benchmarks
// ============================================================================

fn bench_dsl_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("dsl_operations");

    group.bench_function("parse_simple_formula", |b| {
        b.iter(|| {
            black_box(
                finstack_quant_statements::dsl::parser::parse_formula("revenue * 0.6").unwrap(),
            )
        })
    });

    group.bench_function("parse_complex_formula", |b| {
        b.iter(|| {
            black_box(
                finstack_quant_statements::dsl::parser::parse_formula(
                    "(revenue - cogs) * (1 - tax_rate) + interest_income",
                )
                .unwrap(),
            )
        })
    });

    group.bench_function("parse_timeseries_formula", |b| {
        b.iter(|| {
            black_box(
                finstack_quant_statements::dsl::parser::parse_formula(
                    "rolling_mean(revenue, 4) * pct_change(revenue) + lag(revenue, 1)",
                )
                .unwrap(),
            )
        })
    });

    // Compile to core expression
    let ast = finstack_quant_statements::dsl::parser::parse_formula("revenue * 0.6").unwrap();

    group.bench_function("compile_simple_ast", |b| {
        b.iter(|| black_box(finstack_quant_statements::dsl::compiler::compile(&ast).unwrap()))
    });

    let complex_ast = finstack_quant_statements::dsl::parser::parse_formula(
        "(revenue - cogs) * (1 - tax_rate) + interest_income",
    )
    .unwrap();

    group.bench_function("compile_complex_ast", |b| {
        b.iter(|| {
            black_box(finstack_quant_statements::dsl::compiler::compile(&complex_ast).unwrap())
        })
    });

    group.finish();
}

// ============================================================================
// Forecast Method Benchmarks
// ============================================================================

fn bench_forecast_methods(c: &mut Criterion) {
    let mut group = c.benchmark_group("forecast_methods");

    // Forward fill
    let ff_model = ModelBuilder::new("forward_fill")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .forecast("revenue", ForecastSpec::forward_fill())
        .build()
        .unwrap();

    group.bench_function("forecast_forward_fill", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&ff_model).unwrap())
        })
    });

    // Growth rate
    let growth_model = ModelBuilder::new("growth")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .forecast("revenue", ForecastSpec::growth(0.05))
        .build()
        .unwrap();

    group.bench_function("forecast_growth_rate", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&growth_model).unwrap())
        })
    });

    // Seasonal pattern
    let mut seasonal_params = IndexMap::new();
    seasonal_params.insert(
        "historical".into(),
        serde_json::json!([
            100_000.0, 110_000.0, 120_000.0, 110_000.0, 105_000.0, 115_000.0, 125_000.0, 115_000.0
        ]),
    );
    seasonal_params.insert("season_length".into(), serde_json::json!(4));
    seasonal_params.insert("mode".into(), serde_json::json!("multiplicative"));

    let seasonal_model = ModelBuilder::new("seasonal")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .forecast(
            "revenue",
            ForecastSpec {
                method: ForecastMethod::Seasonal,
                params: seasonal_params,
            },
        )
        .build()
        .unwrap();

    group.bench_function("forecast_seasonal", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&seasonal_model).unwrap())
        })
    });

    // Log-normal distribution (deterministic with seed)
    let lognormal_model = ModelBuilder::new("lognormal")
        .periods("2025Q1..Q4", Some("2025Q1"))
        .unwrap()
        .value(
            "revenue",
            &[(
                PeriodId::quarter(2025, 1),
                AmountOrScalar::scalar(100_000.0),
            )],
        )
        .forecast("revenue", ForecastSpec::lognormal(0.05, 0.10, 42))
        .build()
        .unwrap();

    group.bench_function("forecast_lognormal", |b| {
        b.iter(|| {
            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&lognormal_model).unwrap())
        })
    });

    group.finish();
}

// ============================================================================
// Registry Benchmarks
// ============================================================================

fn bench_registry_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("registry_operations");

    // Load registry
    group.bench_function("create_empty_registry", |b| {
        b.iter(|| black_box(Registry::new()))
    });

    // Load builtins
    group.bench_function("load_builtin_metrics", |b| {
        b.iter(|| {
            let mut registry = Registry::new();
            registry.load_builtins().unwrap();
            black_box(registry)
        })
    });

    // Lookup metrics
    let mut registry = Registry::new();
    registry.load_builtins().unwrap();

    group.bench_function("lookup_metric", |b| {
        b.iter(|| black_box(registry.get("fin.gross_profit").unwrap()))
    });

    group.bench_function("check_metric_exists", |b| {
        b.iter(|| black_box(registry.has("fin.gross_profit")))
    });

    group.finish();
}

// ============================================================================
// Results Export Benchmarks
// ============================================================================

fn bench_results_export(c: &mut Criterion) {
    let mut group = c.benchmark_group("results_export");

    // Small model export
    let small_model = ModelBuilder::new("small")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .build()
        .unwrap();

    let mut evaluator = Evaluator::new();
    let small_results = evaluator.evaluate(&small_model).unwrap();

    group.bench_function("export_to_long_table", |b| {
        b.iter(|| black_box(small_results.to_table_long().unwrap()))
    });

    group.bench_function("export_to_wide_table", |b| {
        b.iter(|| black_box(small_results.to_table_wide().unwrap()))
    });

    // Large model export
    let mut large_builder = ModelBuilder::new("large")
        .periods("2025M01..2026M12", None)
        .unwrap();

    let mut monthly_values = Vec::new();
    for m in 1..=24 {
        monthly_values.push((
            PeriodId::month(2025 + (m - 1) / 12, (((m - 1) % 12) + 1) as u8),
            AmountOrScalar::scalar(100_000.0 + m as f64 * 1000.0),
        ));
    }

    large_builder = large_builder.value("revenue", &monthly_values);

    for i in 1..=20 {
        large_builder = large_builder
            .compute(
                format!("metric_{}", i),
                format!("revenue * {}", 0.05 * i as f64),
            )
            .unwrap();
    }

    let large_model = large_builder.build().unwrap();
    let mut evaluator = Evaluator::new();
    let large_results = evaluator.evaluate(&large_model).unwrap();

    group.bench_function("export_large_to_long_table", |b| {
        b.iter(|| black_box(large_results.to_table_long().unwrap()))
    });

    group.bench_function("export_large_to_wide_table", |b| {
        b.iter(|| black_box(large_results.to_table_wide().unwrap()))
    });

    group.finish();
}

// ============================================================================
// Serialization Benchmarks
// ============================================================================

fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let model = ModelBuilder::new("serialize_test")
        .periods("2025Q1..Q4", None)
        .unwrap()
        .value(
            "revenue",
            &[
                (
                    PeriodId::quarter(2025, 1),
                    AmountOrScalar::scalar(100_000.0),
                ),
                (
                    PeriodId::quarter(2025, 2),
                    AmountOrScalar::scalar(110_000.0),
                ),
                (
                    PeriodId::quarter(2025, 3),
                    AmountOrScalar::scalar(120_000.0),
                ),
                (
                    PeriodId::quarter(2025, 4),
                    AmountOrScalar::scalar(130_000.0),
                ),
            ],
        )
        .compute("cogs", "revenue * 0.6")
        .unwrap()
        .compute("gross_profit", "revenue - cogs")
        .unwrap()
        .compute("opex", "revenue * 0.2")
        .unwrap()
        .compute("ebitda", "gross_profit - opex")
        .unwrap()
        .build()
        .unwrap();

    group.bench_function("serialize_model_to_json", |b| {
        b.iter(|| black_box(serde_json::to_string(&model).unwrap()))
    });

    let json = serde_json::to_string(&model).unwrap();

    group.bench_function("deserialize_model_from_json", |b| {
        b.iter(|| black_box(serde_json::from_str::<FinancialModelSpec>(&json).unwrap()))
    });

    group.finish();
}

// ============================================================================
// Full End-to-End Benchmarks
// ============================================================================

fn bench_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");

    // Simple P&L model
    group.bench_function("simple_pl_model", |b| {
        b.iter(|| {
            let model = ModelBuilder::new("pl")
                .periods("2025Q1..Q4", Some("2025Q2"))
                .unwrap()
                .value(
                    "revenue",
                    &[
                        (
                            PeriodId::quarter(2025, 1),
                            AmountOrScalar::scalar(1_000_000.0),
                        ),
                        (
                            PeriodId::quarter(2025, 2),
                            AmountOrScalar::scalar(1_100_000.0),
                        ),
                    ],
                )
                .forecast("revenue", ForecastSpec::growth(0.05))
                .compute("cogs", "revenue * 0.6")
                .unwrap()
                .compute("gross_profit", "revenue - cogs")
                .unwrap()
                .compute("opex", "revenue * 0.15")
                .unwrap()
                .compute("ebitda", "gross_profit - opex")
                .unwrap()
                .compute("margin", "ebitda / revenue")
                .unwrap()
                .build()
                .unwrap();

            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&model).unwrap())
        })
    });

    // Complex financial model
    group.bench_function("complex_financial_model", |b| {
        b.iter(|| {
            let mut seasonal_params = IndexMap::new();
            seasonal_params.insert(
                "historical".into(),
                serde_json::json!([
                    1_000_000.0,
                    1_100_000.0,
                    1_200_000.0,
                    1_150_000.0,
                    1_050_000.0,
                    1_150_000.0,
                    1_250_000.0,
                    1_200_000.0
                ]),
            );
            seasonal_params.insert("season_length".into(), serde_json::json!(4));
            seasonal_params.insert("mode".into(), serde_json::json!("multiplicative"));

            let model = ModelBuilder::new("complex")
                .periods("2025Q1..Q4", Some("2025Q1"))
                .unwrap()
                .value(
                    "revenue",
                    &[(
                        PeriodId::quarter(2025, 1),
                        AmountOrScalar::scalar(1_000_000.0),
                    )],
                )
                .forecast(
                    "revenue",
                    ForecastSpec {
                        method: ForecastMethod::Seasonal,
                        params: seasonal_params,
                    },
                )
                .compute("cogs", "revenue * 0.6")
                .unwrap()
                .compute("gross_profit", "revenue - cogs")
                .unwrap()
                .compute("opex", "revenue * 0.15")
                .unwrap()
                .compute("depreciation", "10000")
                .unwrap()
                .compute("ebitda", "gross_profit - opex")
                .unwrap()
                .compute("ebit", "ebitda - depreciation")
                .unwrap()
                .compute("interest_expense", "50000")
                .unwrap()
                .compute("ebt", "ebit - interest_expense")
                .unwrap()
                .compute("tax_expense", "ebt * 0.21")
                .unwrap()
                .compute("net_income", "ebt - tax_expense")
                .unwrap()
                .compute("revenue_growth", "pct_change(revenue)")
                .unwrap()
                .compute("revenue_ttm", "rolling_sum(revenue, 4)")
                .unwrap()
                .compute("ebitda_margin", "ebitda / revenue")
                .unwrap()
                .build()
                .unwrap();

            let mut evaluator = Evaluator::new();
            black_box(evaluator.evaluate(&model).unwrap())
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_model_building,
    bench_model_evaluation,
    bench_dsl_operations,
    bench_forecast_methods,
    bench_registry_operations,
    bench_results_export,
    bench_serialization,
    bench_end_to_end
);

criterion_main!(benches);
