//! Scenarios benchmarks.
//!
//! Measures performance of critical scenario operations:
//! - Scenario composition and priority-based merging
//! - Market data shocks (FX, equity, curves, vol surfaces, base correlation)
//! - Statement forecast adjustments
//! - Complex multi-operation scenarios
//! - Serde serialization/deserialization
//! - Time roll-forward operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::build_periods;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::scalars::MarketScalar;
use finstack_quant_core::market_data::surfaces::VolSurface;
use finstack_quant_core::market_data::term_structures::BaseCorrelationCurve;
use finstack_quant_core::market_data::term_structures::{DiscountCurve, HazardCurve};
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::fx::FxMatrix;
use finstack_quant_core::money::fx::SimpleFxProvider;
use finstack_quant_core::money::Money;
use finstack_quant_scenarios::{
    Compounding, CurveKind, ExecutionContext, OperationSpec, RateBindingSpec, ScenarioEngine,
    ScenarioSpec, TenorMatchMode, VolSurfaceKind,
};
use finstack_quant_statements::types::{AmountOrScalar, NodeSpec, NodeType};
use finstack_quant_statements::FinancialModelSpec;
use indexmap::{indexmap, IndexMap};
use std::hint::black_box;
use std::sync::Arc;
use time::macros::date;

// ================================
// Helper Functions
// ================================

/// Create a base market context with discount curves, FX, and equity prices
fn create_base_market() -> MarketContext {
    let base = date!(2025 - 01 - 01);

    // USD discount curve
    let usd_curve = DiscountCurve::builder("USD_SOFR")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.99),
            (0.5, 0.98),
            (1.0, 0.96),
            (2.0, 0.92),
            (5.0, 0.82),
            (10.0, 0.65),
            (30.0, 0.35),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    // EUR discount curve
    let eur_curve = DiscountCurve::builder("EUR_ESTR")
        .base_date(base)
        .knots([
            (0.0, 1.0),
            (0.25, 0.985),
            (0.5, 0.97),
            (1.0, 0.95),
            (2.0, 0.91),
            (5.0, 0.81),
            (10.0, 0.64),
            (30.0, 0.34),
        ])
        .interp(InterpStyle::MonotoneConvex)
        .build()
        .unwrap();

    // FX matrix
    let fx_provider = Arc::new({
        let p = SimpleFxProvider::new();
        p.set_quote(Currency::EUR, Currency::USD, 1.10)
            .expect("valid rate");
        p.set_quote(Currency::GBP, Currency::USD, 1.25)
            .expect("valid rate");
        p.set_quote(Currency::JPY, Currency::USD, 0.0067)
            .expect("valid rate");
        p
    });
    let fx_matrix = FxMatrix::new(fx_provider);

    // Vol surface
    let vol_surface = VolSurface::builder("SPX_VOL")
        .expiries(&[0.25, 0.5, 1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.25, 0.20, 0.22])
        .row(&[0.24, 0.19, 0.21])
        .row(&[0.23, 0.18, 0.20])
        .build()
        .unwrap();

    // Base correlation curve
    let base_corr = BaseCorrelationCurve::builder("CDX_IG")
        .knots(vec![(3.0, 0.30), (7.0, 0.50), (10.0, 0.60)])
        .build()
        .unwrap();

    // Hazard curves (credit)
    let hazard_ig = HazardCurve::builder("CDX_IG_HAZARD")
        .base_date(base)
        .recovery_rate(0.40)
        .knots([
            (0.0, 0.0),
            (1.0, 0.01),
            (3.0, 0.015),
            (5.0, 0.02),
            (10.0, 0.025),
        ])
        .build()
        .unwrap();

    let hazard_hy = HazardCurve::builder("CDX_HY_HAZARD")
        .base_date(base)
        .recovery_rate(0.30)
        .knots([
            (0.0, 0.0),
            (1.0, 0.05),
            (3.0, 0.06),
            (5.0, 0.07),
            (10.0, 0.08),
        ])
        .build()
        .unwrap();

    // Credit vol surface
    let credit_vol = VolSurface::builder("CDX_IG_VOL")
        .expiries(&[0.25, 0.5, 1.0])
        .strikes(&[90.0, 100.0, 110.0])
        .row(&[0.35, 0.30, 0.32])
        .row(&[0.34, 0.29, 0.31])
        .row(&[0.33, 0.28, 0.30])
        .build()
        .unwrap();

    MarketContext::new()
        .insert(usd_curve)
        .insert(eur_curve)
        .insert_fx(fx_matrix)
        .insert_surface(vol_surface)
        .insert_surface(credit_vol)
        .insert(base_corr)
        .insert(hazard_ig)
        .insert(hazard_hy)
        .insert_price("SPY", MarketScalar::Price(Money::new(450.0, Currency::USD)))
        .insert_price("QQQ", MarketScalar::Price(Money::new(380.0, Currency::USD)))
        .insert_price("EWU", MarketScalar::Price(Money::new(32.0, Currency::USD)))
}

/// Create a financial model with multiple nodes
fn create_financial_model() -> FinancialModelSpec {
    let period_plan = build_periods("2025Q1..2026Q4", None).unwrap();
    let periods = period_plan.periods;
    let mut model = FinancialModelSpec::new("test_model", periods.clone());

    // Revenue node
    let mut revenue_values = IndexMap::new();
    for (i, period) in periods.iter().enumerate() {
        revenue_values.insert(
            period.id,
            AmountOrScalar::Scalar(1_000_000.0 * (1.0 + i as f64 * 0.05)),
        );
    }
    model.add_node(NodeSpec::new("Revenue", NodeType::Value).with_values(revenue_values));

    // COGS node
    let mut cogs_values = IndexMap::new();
    for (i, period) in periods.iter().enumerate() {
        cogs_values.insert(
            period.id,
            AmountOrScalar::Scalar(600_000.0 * (1.0 + i as f64 * 0.04)),
        );
    }
    model.add_node(NodeSpec::new("COGS", NodeType::Value).with_values(cogs_values));

    // Interest rate node
    let mut rate_values = IndexMap::new();
    for period in &periods {
        rate_values.insert(period.id, AmountOrScalar::Scalar(0.045));
    }
    model.add_node(NodeSpec::new("InterestRate", NodeType::Value).with_values(rate_values));

    model
}

/// Create multiple scenarios for composition testing
fn create_scenarios_for_composition(count: usize) -> Vec<ScenarioSpec> {
    let mut scenarios = Vec::with_capacity(count);

    for i in 0..count {
        let scenario = ScenarioSpec {
            id: format!("scenario_{}", i),
            name: Some(format!("Test Scenario {}", i)),
            description: Some(format!("Benchmark scenario number {}", i)),
            operations: vec![
                OperationSpec::CurveParallelBp {
                    curve_kind: CurveKind::Discount,
                    curve_id: "USD_SOFR".into(),
                    discount_curve_id: None,
                    bp: (i as f64 + 1.0) * 10.0,
                },
                OperationSpec::EquityPricePct {
                    ids: vec!["SPY".into()],
                    pct: -(i as f64 + 1.0) * 2.0,
                },
            ],
            priority: (i % 3) as i32,
            resolution_mode: Default::default(),
        };
        scenarios.push(scenario);
    }

    scenarios
}

// ================================
// Benchmark Functions
// ================================

fn bench_scenario_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("scenario_composition");

    for scenario_count in [2, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_scenarios", scenario_count)),
            &scenario_count,
            |b, &count| {
                let engine = ScenarioEngine::new();
                // Pre-create scenarios outside `b.iter` so the benchmark
                // measures composition only (not scenario allocation).
                let scenarios = create_scenarios_for_composition(count);
                b.iter(|| {
                    black_box(
                        engine
                            .try_compose(black_box(scenarios.clone()))
                            .expect("compose should succeed"),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_curve_parallel_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("curve_parallel_shock");

    let base_date = date!(2025 - 01 - 01);
    let scenario = ScenarioSpec {
        id: "curve_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 50.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("single_curve", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_curve_node_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("curve_node_shock");

    let base_date = date!(2025 - 01 - 01);

    for num_nodes in [2, 5, 10] {
        let mut nodes = Vec::new();
        for i in 0..num_nodes {
            let tenor = format!("{}Y", i + 1);
            let bp = (i as f64 + 1.0) * 5.0;
            nodes.push((tenor, bp));
        }

        let scenario = ScenarioSpec {
            id: "node_shock".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::CurveNodeBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                nodes: nodes.clone(),
                match_mode: TenorMatchMode::Interpolate,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_nodes", num_nodes)),
            &scenario,
            |b, scenario| {
                let engine = ScenarioEngine::new();
                b.iter(|| {
                    let mut market = create_base_market();
                    let mut model = FinancialModelSpec::new("test", vec![]);
                    let mut ctx = ExecutionContext {
                        market: &mut market,
                        model: Some(&mut model),
                        instruments: None,
                        rate_bindings: None,
                        calendar: None,
                        as_of: base_date,
                    };
                    black_box(
                        engine
                            .apply(black_box(scenario), black_box(&mut ctx))
                            .unwrap(),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_fx_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("fx_shock");

    let base_date = date!(2025 - 01 - 01);
    let scenario = ScenarioSpec {
        id: "fx_shock".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::MarketFxPct {
            base: Currency::EUR,
            quote: Currency::USD,
            pct: 5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("single_pair", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_equity_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("equity_shock");

    let base_date = date!(2025 - 01 - 01);

    for num_equities in [1, 3, 5] {
        let ids: Vec<String> = (0..num_equities).map(|i| format!("EQUITY_{}", i)).collect();

        let scenario = ScenarioSpec {
            id: "equity_shock".into(),
            name: None,
            description: None,
            operations: vec![OperationSpec::EquityPricePct {
                ids: ids.clone(),
                pct: -10.0,
            }],
            priority: 0,
            resolution_mode: Default::default(),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_equities", num_equities)),
            &scenario,
            |b, scenario| {
                let engine = ScenarioEngine::new();
                b.iter(|| {
                    let mut market = create_base_market();
                    let mut model = FinancialModelSpec::new("test", vec![]);
                    let mut ctx = ExecutionContext {
                        market: &mut market,
                        model: Some(&mut model),
                        instruments: None,
                        rate_bindings: None,
                        calendar: None,
                        as_of: base_date,
                    };
                    black_box(
                        engine
                            .apply(black_box(scenario), black_box(&mut ctx))
                            .unwrap(),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_vol_surface_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("vol_surface_shock");

    let base_date = date!(2025 - 01 - 01);

    // Parallel shock
    let parallel_scenario = ScenarioSpec {
        id: "vol_parallel".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::VolSurfaceParallelPct {
            surface_kind: VolSurfaceKind::Equity,
            surface_id: "SPX_VOL".into(),
            pct: 10.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("parallel", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&parallel_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    // Bucket shock
    let bucket_scenario = ScenarioSpec {
        id: "vol_bucket".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::VolSurfaceBucketPct {
            surface_kind: VolSurfaceKind::Equity,
            surface_id: "SPX_VOL".into(),
            tenors: Some(vec!["1M".into(), "3M".into()]),
            strikes: Some(vec![90.0, 100.0, 110.0]),
            pct: 15.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("bucket", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&bucket_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_base_correlation_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("base_correlation_shock");

    let base_date = date!(2025 - 01 - 01);

    // Parallel shock
    let parallel_scenario = ScenarioSpec {
        id: "basecorr_parallel".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::BaseCorrParallelPts {
            surface_id: "CDX_IG".into(),
            points: 0.05,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("parallel", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&parallel_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    // Bucket shock
    let bucket_scenario = ScenarioSpec {
        id: "basecorr_bucket".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::BaseCorrBucketPts {
            surface_id: "CDX_IG".into(),
            detachment_bps: Some(vec![300, 700]),
            maturities: None,
            points: 0.03,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("bucket", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&bucket_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_statement_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("statement_operations");

    let base_date = date!(2025 - 01 - 01);

    // Percent change
    let percent_scenario = ScenarioSpec {
        id: "stmt_percent".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastPercent {
            node_id: "Revenue".into(),
            pct: -5.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("forecast_percent", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = create_financial_model();
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&percent_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    // Value assign
    let assign_scenario = ScenarioSpec {
        id: "stmt_assign".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::StmtForecastAssign {
            node_id: "Revenue".into(),
            value: 1_500_000.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("forecast_assign", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = create_financial_model();
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&assign_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_complex_multi_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_multi_operation");

    let base_date = date!(2025 - 01 - 01);

    for num_ops in [5, 10, 20] {
        let mut operations = Vec::new();

        // Mix of different operation types
        for i in 0..num_ops {
            match i % 5 {
                0 => operations.push(OperationSpec::CurveParallelBp {
                    curve_kind: CurveKind::Discount,
                    curve_id: if i % 2 == 0 {
                        "USD_SOFR".into()
                    } else {
                        "EUR_ESTR".into()
                    },
                    discount_curve_id: None,
                    bp: (i as f64 + 1.0) * 5.0,
                }),
                1 => operations.push(OperationSpec::EquityPricePct {
                    ids: vec!["SPY".into()],
                    pct: -(i as f64 + 1.0),
                }),
                2 => operations.push(OperationSpec::MarketFxPct {
                    base: Currency::EUR,
                    quote: Currency::USD,
                    pct: (i as f64 + 1.0) * 0.5,
                }),
                3 => operations.push(OperationSpec::StmtForecastPercent {
                    node_id: "Revenue".into(),
                    pct: (i as f64 + 1.0) * 2.0,
                }),
                4 => operations.push(OperationSpec::VolSurfaceParallelPct {
                    surface_kind: VolSurfaceKind::Equity,
                    surface_id: "SPX_VOL".into(),
                    pct: (i as f64 + 1.0) * 3.0,
                }),
                _ => unreachable!(),
            }
        }

        let scenario = ScenarioSpec {
            id: format!("complex_{}", num_ops),
            name: Some(format!("Complex {} ops", num_ops)),
            description: None,
            operations,
            priority: 0,
            resolution_mode: Default::default(),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}_operations", num_ops)),
            &scenario,
            |b, scenario| {
                let engine = ScenarioEngine::new();
                b.iter(|| {
                    let mut market = create_base_market();
                    let mut model = create_financial_model();
                    let mut ctx = ExecutionContext {
                        market: &mut market,
                        model: Some(&mut model),
                        instruments: None,
                        rate_bindings: None,
                        calendar: None,
                        as_of: base_date,
                    };
                    black_box(
                        engine
                            .apply(black_box(scenario), black_box(&mut ctx))
                            .unwrap(),
                    )
                });
            },
        );
    }

    group.finish();
}

fn bench_serde_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("serde_roundtrip");

    let scenario = ScenarioSpec {
        id: "serde_test".into(),
        name: Some("Serde Benchmark".into()),
        description: Some("Testing serialization performance".into()),
        operations: vec![
            // Rates
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: "USD_SOFR".into(),
                discount_curve_id: None,
                bp: 50.0,
            },
            // Credit hazard curves
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "CDX_IG_HAZARD".into(),
                discount_curve_id: None,
                bp: 75.0,
            },
            OperationSpec::CurveNodeBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "CDX_HY_HAZARD".into(),
                discount_curve_id: None,
                nodes: vec![("3Y".into(), 100.0), ("5Y".into(), 150.0)],
                match_mode: TenorMatchMode::Interpolate,
            },
            // FX
            OperationSpec::MarketFxPct {
                base: Currency::EUR,
                quote: Currency::USD,
                pct: 5.0,
            },
            // Equity
            OperationSpec::EquityPricePct {
                ids: vec!["SPY".into(), "QQQ".into(), "EWU".into()],
                pct: -10.0,
            },
            // Equity vol
            OperationSpec::VolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Equity,
                surface_id: "SPX_VOL".into(),
                pct: 15.0,
            },
            // Credit vol
            OperationSpec::VolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Credit,
                surface_id: "CDX_IG_VOL".into(),
                pct: 20.0,
            },
            // Base correlation
            OperationSpec::BaseCorrParallelPts {
                surface_id: "CDX_IG".into(),
                points: 0.05,
            },
            // Credit spreads
            OperationSpec::InstrumentSpreadBpByType {
                instrument_types: vec![finstack_quant_valuations::pricer::InstrumentType::CDS],
                bp: 100.0,
            },
            // Statements
            OperationSpec::StmtForecastPercent {
                node_id: "Revenue".into(),
                pct: -5.0,
            },
            OperationSpec::StmtForecastAssign {
                node_id: "COGS".into(),
                value: 750_000.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("serialize", |b| {
        b.iter(|| black_box(serde_json::to_string(black_box(&scenario)).unwrap()));
    });

    let json = serde_json::to_string(&scenario).unwrap();

    group.bench_function("deserialize", |b| {
        b.iter(|| black_box(serde_json::from_str::<ScenarioSpec>(black_box(&json)).unwrap()));
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_string(black_box(&scenario)).unwrap();
            black_box(serde_json::from_str::<ScenarioSpec>(&json).unwrap())
        });
    });

    group.finish();
}

fn bench_rate_bindings(c: &mut Criterion) {
    let mut group = c.benchmark_group("rate_bindings");

    let base_date = date!(2025 - 01 - 01);

    let rate_bindings = Some(indexmap! {
        "InterestRate".into() => RateBindingSpec {
            node_id: "InterestRate".into(),
            curve_id: "USD_SOFR".into(),
            tenor: "1Y".to_string(),
            compounding: Compounding::Continuous,
            day_count: None,
        },
    });

    let scenario = ScenarioSpec {
        id: "rate_bindings".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::Discount,
            curve_id: "USD_SOFR".into(),
            discount_curve_id: None,
            bp: 100.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("with_rate_bindings", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = create_financial_model();
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: rate_bindings.clone(),
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_hazard_curve_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("hazard_curve_shock");

    let base_date = date!(2025 - 01 - 01);

    // Parallel hazard shock
    let parallel_scenario = ScenarioSpec {
        id: "hazard_parallel".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "CDX_IG_HAZARD".into(),
            discount_curve_id: None,
            bp: 50.0, // +50bp widening
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("parallel_ig", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&parallel_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    // Node-specific hazard shock (key rate bumps)
    let node_scenario = ScenarioSpec {
        id: "hazard_node".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::CurveNodeBp {
            curve_kind: CurveKind::ParCDS,
            curve_id: "CDX_HY_HAZARD".into(),
            discount_curve_id: None,
            nodes: vec![("3Y".into(), 100.0), ("5Y".into(), 150.0)],
            match_mode: TenorMatchMode::Interpolate,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("node_hy", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&node_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_credit_vol_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_vol_shock");

    let base_date = date!(2025 - 01 - 01);

    // Parallel credit vol shock
    let parallel_scenario = ScenarioSpec {
        id: "credit_vol_parallel".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::VolSurfaceParallelPct {
            surface_kind: VolSurfaceKind::Credit,
            surface_id: "CDX_IG_VOL".into(),
            pct: 20.0, // +20% credit vol increase
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("parallel", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&parallel_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    // Bucket credit vol shock (specific tenors)
    let bucket_scenario = ScenarioSpec {
        id: "credit_vol_bucket".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::VolSurfaceBucketPct {
            surface_kind: VolSurfaceKind::Credit,
            surface_id: "CDX_IG_VOL".into(),
            tenors: Some(vec!["3M".into(), "1Y".into()]),
            strikes: Some(vec![90.0, 100.0]),
            pct: 25.0,
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("bucket", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&bucket_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_instrument_spread_shock(c: &mut Criterion) {
    let mut group = c.benchmark_group("instrument_spread_shock");

    let base_date = date!(2025 - 01 - 01);

    // Note: This benchmark tests the operation application even without actual instruments
    // In production, instruments would be provided via ExecutionContext

    // Spread shock by instrument type
    let type_scenario = ScenarioSpec {
        id: "spread_by_type".into(),
        name: None,
        description: None,
        operations: vec![OperationSpec::InstrumentSpreadBpByType {
            instrument_types: vec![
                finstack_quant_valuations::pricer::InstrumentType::CDS,
                finstack_quant_valuations::pricer::InstrumentType::Bond,
            ],
            bp: 100.0, // +100bp spread widening
        }],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("by_type", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None, // Would contain CDS/Bond instruments in real scenario
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            // This will generate a warning but tests the path
            black_box(
                engine
                    .apply(black_box(&type_scenario), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

fn bench_comprehensive_credit_scenario(c: &mut Criterion) {
    let mut group = c.benchmark_group("comprehensive_credit_scenario");

    let base_date = date!(2025 - 01 - 01);

    // Multi-operation credit stress scenario
    let credit_stress = ScenarioSpec {
        id: "credit_stress".into(),
        name: Some("Credit Market Stress".into()),
        description: Some("Comprehensive credit shock with hazard, vol, and correlation".into()),
        operations: vec![
            // Widen hazard rates
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "CDX_IG_HAZARD".into(),
                discount_curve_id: None,
                bp: 75.0,
            },
            OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::ParCDS,
                curve_id: "CDX_HY_HAZARD".into(),
                discount_curve_id: None,
                bp: 200.0,
            },
            // Increase credit vol
            OperationSpec::VolSurfaceParallelPct {
                surface_kind: VolSurfaceKind::Credit,
                surface_id: "CDX_IG_VOL".into(),
                pct: 30.0,
            },
            // Increase correlation (contagion)
            OperationSpec::BaseCorrParallelPts {
                surface_id: "CDX_IG".into(),
                points: 0.15,
            },
            // Spreads widen
            OperationSpec::InstrumentSpreadBpByType {
                instrument_types: vec![finstack_quant_valuations::pricer::InstrumentType::CDS],
                bp: 150.0,
            },
        ],
        priority: 0,
        resolution_mode: Default::default(),
    };

    group.bench_function("credit_stress", |b| {
        let engine = ScenarioEngine::new();
        b.iter(|| {
            let mut market = create_base_market();
            let mut model = FinancialModelSpec::new("test", vec![]);
            let mut ctx = ExecutionContext {
                market: &mut market,
                model: Some(&mut model),
                instruments: None,
                rate_bindings: None,
                calendar: None,
                as_of: base_date,
            };
            black_box(
                engine
                    .apply(black_box(&credit_stress), black_box(&mut ctx))
                    .unwrap(),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scenario_composition,
    bench_curve_parallel_shock,
    bench_curve_node_shock,
    bench_fx_shock,
    bench_equity_shock,
    bench_vol_surface_shock,
    bench_base_correlation_shock,
    bench_statement_operations,
    bench_complex_multi_operation,
    bench_serde_roundtrip,
    bench_rate_bindings,
    bench_hazard_curve_shock,
    bench_credit_vol_shock,
    bench_instrument_spread_shock,
    bench_comprehensive_credit_scenario,
);

criterion_main!(benches);
