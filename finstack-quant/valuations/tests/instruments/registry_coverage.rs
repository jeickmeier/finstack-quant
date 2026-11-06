//! Registry-driven persistence coverage for every canonical instrument tag.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use finstack_quant_valuations::instruments::json_loader::InstrumentEnvelope;
use finstack_quant_valuations::instruments::json_loader::instrument_json_of;
use finstack_quant_valuations::instruments::json_loader::instrument_registry;
use finstack_quant_valuations::pricer::InstrumentType;
use finstack_quant_valuations::schema::instrument_types;
use serde::Deserialize;
use strum::IntoEnumIterator;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageManifest {
    schema_version: u32,
    instrument: Vec<CoverageEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageEntry {
    tag: String,
    /// `Type.field: NestedType` naming the field whose type lacks `PartialEq`;
    /// absent exactly when `InstrumentJson::semantic_eq` compares the type.
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

/// The canonical fixture `gen-write` writes for a tag, relative to `tests/instruments/`.
fn fixture_relative(tag: &str) -> String {
    format!("json_examples/{tag}.json")
}

fn fixture_path(entry: &CoverageEntry) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("instruments")
        .join(fixture_relative(&entry.tag))
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
    if manifest.instrument.len() != instrument_types().len() {
        return false;
    }

    let instruments_dir = Path::new("finstack-quant")
        .join("valuations")
        .join("tests")
        .join("instruments");
    let mut expected_paths = BTreeSet::new();
    let mut entries = Vec::new();
    for entry in &manifest.instrument {
        let relative = instruments_dir.join(fixture_relative(&entry.tag));
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
        let destination = manifest_dir.join(fixture_relative(&entry.tag));
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
    let registry = instrument_types();
    assert_eq!(
        registry.len(),
        InstrumentType::iter().count(),
        "every InstrumentType has exactly one canonical registry tag"
    );
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
fn registry_manifest_and_fixtures_are_consistent() {
    let manifest = load_manifest();
    assert_eq!(manifest.schema_version, 1);

    let registry: BTreeSet<&str> = instrument_types().iter().copied().collect();
    assert_eq!(
        registry.len(),
        instrument_types().len(),
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
        let typed_equality = fixture_envelope(entry)
            .instrument
            .semantic_eq(&fixture_envelope(entry).instrument);
        if let Some(blocker) = entry.partial_eq_blocker.as_deref() {
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
        }
        assert_eq!(
            entry.partial_eq_blocker.is_none(),
            typed_equality.is_some(),
            "{}: partial_eq_blocker must be set exactly when the Rust type lacks PartialEq",
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
        declared_fixtures.insert(fixture_relative(&entry.tag));
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
    let limits = finstack_quant_core::contract::LoadLimits::default();
    let mut failures = Vec::new();

    for tag in instrument_types() {
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
            let runtime_json = instrument_json_of(first_runtime.as_ref()).unwrap_or_else(|| {
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
            let first_canonical = finstack_quant_core::canonical::to_canonical_bytes(
                &first_envelope,
            )
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
            let second_canonical =
                finstack_quant_core::canonical::to_canonical_bytes(&second_envelope)
                    .unwrap_or_else(|error| {
                        panic!("{tag}: second canonical serialization failed: {error}")
                    });
            assert_eq!(
                first_canonical, second_canonical,
                "{tag}: canonical bytes changed after strict decode"
            );

            if let Some(equal) = first_envelope
                .instrument
                .semantic_eq(&second_envelope.instrument)
            {
                assert!(
                    equal,
                    "{tag}: concrete typed PartialEq detected a semantic round-trip change"
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
        instrument_types().len(),
        failures.join("\n")
    );
}
