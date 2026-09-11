//! Scaling guards for `finstack-quant-scenarios`.
//!
//! Complements `scenarios.rs` (absolute cost at one size) by measuring how cost
//! grows with operation count, curve count, and book size. Read ns-per-element
//! across sizes: flat is linear; rising means a super-linear term is back.
//! The credit replay case also measures exact historical quote recalibration.

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[path = "support/fixtures.rs"]
mod fixtures;

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use finstack_quant_core::market_data::hierarchy::HierarchyTarget;
use finstack_quant_scenarios::{
    CurveKind, InstrumentType, OperationSpec, ScenarioEngine, ScenarioSpec, TimeRollMode,
};
use fixtures::{
    apply_market, apply_with_instruments, compose_specs, hierarchy_hazard_market, hierarchy_market,
    lean_market, sample_bonds, spec, USD_SOFR,
};

fn scaling_same_curve_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_same_curve_ops");
    let market = lean_market();
    let engine = ScenarioEngine::new();

    for n in [1_usize, 8, 24, 48] {
        let operations = (0..n)
            .map(|i| OperationSpec::CurveParallelBp {
                curve_kind: CurveKind::Discount,
                curve_id: USD_SOFR.into(),
                discount_curve_id: None,
                bp: 1.0 + i as f64,
            })
            .collect();
        let scenario = spec(&format!("same_curve_{n}"), operations);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(apply_market(&engine, &scenario, &market)));
        });
    }
    group.finish();
}

fn scaling_hierarchy_curves(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_hierarchy_curves");
    let engine = ScenarioEngine::new();
    let target = HierarchyTarget {
        path: vec!["Rates".into(), "USD".into()],
        tag_filter: None,
    };
    let scenario = spec(
        "hier_all",
        vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::Discount,
            target,
            bp: 10.0,
            discount_curve_id: None,
        }],
    );

    for n in [16_usize, 64, 128] {
        let market = hierarchy_market(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(apply_market(&engine, &scenario, &market)));
        });
    }
    group.finish();
}

fn scaling_hierarchy_par_cds(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_hierarchy_par_cds");
    group.sample_size(10);
    let engine = ScenarioEngine::new();
    let target = HierarchyTarget {
        path: vec!["Credit".into(), "USD".into()],
        tag_filter: None,
    };
    let scenario = spec(
        "hier_par_cds",
        vec![OperationSpec::HierarchyCurveParallelBp {
            curve_kind: CurveKind::ParCDS,
            target,
            bp: 10.0,
            discount_curve_id: Some(USD_SOFR.into()),
        }],
    );

    for n in [2_usize, 4, 8] {
        let market = hierarchy_hazard_market(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(apply_market(&engine, &scenario, &market)));
        });
    }
    group.finish();
}

fn historical_credit_quote_replay(c: &mut Criterion) {
    use finstack_quant_calibration::{
        api::{engine, schema::CalibrationEnvelope},
        recalibration::CachedRecalibrationProvider,
    };
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_valuations::metrics::risk::{
        MarketScenario, RiskFactorShift, RiskFactorType,
    };

    let envelope: CalibrationEnvelope = serde_json::from_str(include_str!(
        "../../calibration/examples/market_bootstrap/03_single_name_hazard.json"
    ))
    .expect("calibration inputs");
    let calibrated = engine::execute(&envelope).expect("source calibration");
    let market = MarketContext::try_from(calibrated.result.final_market).expect("market");
    let hazard = market.get_hazard("ISSUER-A-CDS").expect("hazard");
    let recipe = hazard.hazard_calibration().expect("replay recipe");
    let scenario = MarketScenario::new(
        hazard.base_date(),
        vec![RiskFactorShift {
            factor: RiskFactorType::CreditSpread {
                curve_id: "ISSUER-A-CDS".into(),
                tenor_years: recipe.spread_risk_inputs[0].pillar_time,
            },
            shift: 0.001,
        }],
    );
    let provider = CachedRecalibrationProvider::new();
    c.bench_function("historical_credit_quote_replay_10bp/cached_result", |b| {
        b.iter(|| {
            black_box(
                scenario
                    .apply(black_box(&market), Some(&provider))
                    .expect("exact quote replay"),
            )
        });
    });
    c.bench_function("historical_credit_quote_replay_10bp/fresh_provider", |b| {
        b.iter(|| {
            let provider = CachedRecalibrationProvider::new();
            black_box(
                scenario
                    .apply(black_box(&market), Some(&provider))
                    .expect("uncached exact quote replay"),
            )
        });
    });
}

fn scaling_instrument_spread(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_instrument_spread");
    let market = lean_market();
    let engine = ScenarioEngine::new();
    let scenario = spec(
        "spread",
        vec![OperationSpec::InstrumentSpreadBpByType {
            instrument_types: vec![InstrumentType::Bond],
            bp: 25.0,
        }],
    );

    for n in [50_usize, 200, 500] {
        let bonds = sample_bonds(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(apply_with_instruments(&engine, &scenario, &market, &bonds)));
        });
    }
    group.finish();
}

fn scaling_time_roll_instruments(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_time_roll_instruments");
    group.sample_size(10);
    let market = lean_market();
    let engine = ScenarioEngine::new();
    let scenario = spec(
        "roll",
        vec![OperationSpec::TimeRollForward {
            period: "1M".into(),
            apply_shocks: false,
            roll_mode: TimeRollMode::CalendarDays,
        }],
    );

    for n in [10_usize, 40, 80] {
        let bonds = sample_bonds(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| black_box(apply_with_instruments(&engine, &scenario, &market, &bonds)));
        });
    }
    group.finish();
}

fn scaling_compose(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling_compose");

    for n in [10_usize, 50, 200] {
        let scenarios = compose_specs(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                black_box(ScenarioSpec::compose(black_box(scenarios.clone())).expect("compose"))
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    scaling_same_curve_ops,
    scaling_hierarchy_curves,
    scaling_hierarchy_par_cds,
    historical_credit_quote_replay,
    scaling_instrument_spread,
    scaling_time_roll_instruments,
    scaling_compose,
);

criterion_main!(benches);
