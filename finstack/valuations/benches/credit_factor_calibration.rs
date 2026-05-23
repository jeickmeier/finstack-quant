//! Credit factor model calibration benchmarks (PR-12).
//!
//! Measures the end-to-end cost of [`CreditCalibrator::calibrate`] at
//! increasing scales: 10 × 24 months × 1 level, 50 × 36 months × 2 levels,
//! and 500 × 60 months × 3 levels.  All panels are synthetic but structurally
//! representative.
//!
//! Group name: `"credit_factor_calibration"`.
//! Bench IDs: `"n_issuers/<N>"`.
//!
//! These benchmarks are **non-gating**: they compile and run via `cargo bench`
//! but are not wired into CI.

#![allow(clippy::unwrap_used)]

use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput};
use finstack_core::factor_model::credit_hierarchy::{
    CreditHierarchySpec, HierarchyDimension, IssuerBetaPolicy,
};
use finstack_factor_model::{
    BetaShrinkage, BucketSizeThresholds, CovarianceStrategy, PanelSpace, VolModelChoice,
};
use finstack_factor_model::{CreditCalibrationConfig, CreditCalibrationInputs, CreditCalibrator};
use serde_json::{json, Value};
use time::{Date, Month};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const ANNUALIZATION: f64 = 12.0; // monthly → annual

/// Deterministic pseudo-random float in [0, 1) based on two seed ints.
fn det_rand(seed_a: usize, seed_b: usize) -> f64 {
    let x = (seed_a.wrapping_mul(1_664_525).wrapping_add(1_013_904_223))
        ^ (seed_b
            .wrapping_mul(6_364_136_223_846_793_005usize)
            .wrapping_add(1_442_695_040_888_963_407usize));
    (x & 0xFFFF) as f64 / 65535.0
}

/// Generate a sequence of monthly ISO date strings ending at `2024-03-31`.
fn monthly_dates(n: usize) -> Vec<String> {
    let end = Date::from_calendar_date(2024, Month::March, 31).unwrap();
    let mut dates = Vec::with_capacity(n);
    for i in 0..n {
        let approx_days_back = (n - 1 - i) as i64 * 30;
        let d = end - time::Duration::days(approx_days_back);
        dates.push(d.to_string());
    }
    dates
}

fn bucket_label(idx: usize) -> &'static str {
    ["IG", "HY", "EM"][idx % 3]
}
fn region_label(idx: usize) -> &'static str {
    ["NA", "EU", "APAC"][idx % 3]
}
fn sector_label(idx: usize) -> &'static str {
    ["FIN", "UTIL", "TECH", "ENERGY", "HEALTH", "CONS"][idx % 6]
}

/// Build a synthetic `CreditCalibrationInputs` using JSON-then-deserialize so
/// that type IDs and dates are handled by serde (no manual construction needed).
fn build_inputs(n_issuers: usize, n_months: usize, n_levels: usize) -> CreditCalibrationInputs {
    let dates = monthly_dates(n_months);
    let as_of = dates.last().unwrap().clone();

    let generic_values: Vec<f64> = (0..n_months)
        .map(|i| 100.0 + 0.5 * (i as f64 * 0.3_f64).sin())
        .collect();

    let mut spreads = serde_json::Map::new();
    let mut tags_map = serde_json::Map::new();
    let mut asof_spreads = serde_json::Map::new();

    for idx in 0..n_issuers {
        let id = format!("ISSUER-{idx:04}");
        let base = 80.0 + (idx % 200) as f64 * 1.5;
        let beta_pc = 0.4 + (idx % 10) as f64 * 0.06;

        let series: Vec<Value> = (0..n_months)
            .map(|t| {
                let v = base + beta_pc * (generic_values[t] - 100.0) + 2.0 * det_rand(idx, t) - 1.0;
                Value::from(v)
            })
            .collect();

        asof_spreads.insert(
            id.clone(),
            Value::from(series.last().unwrap().as_f64().unwrap()),
        );
        spreads.insert(id.clone(), Value::Array(series));

        let mut tag_row = serde_json::Map::new();
        if n_levels >= 1 {
            tag_row.insert("rating".into(), Value::from(bucket_label(idx)));
        }
        if n_levels >= 2 {
            tag_row.insert("region".into(), Value::from(region_label(idx)));
        }
        if n_levels >= 3 {
            tag_row.insert("sector".into(), Value::from(sector_label(idx)));
        }
        tags_map.insert(id, Value::Object(tag_row));
    }

    let json_val = json!({
        "history_panel": {
            "dates": dates,
            "spreads": spreads,
        },
        "issuer_tags": { "tags": tags_map },
        "generic_factor": {
            "spec": { "name": "CDX IG 5Y", "series_id": "cdx.ig.5y" },
            "values": generic_values,
        },
        "as_of": as_of,
        "asof_spreads": asof_spreads,
        "idiosyncratic_overrides": {},
    });

    serde_json::from_value(json_val).expect("inputs deserialization should succeed")
}

fn build_config(n_levels: usize) -> CreditCalibrationConfig {
    let mut levels: Vec<HierarchyDimension> = vec![];
    if n_levels >= 1 {
        levels.push(HierarchyDimension::Rating);
    }
    if n_levels >= 2 {
        levels.push(HierarchyDimension::Region);
    }
    if n_levels >= 3 {
        levels.push(HierarchyDimension::Sector);
    }

    CreditCalibrationConfig {
        policy: IssuerBetaPolicy::GloballyOff,
        hierarchy: CreditHierarchySpec { levels },
        min_bucket_size_per_level: BucketSizeThresholds {
            per_level: (0..n_levels).map(|_| 2usize).collect(),
        },
        vol_model: VolModelChoice::Sample,
        covariance_strategy: CovarianceStrategy::Diagonal,
        beta_shrinkage: BetaShrinkage::None,
        use_returns_or_levels: PanelSpace::Returns,
        annualization_factor: ANNUALIZATION,
    }
}

// ---------------------------------------------------------------------------
// Benchmark entry point
// ---------------------------------------------------------------------------

const BENCH_SCENARIOS: &[(usize, usize, usize)] = &[
    (10, 24, 1),  // tiny
    (50, 36, 2),  // medium
    (500, 60, 3), // large (spec requirement)
];

fn bench_credit_factor_calibration(c: &mut Criterion) {
    let mut group = c.benchmark_group("credit_factor_calibration");
    // Scale down sample count for the large scenario.
    group.sample_size(10);

    for &(n_issuers, n_months, n_levels) in BENCH_SCENARIOS {
        let inputs = build_inputs(n_issuers, n_months, n_levels);
        let config = build_config(n_levels);
        let calibrator = CreditCalibrator::new(config);

        group.throughput(Throughput::Elements(n_issuers as u64));

        let bench_id = BenchmarkId::new("n_issuers", n_issuers);
        group.bench_with_input(bench_id, &(calibrator, inputs), |b, (cal, inp)| {
            b.iter_batched(
                || inp.clone(),
                |inp| cal.calibrate(inp).unwrap(),
                BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_credit_factor_calibration);
criterion_main!(benches);
