//! Registry-driven persistence coverage for every canonical instrument tag.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use finstack_quant_core::market_data::term_structures::HazardCurve;
use finstack_quant_models::credit::pool::StochasticDefaultSpec;
use finstack_quant_valuations::instruments::json_loader::{instrument_registry, registry_tags};
use finstack_quant_valuations::instruments::json_loader::{InstrumentEnvelope, InstrumentJson};
use serde::Deserialize;
use time::macros::date;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageManifest {
    schema_version: u32,
    #[serde(default)]
    non_persistable: Vec<NonPersistablePolicy>,
    instrument: Vec<CoverageEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NonPersistablePolicy {
    tag: String,
    case: String,
    expected_error_contains: Vec<String>,
    rationale: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageEntry {
    tag: String,
    fixture: String,
    lossless: bool,
    #[serde(default)]
    partial_eq_blocker: Option<String>,
    required_keys: Vec<String>,
    no_market_dependencies: bool,
    rationale: String,
}

fn coverage_manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("instruments")
        .join("coverage_manifest.toml")
}

fn load_manifest() -> CoverageManifest {
    let path = coverage_manifest_path();
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    toml::from_str(&contents).unwrap_or_else(|error| panic!("parse {}: {error}", path.display()))
}

fn fixture_path(entry: &CoverageEntry) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("instruments")
        .join(&entry.fixture)
}

fn fixture_envelope(entry: &CoverageEntry) -> InstrumentEnvelope {
    let path = fixture_path(entry);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
}

fn dependencies_are_empty(
    dependencies: &finstack_quant_valuations::instruments::MarketDependencies,
) -> bool {
    dependencies.curves.is_empty()
        && dependencies.market_scalar_ids.is_empty()
        && dependencies.volatility_dependencies.is_empty()
        && dependencies.fx_pairs.is_empty()
        && dependencies.series_ids.is_empty()
}

fn typed_semantic_equality(
    tag: &str,
    first: &InstrumentEnvelope,
    second: &InstrumentEnvelope,
) -> Option<bool> {
    match (tag, &first.instrument, &second.instrument) {
        (
            "inflation_linked_bond",
            InstrumentJson::InflationLinkedBond(first),
            InstrumentJson::InflationLinkedBond(second),
        ) => Some(first == second),
        (
            "agency_mbs_passthrough",
            InstrumentJson::AgencyMbsPassthrough(first),
            InstrumentJson::AgencyMbsPassthrough(second),
        ) => Some(first == second),
        ("agency_tba", InstrumentJson::AgencyTba(first), InstrumentJson::AgencyTba(second)) => {
            Some(first == second)
        }
        ("dollar_roll", InstrumentJson::DollarRoll(first), InstrumentJson::DollarRoll(second)) => {
            Some(first == second)
        }
        (
            "inflation_swap",
            InstrumentJson::InflationSwap(first),
            InstrumentJson::InflationSwap(second),
        ) => Some(first == second),
        (
            "yoy_inflation_swap",
            InstrumentJson::YoYInflationSwap(first),
            InstrumentJson::YoYInflationSwap(second),
        ) => Some(first == second),
        (
            "inflation_cap_floor",
            InstrumentJson::InflationCapFloor(first),
            InstrumentJson::InflationCapFloor(second),
        ) => Some(first == second),
        (
            "forward_rate_agreement",
            InstrumentJson::ForwardRateAgreement(first),
            InstrumentJson::ForwardRateAgreement(second),
        ) => Some(first == second),
        (
            "interest_rate_future",
            InstrumentJson::InterestRateFuture(first),
            InstrumentJson::InterestRateFuture(second),
        ) => Some(first == second),
        (
            "commodity_future",
            InstrumentJson::CommodityFuture(first),
            InstrumentJson::CommodityFuture(second),
        ) => Some(first == second),
        ("fx_future", InstrumentJson::FxFuture(first), InstrumentJson::FxFuture(second)) => {
            Some(first == second)
        }
        (
            "equity_future",
            InstrumentJson::EquityFuture(first),
            InstrumentJson::EquityFuture(second),
        ) => Some(first == second),
        (
            "equity_total_return_future",
            InstrumentJson::EquityTotalReturnFuture(first),
            InstrumentJson::EquityTotalReturnFuture(second),
        ) => Some(first == second),
        (
            "interest_rate_future_option",
            InstrumentJson::InterestRateFutureOption(first),
            InstrumentJson::InterestRateFutureOption(second),
        ) => Some(first == second),
        (
            "equity_future_option",
            InstrumentJson::EquityFutureOption(first),
            InstrumentJson::EquityFutureOption(second),
        ) => Some(first == second),
        (
            "volatility_index_future_option",
            InstrumentJson::VolatilityIndexFutureOption(first),
            InstrumentJson::VolatilityIndexFutureOption(second),
        ) => Some(first == second),
        (
            "fx_future_option",
            InstrumentJson::FxFutureOption(first),
            InstrumentJson::FxFutureOption(second),
        ) => Some(first == second),
        (
            "commodity_future_option",
            InstrumentJson::CommodityFutureOption(first),
            InstrumentJson::CommodityFutureOption(second),
        ) => Some(first == second),
        ("cap_floor", InstrumentJson::CapFloor(first), InstrumentJson::CapFloor(second)) => {
            Some(first == second)
        }
        ("cms_option", InstrumentJson::CmsOption(first), InstrumentJson::CmsOption(second)) => {
            Some(first == second)
        }
        ("deposit", InstrumentJson::Deposit(first), InstrumentJson::Deposit(second)) => {
            Some(first == second)
        }
        ("repo", InstrumentJson::Repo(first), InstrumentJson::Repo(second)) => {
            Some(first == second)
        }
        ("cds_tranche", InstrumentJson::CDSTranche(first), InstrumentJson::CDSTranche(second)) => {
            Some(first == second)
        }
        ("cds_option", InstrumentJson::CDSOption(first), InstrumentJson::CDSOption(second)) => {
            Some(first == second)
        }
        ("equity", InstrumentJson::Equity(first), InstrumentJson::Equity(second)) => {
            Some(first == second)
        }
        (
            "equity_option",
            InstrumentJson::EquityOption(first),
            InstrumentJson::EquityOption(second),
        ) => Some(first == second),
        (
            "asian_option",
            InstrumentJson::AsianOption(first),
            InstrumentJson::AsianOption(second),
        ) => Some(first == second),
        (
            "barrier_option",
            InstrumentJson::BarrierOption(first),
            InstrumentJson::BarrierOption(second),
        ) => Some(first == second),
        (
            "lookback_option",
            InstrumentJson::LookbackOption(first),
            InstrumentJson::LookbackOption(second),
        ) => Some(first == second),
        (
            "variance_swap",
            InstrumentJson::VarianceSwap(first),
            InstrumentJson::VarianceSwap(second),
        ) => Some(first == second),
        ("fx_spot", InstrumentJson::FxSpot(first), InstrumentJson::FxSpot(second)) => {
            Some(first == second)
        }
        ("fx_swap", InstrumentJson::FxSwap(first), InstrumentJson::FxSwap(second)) => {
            Some(first == second)
        }
        ("fx_forward", InstrumentJson::FxForward(first), InstrumentJson::FxForward(second)) => {
            Some(first == second)
        }
        ("ndf", InstrumentJson::Ndf(first), InstrumentJson::Ndf(second)) => Some(first == second),
        ("fx_option", InstrumentJson::FxOption(first), InstrumentJson::FxOption(second)) => {
            Some(first == second)
        }
        (
            "fx_digital_option",
            InstrumentJson::FxDigitalOption(first),
            InstrumentJson::FxDigitalOption(second),
        ) => Some(first == second),
        (
            "fx_touch_option",
            InstrumentJson::FxTouchOption(first),
            InstrumentJson::FxTouchOption(second),
        ) => Some(first == second),
        (
            "fx_barrier_option",
            InstrumentJson::FxBarrierOption(first),
            InstrumentJson::FxBarrierOption(second),
        ) => Some(first == second),
        (
            "fx_variance_swap",
            InstrumentJson::FxVarianceSwap(first),
            InstrumentJson::FxVarianceSwap(second),
        ) => Some(first == second),
        (
            "quanto_option",
            InstrumentJson::QuantoOption(first),
            InstrumentJson::QuantoOption(second),
        ) => Some(first == second),
        (
            "commodity_spread_option",
            InstrumentJson::CommoditySpreadOption(first),
            InstrumentJson::CommoditySpreadOption(second),
        ) => Some(first == second),
        (
            "autocallable",
            InstrumentJson::Autocallable(first),
            InstrumentJson::Autocallable(second),
        ) => Some(first == second),
        (
            "cliquet_option",
            InstrumentJson::CliquetOption(first),
            InstrumentJson::CliquetOption(second),
        ) => Some(first == second),
        ("tarn", InstrumentJson::Tarn(first), InstrumentJson::Tarn(second)) => {
            Some(first == second)
        }
        ("snowball", InstrumentJson::Snowball(first), InstrumentJson::Snowball(second)) => {
            Some(first == second)
        }
        (
            "cms_spread_option",
            InstrumentJson::CmsSpreadOption(first),
            InstrumentJson::CmsSpreadOption(second),
        ) => Some(first == second),
        (
            "private_markets_fund",
            InstrumentJson::PrivateMarketsFund(first),
            InstrumentJson::PrivateMarketsFund(second),
        ) => Some(first == second),
        (
            "real_estate_asset",
            InstrumentJson::RealEstateAsset(first),
            InstrumentJson::RealEstateAsset(second),
        ) => Some(first == second),
        _ => None,
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else {
        "non-string panic payload".to_string()
    }
}

#[test]
fn registry_coverage_manifest_is_checked_in() {
    let path = coverage_manifest_path();
    assert!(
        path.is_file(),
        "instrument registry coverage manifest is missing: {}",
        path.display()
    );
}

/// Mirror of `scripts/check_generated_instrument_fixtures.py` for in-process Rust tests.
///
/// Kept in Rust so `mise run rust-test` / CI Test Rust do not require `uv` or a
/// Python runtime. The Python script remains the mise/`gen-check` entry point.
fn generated_fixtures_are_valid(root: &Path) -> bool {
    const EXPECTED_CANONICAL_FIXTURES: usize = 78;
    let manifest_path = root
        .join("finstack-quant")
        .join("valuations")
        .join("tests")
        .join("instruments")
        .join("coverage_manifest.toml");
    let Ok(contents) = fs::read_to_string(&manifest_path) else {
        return false;
    };
    let Ok(manifest) = toml::from_str::<CoverageManifest>(&contents) else {
        return false;
    };
    if manifest.instrument.len() != EXPECTED_CANONICAL_FIXTURES {
        return false;
    }

    let instruments_dir = Path::new("finstack-quant")
        .join("valuations")
        .join("tests")
        .join("instruments");
    let mut expected_paths = BTreeSet::new();
    let mut entries = Vec::new();
    for entry in &manifest.instrument {
        let relative = instruments_dir.join(&entry.fixture);
        if !expected_paths.insert(relative.clone()) {
            return false;
        }
        entries.push((entry.tag.as_str(), relative));
    }

    let fixture_root = root.join(&instruments_dir).join("json_examples");
    let Ok(dir) = fs::read_dir(&fixture_root) else {
        return false;
    };
    let mut actual_paths = BTreeSet::new();
    for entry in dir {
        let Ok(entry) = entry else {
            return false;
        };
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        actual_paths.insert(relative.to_path_buf());
    }
    if actual_paths != expected_paths {
        return false;
    }

    for (tag, relative_path) in entries {
        let path = root.join(&relative_path);
        let Ok(bytes) = fs::read(&path) else {
            return false;
        };
        let Ok(document) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return false;
        };
        if document.get("schema").and_then(serde_json::Value::as_str)
            != Some("finstack_quant.instrument/1")
        {
            return false;
        }
        let Some(instrument) = document
            .get("instrument")
            .and_then(serde_json::Value::as_object)
        else {
            return false;
        };
        if instrument.get("type").and_then(serde_json::Value::as_str) != Some(tag) {
            return false;
        }
    }
    true
}

#[test]
fn generator_freshness_check_rejects_non_registry_fixtures() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch")
        .as_nanos();
    let temp_root = std::env::temp_dir().join(format!(
        "finstack-generated-fixture-check-{}-{nonce}",
        std::process::id()
    ));
    let manifest_dir = temp_root
        .join("finstack-quant")
        .join("valuations")
        .join("tests")
        .join("instruments");
    let examples_dir = manifest_dir.join("json_examples");
    fs::create_dir_all(&examples_dir).expect("create temporary fixture tree");
    let manifest = load_manifest();
    fs::copy(
        coverage_manifest_path(),
        manifest_dir.join("coverage_manifest.toml"),
    )
    .expect("copy coverage manifest");
    for entry in &manifest.instrument {
        let destination = manifest_dir.join(&entry.fixture);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).expect("create canonical fixture directory");
        }
        fs::copy(fixture_path(entry), destination).expect("copy canonical fixture");
    }
    let canonical = examples_dir.join("bond.json");
    let auxiliary = examples_dir.join("basket_with_instruments.json");

    assert!(
        generated_fixtures_are_valid(&temp_root),
        "canonical fixture inventory must pass"
    );
    fs::write(&auxiliary, "{\"auxiliary\":true}\n").expect("write auxiliary fixture");
    assert!(
        !generated_fixtures_are_valid(&temp_root),
        "non-registry fixtures must fail generated freshness"
    );
    fs::remove_file(&auxiliary).expect("remove auxiliary fixture");
    fs::write(&canonical, "{\"stale\":true}\n").expect("modify canonical fixture");
    assert!(
        !generated_fixtures_are_valid(&temp_root),
        "canonical fixture changes must fail generated freshness"
    );

    fs::remove_dir_all(&temp_root).expect("remove temporary fixture tree");
}

#[test]
fn public_registry_lists_only_canonical_tags() {
    let registry = registry_tags();
    assert_eq!(registry.len(), 78, "canonical registry tag count drifted");
    assert!(registry.contains(&"snowball"));
    assert!(!registry.contains(&"inverse_floater"));
}

#[test]
fn persisted_tags_match_runtime_instrument_types() {
    for entry in instrument_registry() {
        let examples = entry
            .examples()
            .unwrap_or_else(|error| panic!("build {} example: {error}", entry.tag));
        let [example] = examples.as_slice() else {
            panic!("{} must provide exactly one example", entry.tag);
        };
        let envelope: InstrumentEnvelope = serde_json::from_value(example.clone())
            .unwrap_or_else(|error| panic!("parse {} example: {error}", entry.tag));
        assert_eq!(envelope.instrument.type_tag(), entry.tag);
        let runtime = envelope
            .into_boxed()
            .unwrap_or_else(|error| panic!("build {} runtime instrument: {error}", entry.tag));
        assert_eq!(
            runtime.key().as_str(),
            entry.tag,
            "runtime InstrumentType and persisted instrument tag diverged"
        );
    }
}

#[test]
fn declared_non_persistable_cases_are_exercised() {
    let manifest = load_manifest();
    let registry: BTreeSet<&str> = registry_tags().iter().copied().collect();
    let mut exercised = BTreeSet::new();

    for policy in &manifest.non_persistable {
        assert!(
            registry.contains(policy.tag.as_str()),
            "non-persistable policy references unknown tag {}",
            policy.tag
        );
        assert!(
            !policy.rationale.trim().is_empty(),
            "{}:{} requires a substantive rationale",
            policy.tag,
            policy.case
        );
        assert!(
            exercised.insert((policy.tag.as_str(), policy.case.as_str())),
            "duplicate non-persistable policy {}:{}",
            policy.tag,
            policy.case
        );

        let error = match (policy.tag.as_str(), policy.case.as_str()) {
            ("structured_credit", "stochastic_default_hazard_curve_based") => {
                let curve = HazardCurve::builder("REGISTRY-COVERAGE-HAZARD")
                    .base_date(date!(2026 - 01 - 01))
                    .knots([(1.0, 0.01), (5.0, 0.02)])
                    .recovery_rate(0.40)
                    .build()
                    .expect("valid hazard curve for persistence-policy coverage");
                let spec = StochasticDefaultSpec::from_hazard_curve(curve, 0.5);
                serde_json::to_string(&spec)
                    .expect_err("HazardCurveBased must reject typed persistence")
                    .to_string()
            }
            (tag, case) => {
                panic!("declared non-persistable policy is not exercised: {tag}:{case}")
            }
        };

        for expected in &policy.expected_error_contains {
            assert!(
                error.contains(expected),
                "{}:{} typed serialization error {error:?} omitted required rationale token {expected:?}",
                policy.tag,
                policy.case
            );
        }
    }

    assert!(
        !exercised.is_empty(),
        "the manifest must exercise at least one explicit non-persistable case"
    );
}

#[test]
fn registry_manifest_and_fixtures_are_consistent() {
    let manifest = load_manifest();
    assert_eq!(manifest.schema_version, 1);

    let registry: BTreeSet<&str> = registry_tags().iter().copied().collect();
    assert_eq!(
        registry.len(),
        registry_tags().len(),
        "canonical registry tags must be unique"
    );

    let mut entries = BTreeMap::new();
    let mut declared_fixtures = BTreeSet::new();
    let mut no_partial_eq_rationales = BTreeSet::new();
    for entry in &manifest.instrument {
        assert!(
            entries.insert(entry.tag.as_str(), entry).is_none(),
            "duplicate manifest entry for {}",
            entry.tag
        );
        assert!(
            !entry.rationale.trim().is_empty(),
            "{} must document its persistence policy",
            entry.tag
        );
        assert!(
            entry.lossless || !entry.rationale.trim().is_empty(),
            "{} must explain byte-equality-only coverage",
            entry.tag
        );
        if !entry.lossless {
            let blocker = entry
                .partial_eq_blocker
                .as_deref()
                .unwrap_or_else(|| panic!("{} requires a concrete partial_eq_blocker", entry.tag));
            let (field, blocker_type) = blocker.split_once(':').unwrap_or_else(|| {
                panic!("{} blocker must use `Type.field: NestedType`", entry.tag)
            });
            assert!(
                field.contains('.')
                    && blocker_type
                        .trim()
                        .chars()
                        .next()
                        .is_some_and(char::is_uppercase),
                "{} blocker must name a concrete Rust field and type",
                entry.tag
            );
            assert!(
                entry.rationale.contains(blocker) && entry.rationale.contains("lacks PartialEq"),
                "{} rationale must name its declared concrete blocker {blocker:?}",
                entry.tag
            );
            assert!(
                !entry
                    .rationale
                    .contains("canonical byte stability remains independently enforced")
                    && !entry.rationale.contains("All persisted"),
                "{} rationale still uses the rejected copy template",
                entry.tag
            );
            assert!(
                no_partial_eq_rationales.insert(entry.rationale.as_str()),
                "{} duplicates another no-PartialEq rationale",
                entry.tag
            );
        } else {
            assert!(
                entry.partial_eq_blocker.is_none(),
                "{} has typed equality and must not declare a blocker",
                entry.tag
            );
        }
        assert!(
            entry.lossless
                == typed_semantic_equality(
                    &entry.tag,
                    &fixture_envelope(entry),
                    &fixture_envelope(entry)
                )
                .is_some(),
            "{} lossless policy must exactly match concrete typed PartialEq coverage",
            entry.tag
        );
        assert!(
            !entry.required_keys.is_empty(),
            "{} must declare at least one required spec key",
            entry.tag
        );

        let fixture_path = fixture_path(entry);
        assert!(
            fixture_path.is_file(),
            "{} fixture is missing: {}",
            entry.tag,
            fixture_path.display()
        );
        assert!(
            declared_fixtures.insert(entry.fixture.as_str()),
            "fixture {} is declared more than once",
            entry.fixture
        );
        let fixture: serde_json::Value = serde_json::from_slice(
            &fs::read(&fixture_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", fixture_path.display())),
        )
        .unwrap_or_else(|error| panic!("parse {}: {error}", fixture_path.display()));
        assert_eq!(
            fixture
                .pointer("/instrument/type")
                .and_then(serde_json::Value::as_str),
            Some(entry.tag.as_str()),
            "{} fixture discriminator does not match its manifest tag",
            entry.tag
        );
    }

    let manifest_tags: BTreeSet<&str> = entries.keys().copied().collect();
    assert_eq!(
        manifest_tags, registry,
        "registry and coverage manifest tags must match exactly"
    );

    let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("instruments")
        .join("json_examples");
    let actual_fixtures: BTreeSet<String> = fs::read_dir(&examples_dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", examples_dir.display()))
        .map(|entry| {
            let path = entry
                .unwrap_or_else(|error| panic!("read fixture directory entry: {error}"))
                .path();
            format!(
                "json_examples/{}",
                path.file_name()
                    .unwrap_or_else(|| panic!("fixture path has no filename: {}", path.display()))
                    .to_string_lossy()
            )
        })
        .collect();
    let declared_fixtures: BTreeSet<String> =
        declared_fixtures.into_iter().map(str::to_string).collect();
    assert_eq!(
        actual_fixtures, declared_fixtures,
        "every checked-in JSON fixture must be owned by one canonical registry tag"
    );
}

#[test]
fn every_registered_instrument_satisfies_persistence_contract() {
    let manifest = load_manifest();
    let entries: BTreeMap<&str, &CoverageEntry> = manifest
        .instrument
        .iter()
        .map(|entry| (entry.tag.as_str(), entry))
        .collect();
    let limits = finstack_quant_core::LoadLimits::default();
    let mut failures = Vec::new();

    for tag in registry_tags() {
        let entry = entries
            .get(tag)
            .unwrap_or_else(|| panic!("{tag}: canonical registry tag is absent from manifest"));
        let result = std::panic::catch_unwind(|| {
            let path = fixture_path(entry);
            let bytes = fs::read(&path)
                .unwrap_or_else(|error| panic!("{tag}: read {}: {error}", path.display()));
            let fixture: serde_json::Value = serde_json::from_slice(&bytes)
                .unwrap_or_else(|error| panic!("{tag}: parse {}: {error}", path.display()));

            finstack_quant_valuations::schema::instrument_schema(tag)
                .unwrap_or_else(|error| panic!("{tag}: generated schema is unavailable: {error}"));
            finstack_quant_valuations::schema::validate_instrument_envelope_json(&fixture)
                .unwrap_or_else(|error| panic!("{tag}: fixture fails generated schema: {error}"));

            let (first_runtime, first_report) =
                InstrumentEnvelope::from_slice_strict(&bytes, &limits)
                    .unwrap_or_else(|error| panic!("{tag}: strict fixture decode failed: {error}"));
            assert!(
                first_report.diagnostics.is_empty(),
                "{tag}: current fixture emitted strict-load diagnostics: {:?}",
                first_report.diagnostics
            );
            first_runtime
                .validate_for_pricing()
                .unwrap_or_else(|error| panic!("{tag}: semantic validation failed: {error}"));
            let runtime_json = first_runtime.to_instrument_json().unwrap_or_else(|| {
                panic!("{tag}: runtime instrument is not registry-serializable")
            });
            let runtime_value = serde_json::to_value(runtime_json).unwrap_or_else(|error| {
                panic!("{tag}: runtime JSON serialization failed: {error}")
            });
            assert_eq!(
                runtime_value
                    .get("type")
                    .and_then(serde_json::Value::as_str),
                Some(*tag),
                "{tag}: runtime -> InstrumentJson emitted a non-canonical discriminator"
            );

            let first_envelope: InstrumentEnvelope = serde_json::from_slice(&bytes)
                .unwrap_or_else(|error| panic!("{tag}: typed envelope decode failed: {error}"));
            let first_canonical = finstack_quant_core::to_canonical_bytes(&first_envelope)
                .unwrap_or_else(|error| panic!("{tag}: canonical serialization failed: {error}"));

            let (second_runtime, second_report) =
                InstrumentEnvelope::from_slice_strict(&first_canonical, &limits)
                    .unwrap_or_else(|error| panic!("{tag}: second strict decode failed: {error}"));
            assert!(
                second_report.diagnostics.is_empty(),
                "{tag}: canonical payload emitted strict-load diagnostics: {:?}",
                second_report.diagnostics
            );
            second_runtime
                .validate_for_pricing()
                .unwrap_or_else(|error| {
                    panic!("{tag}: second semantic validation failed: {error}")
                });

            let second_envelope: InstrumentEnvelope = serde_json::from_slice(&first_canonical)
                .unwrap_or_else(|error| panic!("{tag}: canonical typed decode failed: {error}"));
            let second_canonical = finstack_quant_core::to_canonical_bytes(&second_envelope)
                .unwrap_or_else(|error| {
                    panic!("{tag}: second canonical serialization failed: {error}")
                });
            assert_eq!(
                first_canonical, second_canonical,
                "{tag}: canonical bytes changed after strict decode"
            );

            if entry.lossless {
                assert_eq!(
                    typed_semantic_equality(tag, &first_envelope, &second_envelope),
                    Some(true),
                    "{tag}: concrete typed PartialEq detected a semantic round-trip change"
                );
            } else {
                assert!(
                    !entry.rationale.trim().is_empty(),
                    "{tag}: byte-equality-only policy requires a rationale"
                );
            }

            let dependencies = first_runtime
                .market_dependencies()
                .unwrap_or_else(|error| panic!("{tag}: dependency extraction failed: {error}"));
            assert!(
            entry.no_market_dependencies || !dependencies_are_empty(&dependencies),
            "{tag}: dependencies are empty; set no_market_dependencies=true with rationale only if intentional"
        );

            let mut unknown_top = fixture.clone();
            unknown_top
                .as_object_mut()
                .unwrap_or_else(|| panic!("{tag}: envelope must be an object"))
                .insert("__unknown_top_level__".to_string(), serde_json::json!(true));
            let unknown_top_bytes = serde_json::to_vec(&unknown_top)
                .unwrap_or_else(|error| panic!("{tag}: serialize top-level mutation: {error}"));
            assert!(
                InstrumentEnvelope::from_slice_strict(&unknown_top_bytes, &limits).is_err(),
                "{tag}: strict decode accepted a top-level unknown field"
            );

            let mut unknown_nested = fixture.clone();
            unknown_nested
                .pointer_mut("/instrument/spec")
                .and_then(serde_json::Value::as_object_mut)
                .unwrap_or_else(|| panic!("{tag}: fixture spec must be an object"))
                .insert("__unknown_nested__".to_string(), serde_json::json!(true));
            let unknown_nested_bytes = serde_json::to_vec(&unknown_nested)
                .unwrap_or_else(|error| panic!("{tag}: serialize nested mutation: {error}"));
            assert!(
                InstrumentEnvelope::from_slice_strict(&unknown_nested_bytes, &limits).is_err(),
                "{tag}: strict decode accepted a one-level-nested unknown spec field"
            );

            for required_key in &entry.required_keys {
                let mut missing_required = fixture.clone();
                let removed = missing_required
                    .pointer_mut("/instrument/spec")
                    .and_then(serde_json::Value::as_object_mut)
                    .unwrap_or_else(|| panic!("{tag}: fixture spec must be an object"))
                    .remove(required_key);
                assert!(
                    removed.is_some(),
                    "{tag}: manifest required key {required_key:?} is absent from fixture"
                );
                let missing_required_bytes =
                    serde_json::to_vec(&missing_required).unwrap_or_else(|error| {
                        panic!("{tag}: serialize required-key mutation: {error}")
                    });
                assert!(
                    InstrumentEnvelope::from_slice_strict(&missing_required_bytes, &limits)
                        .is_err(),
                    "{tag}: strict decode accepted missing required key {required_key:?}"
                );
            }
        });
        if let Err(payload) = result {
            failures.push(panic_message(payload));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} canonical instrument fixtures failed persistence coverage:\n{}",
        failures.len(),
        registry_tags().len(),
        failures.join("\n")
    );
}
