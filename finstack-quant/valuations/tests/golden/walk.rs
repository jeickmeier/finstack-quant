//! Walk-test for validating every committed golden fixture.

use crate::golden::pricing_common::requested_metrics;
use crate::golden::schema::{Body, GoldenFixture, Market, SCHEMA_VERSION};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations::calibration::api::schema::CalibrationEnvelope;
use finstack_quant_valuations::metrics::MetricId;
use finstack_quant_valuations::pricer::parse_boxed_instrument_json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const DATA_ROOT: &str = "tests/golden/data";
const MANUAL_SCREENSHOT_SOURCES: &[&str] = &["bloomberg-screen", "intex"];
const VALID_SOURCES: &[&str] = &[
    "quantlib",
    "bloomberg-api",
    "bloomberg-screen",
    "intex",
    "formula",
    "textbook",
];
const COMMON_TOP_LEVEL_KEYS: &[&str] = &[
    "schema_version",
    "metadata",
    "kind",
    "expected",
    "tolerances",
];
const PRICING_BODY_KEYS: &[&str] = &["model", "market", "instrument"];
const SABR_BODY_KEYS: &[&str] = &[
    "alpha",
    "beta",
    "nu",
    "rho",
    "shift",
    "forward",
    "time_to_expiry",
    "strikes",
];
const ZERO_RISK_METRICS_REQUIRING_REASON: &[&str] = &[
    "bucketed_dv01",
    "convexity",
    "cs01",
    "delta",
    "duration_mod",
    "dv01",
    "foreign_rho",
    "gamma",
    "inflation01",
    "recovery_01",
    "rho",
    "spread_dv01",
    "vega",
];

fn collect_fixture_paths() -> Vec<PathBuf> {
    collect_fixture_paths_from(&data_root())
}

pub(crate) fn collect_fixture_paths_under(relative_dir: &str) -> Result<Vec<PathBuf>, String> {
    let paths = collect_fixture_paths_from(&data_root().join(relative_dir));
    let read_errors = paths
        .iter()
        .filter(|path| path.to_string_lossy().starts_with("__read_dir_error__:"))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if read_errors.is_empty() {
        Ok(paths)
    } else {
        Err(read_errors.join("\n"))
    }
}

fn data_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir).join(DATA_ROOT)
}

fn collect_fixture_paths_from(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }

    let mut paths = Vec::new();
    walk_dir(root, &mut paths);
    paths.sort();
    paths
}

fn walk_dir(dir: &Path, paths: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) => {
            paths.push(PathBuf::from(format!(
                "__read_dir_error__:{}:{}",
                dir.display(),
                err
            )));
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some("screenshots") {
                walk_dir(&path, paths);
            }
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            paths.push(path);
        }
    }
}

pub(crate) fn validate_fixture(path: &Path) -> Result<(), String> {
    if path.to_string_lossy().starts_with("__read_dir_error__:") {
        return Err(path.display().to_string());
    }

    let raw = fs::read_to_string(path).map_err(|err| format!("read failed: {err}"))?;
    let fixture: GoldenFixture =
        serde_json::from_str(&raw).map_err(|err| format!("parse failed: {err}"))?;

    if fixture.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "schema_version is '{}', expected '{}'",
            fixture.schema_version, SCHEMA_VERSION
        ));
    }

    validate_top_level_keys(&raw, &fixture)?;

    if !VALID_SOURCES.contains(&fixture.metadata.source.as_str()) {
        return Err(format!(
            "metadata.source '{}' is not recognized",
            fixture.metadata.source
        ));
    }

    validate_pricing_golden_type(path, &fixture)?;

    validate_non_empty("metadata.name", &fixture.metadata.name)?;
    validate_non_empty("metadata.domain", &fixture.metadata.domain)?;
    validate_non_empty("metadata.description", &fixture.metadata.description)?;
    validate_non_empty("metadata.valuation_date", &fixture.metadata.valuation_date)?;
    validate_non_empty("metadata.source_detail", &fixture.metadata.source_detail)?;
    validate_non_empty("metadata.captured_by", &fixture.metadata.captured_by)?;
    validate_non_empty("metadata.captured_on", &fixture.metadata.captured_on)?;
    validate_non_empty(
        "metadata.last_reviewed_by",
        &fixture.metadata.last_reviewed_by,
    )?;
    validate_non_empty(
        "metadata.last_reviewed_on",
        &fixture.metadata.last_reviewed_on,
    )?;

    for metric in fixture.expected.keys() {
        if !fixture.tolerances.contains_key(metric) {
            return Err(format!("expected has '{metric}' but tolerances does not"));
        }
    }

    for (metric, tolerance) in &fixture.tolerances {
        if !fixture.expected.contains_key(metric) {
            return Err(format!("tolerances has '{metric}' but expected does not"));
        }
        if tolerance.abs.is_none() && tolerance.rel.is_none() {
            return Err(format!("tolerance for '{metric}' has neither abs nor rel"));
        }
    }

    validate_zero_risk_metric_reasons(&fixture)?;

    match &fixture.body {
        Body::Pricing(_) => validate_pricing_body(&fixture)?,
        Body::SabrSmile(_) => validate_sabr_body(&fixture)?,
    }

    if MANUAL_SCREENSHOT_SOURCES.contains(&fixture.metadata.source.as_str())
        && fixture.metadata.screenshots.is_empty()
    {
        return Err(format!(
            "source '{}' requires at least one screenshot",
            fixture.metadata.source
        ));
    }

    validate_screenshot_paths(path, &fixture)?;

    Ok(())
}

fn validate_pricing_golden_type(path: &Path, fixture: &GoldenFixture) -> Result<(), String> {
    if !matches!(fixture.body, Body::Pricing(_)) {
        return Ok(());
    }

    let relative = path
        .strip_prefix(data_root().join("pricing"))
        .map_err(|_| "pricing fixture must live under data/pricing".to_string())?;
    let actual = relative
        .components()
        .next()
        .and_then(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .ok_or("pricing fixture is missing its golden type directory")?;
    let expected = match fixture.metadata.source.as_str() {
        "quantlib" => "quantlib",
        "bloomberg-api" | "bloomberg-screen" => "bloomberg",
        "formula" | "textbook" | "intex" => "regression_goldens",
        source => return Err(format!("metadata.source '{source}' is not recognized")),
    };

    if actual != expected {
        return Err(format!(
            "metadata.source '{}' requires pricing/{expected}/, found pricing/{actual}/",
            fixture.metadata.source
        ));
    }
    Ok(())
}

fn validate_top_level_keys(raw: &str, fixture: &GoldenFixture) -> Result<(), String> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|err| format!("parse top-level keys: {err}"))?;
    let object = value.as_object().ok_or("fixture must be a JSON object")?;

    let mut allowed = COMMON_TOP_LEVEL_KEYS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let body_keys: &[&str] = match &fixture.body {
        Body::Pricing(_) => PRICING_BODY_KEYS,
        Body::SabrSmile(_) => SABR_BODY_KEYS,
    };
    allowed.extend(body_keys.iter().copied());

    for key in object.keys() {
        if !allowed.contains(key.as_str()) {
            return Err(format!("fixture has unexpected top-level key '{key}'"));
        }
    }
    Ok(())
}

fn validate_pricing_body(fixture: &GoldenFixture) -> Result<(), String> {
    let pricing = fixture.pricing().ok_or("pricing body expected")?;
    validate_non_empty("model", &pricing.model)?;

    match &pricing.market {
        Market::Snapshot { data } => {
            serde_json::from_value::<MarketContext>(data.clone())
                .map_err(|err| format!("market.data is not a valid MarketContext: {err}"))?;
        }
        Market::Envelope { envelope } => {
            serde_json::from_value::<CalibrationEnvelope>(envelope.clone()).map_err(|err| {
                format!("market.envelope is not a valid CalibrationEnvelope: {err}")
            })?;
        }
    }

    validate_swaption_underlying_tenor(&pricing.instrument)?;
    let instrument_json = serde_json::to_string(&pricing.instrument)
        .map_err(|err| format!("serialize instrument: {err}"))?;
    parse_boxed_instrument_json(&instrument_json, None)
        .map_err(|err| format!("instrument is not a valid instrument: {err}"))?;

    for metric in requested_metrics(fixture) {
        MetricId::parse_strict(&metric)
            .map_err(|err| format!("expected metric base '{metric}': {err}"))?;
    }

    validate_required_pricing_risk_metrics(fixture)
}

fn validate_sabr_body(fixture: &GoldenFixture) -> Result<(), String> {
    let sabr = fixture.sabr().ok_or("sabr_smile body expected")?;
    if sabr.strikes.is_empty() {
        return Err("sabr_smile fixture must define at least one strike".to_string());
    }

    let strike_keys = sabr
        .strikes
        .iter()
        .map(|entry| entry.key.as_str())
        .collect::<BTreeSet<_>>();
    if strike_keys.len() != sabr.strikes.len() {
        return Err("sabr_smile strike keys must be unique".to_string());
    }
    let expected_keys = fixture
        .expected
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if strike_keys != expected_keys {
        return Err(
            "sabr_smile strike keys must match the expected metric keys exactly".to_string(),
        );
    }
    Ok(())
}

fn strip_default_instrument_inputs(value: &mut serde_json::Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };

    for child in object.values_mut() {
        strip_default_instrument_inputs(child);
    }

    remove_default_string(object, "coupon_type", "Cash");
    remove_default_string(object, "bdc", "modified_following");
    remove_default_string(object, "stub", "ShortFront");
    remove_default_string(object, "vol_surface_extrapolation", "error");
    remove_default_string(object, "bond_risk_basis", "bullet_discountable");
    remove_default_bool(object, "adaptive_bumps", false);
    remove_default_bool(object, "use_gobet_miri", false);
    remove_default_bool(object, "end_of_month", false);
    remove_default_i64(object, "payment_lag_days", 0);
    remove_default_f64(object, "vol_shift", 0.0);
    remove_default_f64(object, "rho_bump_decimal", 0.0001);
    remove_default_f64(object, "vega_bump_decimal", 0.0001);
    remove_empty_array(object, "discrete_dividends");
    remove_empty_object(object, "pricing_overrides");
}

fn remove_default_string(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: &str,
) {
    if object.get(key).and_then(serde_json::Value::as_str) == Some(default) {
        object.remove(key);
    }
}

fn remove_default_bool(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: bool,
) {
    if object.get(key).and_then(serde_json::Value::as_bool) == Some(default) {
        object.remove(key);
    }
}

fn remove_default_i64(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: i64,
) {
    if object.get(key).and_then(serde_json::Value::as_i64) == Some(default) {
        object.remove(key);
    }
}

fn remove_default_f64(
    object: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: f64,
) {
    if object
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .is_some_and(|value| (value - default).abs() < f64::EPSILON)
    {
        object.remove(key);
    }
}

fn remove_empty_array(object: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    if object
        .get(key)
        .and_then(serde_json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        object.remove(key);
    }
}

fn remove_empty_object(object: &mut serde_json::Map<String, serde_json::Value>, key: &str) {
    if object
        .get(key)
        .and_then(serde_json::Value::as_object)
        .is_some_and(serde_json::Map::is_empty)
    {
        object.remove(key);
    }
}

fn validate_swaption_underlying_tenor(instrument_json: &serde_json::Value) -> Result<(), String> {
    let instrument = instrument_json.get("instrument").unwrap_or(instrument_json);
    if instrument.get("type").and_then(serde_json::Value::as_str) != Some("swaption") {
        return Ok(());
    }
    let spec = instrument
        .get("spec")
        .and_then(serde_json::Value::as_object)
        .ok_or("swaption instrument.spec must be an object")?;
    let top_tenor = tenor_years(spec, "swap_start", "swap_end")?;
    let fixed_tenor = leg_tenor_years(spec, "underlying_fixed_leg")?;
    let float_tenor = leg_tenor_years(spec, "underlying_float_leg")?;
    if fixed_tenor != float_tenor {
        return Err(format!(
            "swaption underlying fixed/float leg tenors differ: fixed={fixed_tenor}y, float={float_tenor}y"
        ));
    }
    if top_tenor != fixed_tenor {
        return Err(format!(
            "swaption top-level tenor ({top_tenor}y) does not match underlying leg tenor ({fixed_tenor}y)"
        ));
    }
    Ok(())
}

fn leg_tenor_years(
    spec: &serde_json::Map<String, serde_json::Value>,
    leg_key: &str,
) -> Result<i32, String> {
    let leg = spec
        .get(leg_key)
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| format!("swaption {leg_key} must be an object"))?;
    tenor_years(leg, "start", "end")
}

fn tenor_years(
    object: &serde_json::Map<String, serde_json::Value>,
    start_key: &str,
    end_key: &str,
) -> Result<i32, String> {
    let start = object
        .get(start_key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("swaption {start_key} must be a date string"))?;
    let end = object
        .get(end_key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("swaption {end_key} must be a date string"))?;
    Ok(iso_year(end)? - iso_year(start)?)
}

fn iso_year(date: &str) -> Result<i32, String> {
    date.get(..4)
        .ok_or_else(|| format!("swaption date '{date}' is not YYYY-MM-DD"))?
        .parse::<i32>()
        .map_err(|err| format!("swaption date '{date}' has invalid year: {err}"))
}

fn validate_required_pricing_risk_metrics(fixture: &GoldenFixture) -> Result<(), String> {
    let domain = fixture.metadata.domain.as_str();
    if domain.starts_with("rates.") && !has_expected_metric(fixture, "dv01") {
        return Err("rates pricing fixtures must assert dv01".to_string());
    }

    if domain.starts_with("fixed_income.") && !has_expected_metric(fixture, "dv01") {
        return Err("fixed-income pricing fixtures must assert dv01".to_string());
    }

    if domain.starts_with("credit.") {
        if !has_expected_metric(fixture, "dv01") {
            return Err("credit pricing fixtures must assert dv01".to_string());
        }
        if !has_expected_metric(fixture, "cs01") {
            return Err("credit pricing fixtures must assert cs01".to_string());
        }
    }

    Ok(())
}

fn has_expected_metric(fixture: &GoldenFixture, base_metric: &str) -> bool {
    fixture
        .expected
        .keys()
        .any(|metric| metric_base(metric) == base_metric)
}

fn validate_zero_risk_metric_reasons(fixture: &GoldenFixture) -> Result<(), String> {
    for (metric, expected) in &fixture.expected {
        let base_metric = metric_base(metric);
        if expected.abs() <= f64::EPSILON
            && ZERO_RISK_METRICS_REQUIRING_REASON.contains(&base_metric)
            && !has_zero_metric_reason(fixture, metric)
        {
            return Err(format!(
                "zero risk metric '{metric}' requires a tolerances[metric].tolerance_reason"
            ));
        }
    }
    Ok(())
}

fn has_zero_metric_reason(fixture: &GoldenFixture, metric: &str) -> bool {
    fixture
        .tolerances
        .get(metric)
        .and_then(|tolerance| tolerance.tolerance_reason.as_deref())
        .is_some_and(|reason| !reason.trim().is_empty())
}

fn validate_non_empty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} is empty"));
    }
    Ok(())
}

fn validate_screenshot_paths(path: &Path, fixture: &GoldenFixture) -> Result<(), String> {
    let parent = path.parent().ok_or("fixture has no parent dir")?;
    for shot in &fixture.metadata.screenshots {
        let relative = Path::new(&shot.path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
            || relative.components().next() != Some(Component::Normal("screenshots".as_ref()))
        {
            return Err(format!(
                "screenshot '{}' must be a relative path under screenshots/",
                shot.path
            ));
        }
        match relative.extension().and_then(|ext| ext.to_str()) {
            Some("png" | "jpg" | "jpeg" | "webp") => {}
            _ => {
                return Err(format!(
                    "screenshot '{}' must use an image extension",
                    shot.path
                ));
            }
        }
        let shot_path = parent.join(relative);
        if !shot_path.exists() {
            return Err(format!(
                "screenshot '{}' does not exist (resolved to {:?})",
                shot.path, shot_path
            ));
        }
        if !is_git_tracked(&shot_path) {
            return Err(format!(
                "screenshot '{}' exists but is not tracked by git",
                shot.path
            ));
        }
    }
    Ok(())
}

fn metric_base(metric: &str) -> &str {
    metric.split_once("::").map_or(metric, |(base, _)| base)
}

fn is_git_tracked(path: &Path) -> bool {
    git_tracks(path) || legacy_pricing_path(path).is_some_and(|legacy| git_tracks(&legacy))
}

fn git_tracks(path: &Path) -> bool {
    Command::new("git")
        .arg("ls-files")
        .arg("--error-unmatch")
        .arg(path)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn legacy_pricing_path(path: &Path) -> Option<PathBuf> {
    let pricing_root = data_root().join("pricing");
    let relative = path.strip_prefix(&pricing_root).ok()?;
    let mut components = relative.components();
    let golden_type = components.next()?.as_os_str().to_str()?;
    if !matches!(golden_type, "regression_goldens" | "quantlib" | "bloomberg") {
        return None;
    }
    Some(pricing_root.join(components.as_path()))
}

fn fixture_relative_path(path: &Path) -> Result<String, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let data_root = Path::new(manifest_dir).join(DATA_ROOT);
    path.strip_prefix(data_root)
        .map(|relative| relative.to_string_lossy().to_string())
        .map_err(|err| format!("fixture path {path:?} is outside {DATA_ROOT}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CAP_FLOOR_FIXTURE: &str = "pricing/bloomberg/cap_floor/usd_cap_5y_atm_black.json";
    const DEPOSIT_FIXTURE: &str = "pricing/quantlib/deposit/usd_deposit_3m.json";
    const SWAPTION_FIXTURE: &str =
        "pricing/regression_goldens/swaption/usd_swaption_normal_vol_self_test.json";

    #[test]
    fn pricing_body_rejects_invalid_instrument() {
        let mut fixture = load_fixture(DEPOSIT_FIXTURE);
        let Body::Pricing(pricing) = &mut fixture.body else {
            panic!("deposit fixture must be a pricing fixture");
        };
        pricing.instrument = serde_json::json!({
            "schema": "finstack_quant.instrument/1",
            "instrument": {"type": "deposit", "spec": {}}
        });

        let err = validate_pricing_body(&fixture)
            .expect_err("invalid instrument must fail pricing walk validation");

        assert!(err.contains("instrument"), "unexpected error: {err}");
    }

    #[test]
    fn pricing_body_rejects_zero_risk_metric_without_reason() {
        let mut fixture = load_fixture(DEPOSIT_FIXTURE);
        fixture.expected.insert("dv01".to_string(), 0.0);
        fixture.tolerances.insert(
            "dv01".to_string(),
            crate::golden::schema::ToleranceEntry {
                abs: Some(1e-9),
                rel: None,
                tolerance_reason: None,
            },
        );

        let err = validate_zero_risk_metric_reasons(&fixture)
            .expect_err("zero dv01 without a reason must fail");

        assert!(err.contains("dv01"), "unexpected error: {err}");
    }

    #[test]
    fn manual_screenshot_paths_must_stay_under_screenshots_directory() {
        let path = data_root().join(CAP_FLOOR_FIXTURE);
        let mut fixture = load_fixture(CAP_FLOOR_FIXTURE);
        fixture.metadata.screenshots[0].path = "../usd_cap_5y_atm_black.json".to_string();

        let err = validate_screenshot_paths(&path, &fixture)
            .expect_err("manual screenshot evidence must not escape screenshots/");

        assert!(err.contains("screenshots/"), "unexpected error: {err}");
    }

    #[test]
    fn pricing_body_rejects_inconsistent_swaption_underlying_tenor() {
        let mut fixture = load_fixture(SWAPTION_FIXTURE);
        let Body::Pricing(pricing) = &mut fixture.body else {
            panic!("swaption fixture must be a pricing fixture");
        };
        pricing.instrument["instrument"]["spec"]["swap_end"] = serde_json::json!("2029-05-08");
        pricing.instrument["instrument"]["spec"]["underlying_fixed_leg"]["end"] =
            serde_json::json!("2032-05-05");
        pricing.instrument["instrument"]["spec"]["underlying_float_leg"]["end"] =
            serde_json::json!("2032-05-05");

        let err = validate_pricing_body(&fixture)
            .expect_err("swaption top-level tenor must agree with underlying leg tenors");

        assert!(err.contains("swaption"), "unexpected error: {err}");
    }

    #[test]
    fn top_level_keys_reject_unknown_field() {
        let path = data_root().join(DEPOSIT_FIXTURE);
        let raw = fs::read_to_string(&path).expect("read fixture");
        let mut value: serde_json::Value = serde_json::from_str(&raw).expect("parse");
        value["unexpected"] = serde_json::json!(true);
        let raw = serde_json::to_string(&value).expect("serialize");
        let fixture: GoldenFixture = serde_json::from_str(&raw).expect("reparse");

        let err = validate_top_level_keys(&raw, &fixture)
            .expect_err("unknown top-level key must be rejected");

        assert!(err.contains("unexpected"), "unexpected error: {err}");
    }

    fn load_fixture(relative_path: &str) -> GoldenFixture {
        let raw = fs::read_to_string(data_root().join(relative_path)).expect("read fixture");
        serde_json::from_str(&raw).expect("parse fixture")
    }
}

#[test]
fn all_fixtures_well_formed() {
    let failures = collect_fixture_paths()
        .iter()
        .filter_map(|path| {
            validate_fixture(path).err().map(|msg| {
                if msg.starts_with("__read_dir_error__:") {
                    msg
                } else {
                    format!("{}: {}", path.display(), msg)
                }
            })
        })
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "{} fixture(s) failed validation:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn pricing_fixture_discovery_uses_existing_json_files() {
    let pricing_paths = collect_fixture_paths_under("pricing")
        .expect("pricing fixture discovery should walk the pricing directory");
    let relatives = pricing_paths
        .iter()
        .map(|path| fixture_relative_path(path).expect("pricing fixture should be under data root"))
        .collect::<BTreeSet<_>>();

    assert!(relatives.contains("pricing/bloomberg/cds/cds_5y_par_spread.json"));
    assert!(relatives.contains("pricing/bloomberg/irs/usd_sofr_5y_receive_fixed_swpm.json"));
    assert!(!relatives.contains("pricing/bloomberg/cds/cds_5y_running_upfront.json"));
}

#[test]
fn pricing_instrument_json_accepts_omitted_golden_defaults() {
    let failures = collect_fixture_paths_under("pricing")
        .expect("pricing fixture discovery should walk the pricing directory")
        .into_iter()
        .filter_map(|path| {
            let relative =
                fixture_relative_path(&path).unwrap_or_else(|_| path.display().to_string());
            stripped_default_instrument_parse_error(&path).map(|err| format!("{relative}: {err}"))
        })
        .collect::<Vec<_>>();

    assert!(
        failures.is_empty(),
        "{} pricing fixture(s) require explicit default instrument inputs:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn stripped_default_instrument_parse_error(path: &Path) -> Option<String> {
    let raw = match fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(err) => return Some(format!("read failed: {err}")),
    };
    let fixture: GoldenFixture = match serde_json::from_str(&raw) {
        Ok(fixture) => fixture,
        Err(err) => return Some(format!("parse failed: {err}")),
    };
    let Some(pricing) = fixture.pricing() else {
        return Some("not a pricing fixture".to_string());
    };

    let mut instrument_json = pricing.instrument.clone();
    strip_default_instrument_inputs(&mut instrument_json);
    let instrument_json = match serde_json::to_string(&instrument_json) {
        Ok(instrument_json) => instrument_json,
        Err(err) => return Some(format!("serialize stripped instrument: {err}")),
    };

    parse_boxed_instrument_json(&instrument_json, None)
        .err()
        .map(|err| format!("stripped default instrument inputs failed to parse: {err}"))
}
