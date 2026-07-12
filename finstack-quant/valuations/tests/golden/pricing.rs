//! Pricing-domain golden tests.

use crate::golden::runner::run_golden_at_path;
use crate::golden::schema::GoldenFixture;
use crate::golden::tolerance::ComparisonResult;
use crate::golden::walk::collect_fixture_paths_under;
use serde::Deserialize;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::Path;

/// One unresolved metric in `known_non_executable.json`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnresolvedMetricEntry {
    /// Exact expected metric key.
    metric: String,
    /// Why this benchmark gap remains unresolved.
    reason: String,
    /// Concrete expected/actual evidence for the gap.
    evidence: String,
}

/// Metric-specific unresolved entries for one fixture.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NonExecutableEntry {
    /// Fixture path relative to `tests/golden/data/`.
    path: String,
    /// Human-readable fixture-level summary.
    description: String,
    /// Exact unresolved metrics.
    metrics: Vec<UnresolvedMetricEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NonExecutableFile {
    description: String,
    fixtures: Vec<NonExecutableEntry>,
}

#[derive(Default)]
struct FixtureOutcome {
    failures: Vec<String>,
    expected_unresolved: Vec<String>,
}

/// Load and validate the shared metric-specific unresolved list.
///
/// Setting `GOLDEN_IGNORE_NON_EXECUTABLE` returns an empty list so every metric
/// is compared strictly (see `mise goldens-test-strict`).
fn known_non_executable() -> Result<Vec<NonExecutableEntry>, String> {
    if strict_golden_mode_enabled() {
        return Ok(Vec::new());
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/known_non_executable.json");
    let raw =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let parsed: NonExecutableFile =
        serde_json::from_str(&raw).map_err(|err| format!("parse {}: {err}", path.display()))?;
    require_non_empty("description", &parsed.description)?;

    let data_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/data");
    let mut seen_paths = HashSet::new();
    for fixture_entry in &parsed.fixtures {
        require_non_empty("path", &fixture_entry.path)?;
        require_non_empty("fixture description", &fixture_entry.description)?;
        if !seen_paths.insert(fixture_entry.path.as_str()) {
            return Err(format!(
                "duplicate unresolved fixture entry '{}'",
                fixture_entry.path
            ));
        }
        if fixture_entry.metrics.is_empty() {
            return Err(format!(
                "unresolved fixture entry '{}' must contain at least one metric",
                fixture_entry.path
            ));
        }

        let fixture_path = data_root.join(&fixture_entry.path);
        if !fixture_path.exists() {
            return Err(format!(
                "stale unresolved fixture entry '{}': fixture does not exist",
                fixture_entry.path
            ));
        }
        crate::golden::walk::validate_fixture(&fixture_path)
            .map_err(|err| format!("validate unresolved fixture {fixture_path:?}: {err}"))?;
        let fixture_raw = std::fs::read_to_string(&fixture_path)
            .map_err(|err| format!("read unresolved fixture {fixture_path:?}: {err}"))?;
        let fixture: GoldenFixture = serde_json::from_str(&fixture_raw)
            .map_err(|err| format!("parse unresolved fixture {fixture_path:?}: {err}"))?;

        let mut seen_metrics = HashSet::new();
        for metric_entry in &fixture_entry.metrics {
            require_non_empty("metric", &metric_entry.metric)?;
            require_non_empty("reason", &metric_entry.reason)?;
            require_non_empty("evidence", &metric_entry.evidence)?;
            if !fixture.expected.contains_key(&metric_entry.metric) {
                return Err(format!(
                    "unresolved metric '{}' is not expected by fixture '{}'",
                    metric_entry.metric, fixture_entry.path
                ));
            }
            if !seen_metrics.insert(metric_entry.metric.as_str()) {
                return Err(format!(
                    "duplicate unresolved metric '{}' for fixture '{}'",
                    metric_entry.metric, fixture_entry.path
                ));
            }
        }
    }
    Ok(parsed.fixtures)
}

fn strict_golden_mode_enabled() -> bool {
    env_value_is_truthy(std::env::var_os("GOLDEN_IGNORE_NON_EXECUTABLE").as_deref())
}

fn env_value_is_truthy(value: Option<&OsStr>) -> bool {
    value.and_then(OsStr::to_str).is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn require_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!(
            "unresolved allowlist field '{field}' must be non-empty"
        ));
    }
    Ok(())
}

fn unresolved_metrics_for_path<'a>(
    entries: &'a [NonExecutableEntry],
    path: &Path,
) -> &'a [UnresolvedMetricEntry] {
    let data_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/data");
    let relative = path
        .strip_prefix(data_root)
        .map(|value| value.to_string_lossy())
        .unwrap_or_else(|_| path.to_string_lossy());
    entries
        .iter()
        .find(|entry| entry.path == relative)
        .map_or(&[], |entry| entry.metrics.as_slice())
}

fn classify_fixture_run(
    path: &Path,
    unresolved: &[UnresolvedMetricEntry],
    run_result: Result<Vec<ComparisonResult>, String>,
) -> FixtureOutcome {
    let mut outcome = FixtureOutcome::default();
    let mut results = match run_result {
        Ok(results) => results,
        Err(err) => {
            outcome
                .failures
                .push(format!("run fixture {path:?}: {err}"));
            return outcome;
        }
    };
    results.sort_by(|left, right| left.metric.cmp(&right.metric));

    for unresolved_entry in unresolved {
        if !results
            .iter()
            .any(|result| result.metric == unresolved_entry.metric)
        {
            outcome.failures.push(format!(
                "invalid unresolved metric '{}' for {path:?}: runner did not compare it",
                unresolved_entry.metric
            ));
        }
    }

    for result in results {
        let unresolved_entry = unresolved
            .iter()
            .find(|entry| entry.metric == result.metric);
        match (result.passed, unresolved_entry) {
            (true, Some(entry)) => outcome.failures.push(format!(
                "stale unresolved metric '{}' for {path:?}: comparison now passes; remove the allowlist entry",
                entry.metric
            )),
            (false, Some(entry)) => outcome.expected_unresolved.push(format!(
                "expected unresolved metric {path:?}::{}: {} Evidence: {}\n{}",
                entry.metric,
                entry.reason,
                entry.evidence,
                result.failure_message(&path.display().to_string())
            )),
            (false, None) => outcome
                .failures
                .push(result.failure_message(&path.display().to_string())),
            (true, None) => {}
        }
    }
    outcome
}

#[test]
#[ignore = "slow: covered by mise goldens-test or mise rust-test-slow"]
fn golden_pricing_fixtures_from_existing_json_files() {
    let mut paths =
        collect_fixture_paths_under("pricing").expect("pricing fixture discovery should succeed");
    if let Ok(filter) = std::env::var("GOLDEN_FIXTURE_FILTER") {
        paths.retain(|path| path.to_string_lossy().contains(&filter));
    }
    assert!(
        !paths.is_empty(),
        "pricing fixture discovery did not find any JSON files"
    );

    let allowlist = known_non_executable().expect("metric-specific unresolved allowlist is valid");
    let mut failures = Vec::new();
    for path in paths {
        let unresolved = unresolved_metrics_for_path(&allowlist, &path);
        let outcome = classify_fixture_run(&path, unresolved, run_golden_at_path(&path));
        for expected in outcome.expected_unresolved {
            eprintln!("{expected}");
        }
        failures.extend(outcome.failures);
    }

    assert!(
        failures.is_empty(),
        "{} pricing golden fixture failure(s):\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[cfg(test)]
mod unresolved_metric_tests {
    use super::*;
    use crate::golden::schema::ToleranceEntry;
    use crate::golden::tolerance::compare;

    fn unresolved(metric: &str) -> UnresolvedMetricEntry {
        UnresolvedMetricEntry {
            metric: metric.to_string(),
            reason: "known benchmark gap".to_string(),
            evidence: "expected 1, actual 2".to_string(),
        }
    }

    fn passed(metric: &str) -> crate::golden::tolerance::ComparisonResult {
        compare(
            metric,
            1.0,
            1.0,
            &ToleranceEntry {
                abs: Some(0.01),
                rel: None,
                tolerance_reason: None,
            },
        )
    }

    fn failed(metric: &str) -> crate::golden::tolerance::ComparisonResult {
        compare(
            metric,
            2.0,
            1.0,
            &ToleranceEntry {
                abs: Some(0.01),
                rel: None,
                tolerance_reason: None,
            },
        )
    }

    #[test]
    fn execution_error_is_fatal_even_with_unresolved_metric() {
        let outcome = classify_fixture_run(
            Path::new("fixture.json"),
            &[unresolved("npv")],
            Err("pricing exploded".to_string()),
        );
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("pricing exploded"));
        assert!(outcome.expected_unresolved.is_empty());
    }

    #[test]
    fn unrelated_metric_mismatch_is_fatal() {
        let outcome = classify_fixture_run(
            Path::new("fixture.json"),
            &[unresolved("npv")],
            Ok(vec![failed("dv01"), failed("npv")]),
        );
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("dv01"));
        assert_eq!(outcome.expected_unresolved.len(), 1);
    }

    #[test]
    fn listed_metric_mismatch_is_expected_unresolved() {
        let outcome = classify_fixture_run(
            Path::new("fixture.json"),
            &[unresolved("npv")],
            Ok(vec![failed("npv")]),
        );
        assert!(outcome.failures.is_empty());
        assert_eq!(outcome.expected_unresolved.len(), 1);
        assert!(outcome.expected_unresolved[0].contains("known benchmark gap"));
    }

    #[test]
    fn listed_metric_pass_is_stale_and_fatal() {
        let outcome = classify_fixture_run(
            Path::new("fixture.json"),
            &[unresolved("npv")],
            Ok(vec![passed("npv")]),
        );
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("stale"));
    }

    #[test]
    fn invalid_metric_entry_is_fatal() {
        let outcome = classify_fixture_run(
            Path::new("fixture.json"),
            &[unresolved("not_a_metric")],
            Ok(vec![passed("npv")]),
        );
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("not_a_metric"));
    }

    #[test]
    fn strict_mode_truthy_values_match_python() {
        let cases = [
            (None, false),
            (Some(""), false),
            (Some("0"), false),
            (Some("false"), false),
            (Some("FALSE"), false),
            (Some("1"), true),
            (Some("true"), true),
            (Some("YES"), true),
            (Some("on"), true),
        ];
        for (value, expected) in cases {
            assert_eq!(
                env_value_is_truthy(value.map(OsStr::new)),
                expected,
                "value={value:?}"
            );
        }
    }

    #[test]
    fn unresolved_schema_rejects_unknown_fields_at_every_level() {
        let malformed = [
            r#"{"description":"test","fixtures":[],"unknown":true}"#,
            r#"{
                "description":"test",
                "fixtures":[{
                    "path":"pricing/deposit/usd_deposit_3m.json",
                    "description":"test fixture",
                    "metrics":[],
                    "unknown":true
                }]
            }"#,
            r#"{
                "description":"test",
                "fixtures":[{
                    "path":"pricing/deposit/usd_deposit_3m.json",
                    "description":"test fixture",
                    "metrics":[{
                        "metric":"npv",
                        "reason":"gap",
                        "evidence":"expected 1, actual 2",
                        "unknown":true
                    }]
                }]
            }"#,
        ];
        for raw in malformed {
            let err = serde_json::from_str::<NonExecutableFile>(raw)
                .expect_err("unknown fields must be rejected");
            assert!(err.to_string().contains("unknown field"), "{err}");
        }
    }

    #[test]
    fn unresolved_schema_rejects_invalid_types() {
        for raw in [
            "[]",
            r#"{"description":"test","fixtures":{}}"#,
            r#"{"description":"test","fixtures":[1]}"#,
            r#"{
                "description":"test",
                "fixtures":[{
                    "path":"pricing/deposit/usd_deposit_3m.json",
                    "description":"test fixture",
                    "metrics":{}
                }]
            }"#,
        ] {
            assert!(serde_json::from_str::<NonExecutableFile>(raw).is_err());
        }
    }

    #[test]
    fn required_allowlist_strings_reject_blanks() {
        for (field, value) in [
            ("description", ""),
            ("path", " "),
            ("fixture description", "\t"),
            ("metric", "\n"),
            ("reason", ""),
            ("evidence", " "),
        ] {
            let err = require_non_empty(field, value).expect_err("blank field must fail");
            assert!(err.contains(field));
        }
    }

    #[test]
    fn multiple_unresolved_messages_are_sorted_by_metric() {
        let outcome = classify_fixture_run(
            Path::new("fixture.json"),
            &[unresolved("npv"), unresolved("dv01")],
            Ok(vec![failed("npv"), failed("dv01")]),
        );
        assert!(outcome.failures.is_empty());
        assert_eq!(outcome.expected_unresolved.len(), 2);
        assert!(outcome.expected_unresolved[0].contains("::dv01:"));
        assert!(outcome.expected_unresolved[1].contains("::npv:"));
    }
}
