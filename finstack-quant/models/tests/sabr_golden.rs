//! SABR volatility-model golden tests and fixture validation.

use finstack_quant_models::SabrParameters;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const SCHEMA: &str = "finstack_quant.golden/1";
const FIXTURE_ROOT: &str = "tests/data/market_data/sabr";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SabrFixture {
    schema: String,
    metadata: Metadata,
    kind: String,
    alpha: f64,
    beta: f64,
    nu: f64,
    rho: f64,
    #[serde(default)]
    shift: Option<f64>,
    forward: f64,
    time_to_expiry: f64,
    strikes: Vec<StrikeEntry>,
    expected: BTreeMap<String, f64>,
    tolerances: BTreeMap<String, Tolerance>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    name: String,
    domain: String,
    description: String,
    valuation_date: String,
    source: String,
    source_detail: String,
    captured_by: String,
    captured_on: String,
    last_reviewed_by: String,
    last_reviewed_on: String,
    review_interval_months: u32,
    regen_command: String,
    #[serde(default)]
    screenshots: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StrikeEntry {
    key: String,
    strike: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tolerance {
    #[serde(default)]
    abs: Option<f64>,
    #[serde(default)]
    rel: Option<f64>,
    #[serde(default)]
    tolerance_reason: Option<String>,
}

fn fixture_paths() -> Result<Vec<PathBuf>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_ROOT);
    let entries = std::fs::read_dir(&root)
        .map_err(|error| format!("read SABR fixture directory {root:?}: {error}"))?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect::<Vec<_>>();
    paths.sort();
    if let Ok(filter) = std::env::var("GOLDEN_FIXTURE_FILTER") {
        paths.retain(|path| path.to_string_lossy().contains(&filter));
    }
    Ok(paths)
}

fn read_fixture(path: &Path) -> Result<SabrFixture, String> {
    let raw =
        std::fs::read_to_string(path).map_err(|error| format!("read fixture {path:?}: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("parse fixture {path:?}: {error}"))
}

fn validate_fixture(fixture: &SabrFixture) -> Result<(), String> {
    if fixture.schema != SCHEMA {
        return Err(format!("unsupported schema '{}'", fixture.schema));
    }
    if fixture.kind != "sabr_smile" || fixture.metadata.domain != "volatility.sabr" {
        return Err(format!(
            "fixture '{}' is not a volatility.sabr/sabr_smile fixture",
            fixture.metadata.name
        ));
    }
    if fixture.strikes.is_empty() {
        return Err("sabr_smile fixture must define at least one strike".to_string());
    }
    let strike_keys = fixture
        .strikes
        .iter()
        .map(|entry| entry.key.as_str())
        .collect::<BTreeSet<_>>();
    let expected_keys = fixture
        .expected
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if strike_keys.len() != fixture.strikes.len() {
        return Err("sabr_smile strike keys must be unique".to_string());
    }
    if strike_keys != expected_keys {
        return Err("strike keys must match expected keys exactly".to_string());
    }
    if fixture.tolerances.keys().collect::<BTreeSet<_>>()
        != fixture.expected.keys().collect::<BTreeSet<_>>()
    {
        return Err("tolerance keys must match expected keys exactly".to_string());
    }
    for (key, tolerance) in &fixture.tolerances {
        if tolerance.abs.is_none() && tolerance.rel.is_none() {
            return Err(format!("tolerance for '{key}' has neither abs nor rel"));
        }
    }
    Ok(())
}

fn run_fixture(fixture: &SabrFixture) -> Result<Vec<String>, String> {
    validate_fixture(fixture)?;
    let params = if let Some(shift) = fixture.shift {
        SabrParameters::new_with_shift(fixture.alpha, fixture.beta, fixture.nu, fixture.rho, shift)
    } else {
        SabrParameters::new(fixture.alpha, fixture.beta, fixture.nu, fixture.rho)
    }
    .map_err(|error| format!("build SABR parameters: {error}"))?;
    let mut failures = Vec::new();
    for strike in &fixture.strikes {
        let actual = params
            .implied_volatility(fixture.forward, strike.strike, fixture.time_to_expiry)
            .map_err(|error| format!("price strike '{}': {error}", strike.key))?;
        let expected = fixture.expected[&strike.key];
        let tolerance = &fixture.tolerances[&strike.key];
        let abs_diff = (actual - expected).abs();
        let rel_diff = abs_diff / expected.abs().max(1.0e-12);
        let passed = tolerance.abs.is_some_and(|limit| abs_diff <= limit)
            || tolerance.rel.is_some_and(|limit| rel_diff <= limit);
        if !passed {
            failures.push(format!(
                "{}: actual={actual:.16e}, expected={expected:.16e}, abs_diff={abs_diff:.3e}, reason={}",
                strike.key,
                tolerance.tolerance_reason.as_deref().unwrap_or("unspecified")
            ));
        }
    }
    Ok(failures)
}

#[test]
fn golden_sabr_fixtures_from_existing_json_files() {
    let paths = fixture_paths().expect("SABR fixture discovery should succeed");
    assert!(!paths.is_empty(), "no SABR JSON fixtures were discovered");
    let mut failures = Vec::new();
    for path in paths {
        match read_fixture(&path).and_then(|fixture| run_fixture(&fixture)) {
            Ok(fixture_failures) => failures.extend(
                fixture_failures
                    .into_iter()
                    .map(|failure| format!("{}: {failure}", path.display())),
            ),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    assert!(
        failures.is_empty(),
        "{} SABR golden fixture failure(s):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn sabr_fixture_schema_and_provenance_parse_under_models() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_ROOT)
        .join("beta_half_smile.json");
    let fixture = read_fixture(&path).expect("fixture parses");
    validate_fixture(&fixture).expect("fixture validates");
    assert_eq!(fixture.metadata.name, "beta_half_smile");
    assert_eq!(fixture.strikes.len(), 5);
    assert!(fixture.shift.is_none());
    assert!(!fixture.metadata.description.is_empty());
    assert!(!fixture.metadata.valuation_date.is_empty());
    assert_eq!(fixture.metadata.source, "formula");
    assert!(!fixture.metadata.source_detail.is_empty());
    assert!(!fixture.metadata.captured_by.is_empty());
    assert!(!fixture.metadata.captured_on.is_empty());
    assert!(!fixture.metadata.last_reviewed_by.is_empty());
    assert!(!fixture.metadata.last_reviewed_on.is_empty());
    assert!(fixture.metadata.review_interval_months > 0);
    assert!(fixture.metadata.regen_command.is_empty());
    assert!(fixture.metadata.screenshots.is_empty());
}
