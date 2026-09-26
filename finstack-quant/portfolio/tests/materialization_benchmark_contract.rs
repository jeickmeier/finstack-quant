//! Contract tests for the Phase 7 materialization benchmark harness.

#[path = "../benches/materialization_fixtures.rs"]
mod materialization_fixtures;
#[path = "../benches/materialization_gate.rs"]
mod materialization_gate;

use std::collections::HashSet;

use finstack_quant_core::contract::LoadLimits;
use finstack_quant_portfolio::{InstrumentArtifactCache, Portfolio};
use serde_json::Value;

use materialization_fixtures::{build_fixture, fixture_a, fixture_b, FixtureKind};
use materialization_gate::{
    evaluate_latency_gates, p95_millis, parse_sample_count, COLD_ENGINEERING_TARGET_MILLIS,
    COLD_HARD_LIMIT_MILLIS, WARM_ENGINEERING_TARGET_MILLIS,
};

#[test]
fn shared_fixtures_are_deterministic_and_have_required_shapes() {
    let unique = fixture_a();
    let unique_again = fixture_a();
    assert_eq!(unique.bytes, unique_again.bytes);
    assert_eq!(unique.name, "materialization-a-5000-unique");
    assert_eq!(unique.artifact_count, 5_000);
    assert_eq!(unique.position_count, 5_000);

    let parsed: Value = serde_json::from_slice(&unique.bytes).expect("fixture JSON");
    let instruments = parsed["instruments"].as_array().expect("instrument array");
    let hashes: HashSet<_> = instruments
        .iter()
        .map(|instrument| {
            instrument["content_hash"]
                .as_str()
                .expect("fixture content hash")
        })
        .collect();
    assert_eq!(hashes.len(), 5_000);
    assert_eq!(
        instruments
            .iter()
            .filter(|instrument| instrument["envelope"]["instrument"]["type"] == "bond")
            .count(),
        2_500
    );
    assert_eq!(
        instruments
            .iter()
            .filter(|instrument| instrument["envelope"]["instrument"]["type"] == "cms_swap")
            .count(),
        2_500
    );
    for (index, instrument) in instruments.iter().enumerate() {
        let spec = &instrument["envelope"]["instrument"]["spec"];
        if index % 2 == 0 {
            let schedule = spec["cashflow_spec"]["amortizing"]["schedule"]["step_remaining"]
                ["schedule"]
                .as_array()
                .expect("bond serialized amortization schedule");
            assert_eq!(schedule.len(), 20);
            assert_eq!(schedule[0][0], "2025-07-01");
            assert_eq!(schedule[19][0], "2035-01-01");
            assert_eq!(schedule[19][1]["amount"], "0");
        } else {
            let fixing_dates = spec["fixing_dates"]
                .as_array()
                .expect("CMS fixing schedule");
            let payment_dates = spec["payment_dates"]
                .as_array()
                .expect("CMS payment schedule");
            let accruals = spec["accrual_fractions"]
                .as_array()
                .expect("CMS accrual schedule");
            let funding_dates = spec["funding_leg"]["payment_dates"]
                .as_array()
                .expect("funding payment schedule");
            assert_eq!(fixing_dates.len(), 20);
            assert_eq!(payment_dates.len(), 20);
            assert_eq!(accruals.len(), 20);
            assert_eq!(funding_dates.len(), 20);
            assert_eq!(fixing_dates[0], "2024-12-30");
            assert_eq!(payment_dates[0], "2025-07-01");
            assert_eq!(payment_dates[19], "2035-01-02");
            assert_eq!(funding_dates, payment_dates);
            assert!(accruals.iter().all(|value| {
                value
                    .as_f64()
                    .is_some_and(|fraction| fraction > 0.0 && fraction < 0.52)
            }));
        }
    }

    let positions = parsed["positions"].as_array().expect("position array");
    assert_eq!(positions.len(), 5_000);
    for (index, position) in positions.iter().enumerate() {
        assert_eq!(
            position["artifact_id"],
            format!("artifact-{index:05}"),
            "fixture A must map positions one-to-one"
        );
        assert_eq!(
            position["instrument_id"],
            instruments[index]["envelope"]["instrument"]["spec"]["id"]
        );
    }

    let deduplicated = fixture_b();
    assert_eq!(deduplicated.name, "materialization-b-5000-50");
    assert_eq!(deduplicated.artifact_count, 50);
    assert_eq!(deduplicated.position_count, 5_000);
    assert!(deduplicated.dependency_count > 0);
}

#[test]
fn materialization_report_exposes_required_benchmark_counters() {
    let fixture = build_fixture(FixtureKind::DeduplicatedScheduleRich, 4, 8);
    let cache = InstrumentArtifactCache::with_capacity(4);
    let (_portfolio, report) =
        Portfolio::from_materialization(&fixture.bytes, &cache, &LoadLimits::default())
            .expect("benchmark fixture materializes");

    assert_eq!(report.input_bytes, fixture.bytes.len());
    assert_eq!(report.unique_instruments, fixture.artifact_count);
    assert_eq!(report.positions, fixture.position_count);
    assert_eq!(report.dependencies, fixture.dependency_count);
    assert!(report.timing_available);
    assert!(
        report.phase_nanos.parse
            + report.phase_nanos.validate_versions
            + report.phase_nanos.decode_instruments
            + report.phase_nanos.build_positions
            + report.phase_nanos.index_build
            > 0
    );
}

#[test]
fn validation_only_checks_semantics_without_building_positions_or_indices() {
    let fixture = build_fixture(FixtureKind::DeduplicatedScheduleRich, 4, 8);
    let cache = InstrumentArtifactCache::with_capacity(4);
    let report =
        Portfolio::validate_materialization(&fixture.bytes, &cache, &LoadLimits::default())
            .expect("valid fixture");

    assert_eq!(report.positions, 8);
    assert_eq!(report.unique_instruments, 4);
    assert_eq!(report.dependencies, fixture.dependency_count);
    assert_eq!(report.phase_nanos.build_positions, 0);
    assert_eq!(report.phase_nanos.index_build, 0);

    let mut invalid: Value = serde_json::from_slice(&fixture.bytes).expect("fixture JSON");
    invalid["positions"][0]["quantity"] = Value::String("NaN".to_string());
    let invalid = serde_json::to_vec(&invalid).expect("invalid fixture bytes");
    assert!(Portfolio::validate_materialization(&invalid, &cache, &LoadLimits::default()).is_err());

    let source: Value = serde_json::from_slice(&fixture.bytes).expect("fixture JSON");
    for invalid in [
        {
            let mut value = source.clone();
            value["schema"] =
                Value::String("finstack_quant.portfolio_materialization/999".to_string());
            value
        },
        {
            let mut value = source.clone();
            value["instruments"][0]["content_hash"] = Value::String("0".repeat(64));
            value
        },
        {
            let mut value = source.clone();
            value["instruments"][0]["dependencies"] = serde_json::json!({});
            value
        },
        {
            let mut value = source;
            value["positions"][0]["instrument_id"] = Value::String("WRONG-INSTRUMENT".to_string());
            value
        },
    ] {
        let bytes = serde_json::to_vec(&invalid).expect("invalid fixture bytes");
        assert!(
            Portfolio::validate_materialization(&bytes, &cache, &LoadLimits::default()).is_err()
        );
    }
    assert!(Portfolio::validate_materialization(
        &fixture.bytes,
        &cache,
        &LoadLimits::default().with_max_positions(7),
    )
    .is_err());
}

#[test]
fn validation_only_enforces_entity_book_and_hierarchy_invariants() {
    let fixture = build_fixture(FixtureKind::DeduplicatedScheduleRich, 2, 4);
    let source: Value = serde_json::from_slice(&fixture.bytes).expect("fixture JSON");

    let cases = [
        (
            {
                let mut value = source.clone();
                value["positions"][0]["entity_id"] = Value::String("missing-entity".to_string());
                value
            },
            "portfolio/unknown-entity",
        ),
        (
            with_books(
                source.clone(),
                serde_json::json!({
                    "book-a": book("book-a", None, &["missing-position"], &[])
                }),
            ),
            "portfolio/book-missing-position",
        ),
        (
            with_books(
                source.clone(),
                serde_json::json!({
                    "book-a": book("book-a", Some("missing-parent"), &[], &[])
                }),
            ),
            "portfolio/book-missing-parent",
        ),
        (
            with_books(
                source.clone(),
                serde_json::json!({
                    "book-a": book("book-a", Some("book-b"), &[], &["book-b"]),
                    "book-b": book("book-b", Some("book-a"), &[], &["book-a"])
                }),
            ),
            "portfolio/book-cycle",
        ),
        (
            with_books(
                source.clone(),
                serde_json::json!({
                    "book-a": book("book-a", None, &[], &["missing-child"])
                }),
            ),
            "portfolio/book-missing-child",
        ),
        (
            with_books(
                source.clone(),
                serde_json::json!({
                    "book-a": book("book-a", None, &["position-00000"], &[]),
                    "book-b": book("book-b", None, &["position-00000"], &[])
                }),
            ),
            "portfolio/position-multiple-books",
        ),
        (
            with_books(
                source,
                serde_json::json!({
                    "book-a": book("different-id", None, &[], &[])
                }),
            ),
            "portfolio/book-id-mismatch",
        ),
    ];

    for (invalid, expected_code) in cases {
        assert_validation_code(&invalid, expected_code);
    }
}

fn book(id: &str, parent: Option<&str>, positions: &[&str], children: &[&str]) -> Value {
    serde_json::json!({
        "id": id,
        "name": null,
        "parent_id": parent,
        "position_ids": positions,
        "child_book_ids": children,
    })
}

fn with_books(mut bundle: Value, books: Value) -> Value {
    bundle["portfolio"]["books"] = books;
    bundle
}

fn assert_validation_code(bundle: &Value, expected_code: &str) {
    let bytes = serde_json::to_vec(bundle).expect("invalid fixture bytes");
    let cache = InstrumentArtifactCache::with_capacity(2);
    let error = Portfolio::validate_materialization(&bytes, &cache, &LoadLimits::default())
        .expect_err("invalid envelope must fail validation");
    let finstack_quant_portfolio::Error::MaterializationFailed(report) = error else {
        panic!("expected materialization diagnostics");
    };
    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == expected_code),
        "expected diagnostic {expected_code}, got {:?}",
        report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn latency_gate_logic_distinguishes_hard_and_engineering_targets() {
    let mut samples = (1..=20).map(f64::from).collect::<Vec<_>>();
    assert_eq!(p95_millis(&mut samples), 19.0);

    let passing = evaluate_latency_gates(499.0, 249.0);
    assert!(passing.cold_hard_pass);
    assert!(passing.cold_target_pass);
    assert!(passing.warm_target_pass);

    let target_miss = evaluate_latency_gates(500.0, 250.0);
    assert!(target_miss.cold_hard_pass);
    assert!(!target_miss.cold_target_pass);
    assert!(!target_miss.warm_target_pass);

    let hard_miss = evaluate_latency_gates(1_000.0, 1.0);
    assert!(!hard_miss.cold_hard_pass);
    assert_eq!(COLD_HARD_LIMIT_MILLIS, 1_000.0);
    assert_eq!(COLD_ENGINEERING_TARGET_MILLIS, 500.0);
    assert_eq!(WARM_ENGINEERING_TARGET_MILLIS, 250.0);
}

#[test]
fn acceptance_sample_count_requires_at_least_one_hundred_without_smoke_override() {
    assert_eq!(parse_sample_count(None, false).expect("default"), 100);
    assert_eq!(
        parse_sample_count(Some("100"), false).expect("minimum"),
        100
    );
    assert_eq!(parse_sample_count(Some("250"), false).expect("larger"), 250);
    assert!(parse_sample_count(Some("99"), false).is_err());
    assert!(parse_sample_count(Some("nan"), false).is_err());
    assert_eq!(parse_sample_count(Some("5"), true).expect("smoke"), 5);
}
