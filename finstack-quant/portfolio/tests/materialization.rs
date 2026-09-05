//! Contract and fast-path tests for portfolio materialization bundles.

use std::any::Any;
use std::sync::{Arc, Barrier};
use std::thread;

use finstack_quant_core::contract::LoadLimits;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_core::{content_hash, to_canonical_bytes};
use finstack_quant_portfolio::materialization::{
    InstrumentArtifact, InstrumentArtifactCache, MaterializedPosition, PortfolioHeader,
    PortfolioMaterializationEnvelope,
};
use finstack_quant_portfolio::position::{Position, PositionUnit};
use finstack_quant_portfolio::types::{Entity, PositionId};
use finstack_quant_portfolio::{Error, Portfolio};
use finstack_quant_valuations::instruments::rates::deposit::Deposit;
use finstack_quant_valuations::instruments::{
    Attributes, Instrument, InstrumentEnvelope, InstrumentJson, MarketDependencies,
    VolatilityDependency,
};
use finstack_quant_valuations::pricer::InstrumentType;
use indexmap::IndexMap;
use time::macros::date;

fn deposit_envelope(index: usize) -> InstrumentEnvelope {
    let deposit = Deposit::builder()
        .id(format!("DEP-{index}").into())
        .notional(
            Money::new(1_000_000.0 + index as f64, Currency::USD).expect("valid money fixture"),
        )
        .start_date(date!(2025 - 01 - 01))
        .maturity(date!(2025 - 02 - 01))
        .day_count(finstack_quant_core::dates::DayCount::Act360)
        .discount_curve_id("USD-OIS".into())
        .build()
        .expect("deposit fixture");
    InstrumentEnvelope::new(InstrumentJson::Deposit(deposit))
}

fn artifact(index: usize) -> InstrumentArtifact {
    let envelope = deposit_envelope(index);
    InstrumentArtifact {
        artifact_id: format!("artifact-{index}"),
        content_hash: Some(envelope.content_hash().expect("hash fixture")),
        envelope,
        dependencies: None,
    }
}

fn position(index: usize, artifact_index: usize) -> MaterializedPosition {
    MaterializedPosition {
        id: PositionId::new(format!("position-{index}")),
        entity_id: "entity".into(),
        instrument_id: format!("DEP-{artifact_index}"),
        artifact_id: format!("artifact-{artifact_index}"),
        quantity: index as f64 + 1.0,
        unit: PositionUnit::Units,
        attributes: IndexMap::new(),
        meta: IndexMap::new(),
    }
}

fn envelope(artifact_count: usize, position_count: usize) -> PortfolioMaterializationEnvelope {
    PortfolioMaterializationEnvelope {
        schema: finstack_quant_portfolio::materialization::PortfolioMaterializationSchema::CURRENT,
        portfolio: PortfolioHeader {
            id: "portfolio".to_string(),
            name: Some("Materialized Portfolio".to_string()),
            base_currency: Currency::USD,
            as_of: date!(2025 - 01 - 01),
            entities: IndexMap::from([("entity".into(), Entity::new("entity"))]),
            books: IndexMap::new(),
            tags: IndexMap::from([("desk".to_string(), "rates".to_string())]),
            meta: IndexMap::new(),
        },
        instruments: (0..artifact_count).map(artifact).collect(),
        positions: (0..position_count)
            .map(|index| position(index, index % artifact_count))
            .collect(),
        materializer: None,
    }
}

#[test]
fn materialization_envelope_has_exact_canonical_bytes_and_hash() {
    let envelope = envelope(1, 1);
    let canonical = to_canonical_bytes(&envelope).expect("materialization envelope canonicalizes");
    let hash = content_hash(&envelope).expect("materialization envelope hashes");
    let canonical_value: serde_json::Value =
        serde_json::from_slice(&canonical).expect("canonical materialization is JSON");
    let instrument_envelope = &canonical_value["instruments"][0]["envelope"];
    assert_eq!(instrument_envelope["schema"], "finstack_quant.instrument/1");
    assert!(instrument_envelope["instrument"].is_object());
    assert!(
        instrument_envelope
            .as_object()
            .expect("instrument envelope is an object")
            .keys()
            .all(|key| !key.starts_with("$serde_json")),
        "canonical typed envelope must not expose serde_json internals"
    );
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("canonical");
    let json_path = directory.join("portfolio_materialization.json");
    let hash_path = directory.join("portfolio_materialization.sha256");

    if std::env::var_os("FQ_UPDATE_CANONICAL_GOLDENS").is_some() {
        std::fs::create_dir_all(&directory).expect("create canonical fixture directory");
        std::fs::write(&json_path, &canonical).expect("write materialization canonical fixture");
        std::fs::write(&hash_path, format!("{hash}\n"))
            .expect("write materialization hash fixture");
    }

    assert_eq!(
        canonical,
        std::fs::read(&json_path).expect("read materialization canonical fixture")
    );
    assert_eq!(
        hash,
        std::fs::read_to_string(&hash_path)
            .expect("read materialization hash fixture")
            .trim()
    );
}

fn load(
    envelope: &PortfolioMaterializationEnvelope,
    cache: &InstrumentArtifactCache,
    limits: &LoadLimits,
) -> Result<(Portfolio, finstack_quant_portfolio::MaterializationReport), Error> {
    let bytes = serde_json::to_vec(envelope).expect("serialize fixture");
    Portfolio::from_materialization(&bytes, cache, limits)
}

fn report_from_error(error: Error) -> finstack_quant_core::contract::ValidationReport {
    match error {
        Error::MaterializationFailed(report) => *report,
        other => panic!("expected materialization diagnostics, got {other:?}"),
    }
}

fn nested_json(depth: usize) -> serde_json::Value {
    (0..depth).fold(
        serde_json::json!("leaf"),
        |value, _| serde_json::json!({"nested": value}),
    )
}

#[test]
fn materialization_types_reject_unknown_fields() {
    let mut value = serde_json::to_value(envelope(1, 1)).expect("serialize fixture");
    value
        .as_object_mut()
        .expect("envelope object")
        .insert("unknown".to_string(), serde_json::json!(true));
    assert!(serde_json::from_value::<PortfolioMaterializationEnvelope>(value).is_err());

    let mut value = serde_json::to_value(envelope(1, 1)).expect("serialize fixture");
    value["portfolio"]["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<PortfolioMaterializationEnvelope>(value).is_err());

    let mut value = serde_json::to_value(envelope(1, 1)).expect("serialize fixture");
    value["instruments"][0]["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<PortfolioMaterializationEnvelope>(value).is_err());

    let mut value = serde_json::to_value(envelope(1, 1)).expect("serialize fixture");
    value["positions"][0]["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<PortfolioMaterializationEnvelope>(value).is_err());
}

#[test]
fn nested_materialization_wire_objects_reject_unknown_fields() {
    let mut bundle = envelope(1, 1);
    bundle.portfolio.books.insert(
        "book".into(),
        finstack_quant_portfolio::book::Book::new("book", Some("Book".to_string())),
    );
    let mut dependencies = MarketDependencies::default();
    dependencies.add_discount_curve("USD-OIS");
    dependencies.add_volatility_dependency(VolatilityDependency::new(
        "USD-SWAPTION",
        None,
        Some(0.02),
    ));
    dependencies.add_fx_pair(Currency::USD, Currency::EUR);
    bundle.instruments[0].dependencies = Some(dependencies);
    let original = serde_json::to_value(bundle).expect("serialize strict fixture");

    for pointer in [
        "/portfolio/entities/entity",
        "/portfolio/books/book",
        "/instruments/0/dependencies",
        "/instruments/0/dependencies/curves",
        "/instruments/0/dependencies/volatility_dependencies/0",
        "/instruments/0/dependencies/fx_pairs/0",
    ] {
        let mut mutated = original.clone();
        mutated
            .pointer_mut(pointer)
            .and_then(serde_json::Value::as_object_mut)
            .unwrap_or_else(|| panic!("{pointer} must be an object"))
            .insert("__unknown__".to_string(), serde_json::json!(true));
        assert!(
            serde_json::from_value::<PortfolioMaterializationEnvelope>(mutated).is_err(),
            "unknown nested field accepted at {pointer}"
        );
    }
}

#[test]
fn limits_are_enforced_before_artifact_decode() {
    let bundle = envelope(2, 2);
    let bytes = serde_json::to_vec(&bundle).expect("serialize fixture");
    let cache = InstrumentArtifactCache::new();

    let error = Portfolio::from_materialization(
        &bytes,
        &cache,
        &LoadLimits::default().with_max_bytes(bytes.len() - 1),
    )
    .expect_err("byte limit");
    assert!(matches!(
        error,
        finstack_quant_portfolio::Error::ContractLimitExceeded {
            ref what,
            found,
            limit,
        } if what == "bytes" && found == bytes.len() && limit == bytes.len() - 1
    ));
    assert_eq!(cache.len(), 0);

    let error = load(
        &bundle,
        &cache,
        &LoadLimits::default().with_max_artifacts(1),
    )
    .expect_err("artifact limit");
    assert!(matches!(
        error,
        finstack_quant_portfolio::Error::ContractLimitExceeded {
            ref what,
            found: 2,
            limit: 1,
        } if what == "artifacts"
    ));
    assert_eq!(cache.len(), 0);

    let error = load(
        &bundle,
        &cache,
        &LoadLimits::default().with_max_positions(1),
    )
    .expect_err("position limit");
    assert!(matches!(
        error,
        finstack_quant_portfolio::Error::ContractLimitExceeded {
            ref what,
            found: 2,
            limit: 1,
        } if what == "positions"
    ));
    assert_eq!(cache.len(), 0);
}

#[test]
fn collection_limits_precede_typed_element_validation() {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "schema": "finstack_quant.portfolio_materialization/1",
        "portfolio": {},
        "instruments": [{}, {}],
        "positions": []
    }))
    .expect("serialize malformed over-limit fixture");
    let cache = InstrumentArtifactCache::new();

    let error = Portfolio::from_materialization(
        &bytes,
        &cache,
        &LoadLimits::default().with_max_artifacts(1),
    )
    .expect_err("artifact limit must precede element validation");

    assert!(matches!(
        error,
        finstack_quant_portfolio::Error::ContractLimitExceeded {
            ref what,
            found: 2,
            limit: 1,
        } if what == "artifacts"
    ));
    assert_eq!(cache.len(), 0);
}

#[test]
fn materialization_depth_limit_covers_outer_and_raw_instrument_json() {
    let cache = InstrumentArtifactCache::new();
    let mut outer = serde_json::to_value(envelope(1, 1)).expect("serialize outer fixture");
    outer["portfolio"]["meta"]["deep"] = nested_json(12);
    let outer_bytes = serde_json::to_vec(&outer).expect("serialize nested outer fixture");
    let outer_error = Portfolio::from_materialization(
        &outer_bytes,
        &cache,
        &LoadLimits::default().with_max_depth(10),
    )
    .expect_err("outer metadata nesting must respect max_depth");
    assert!(matches!(
        outer_error,
        Error::ContractLimitExceeded {
            ref what,
            found,
            limit: 10,
        } if what == "JSON depth" && found > 10
    ));
    assert_eq!(cache.decode_count(), 0);

    let mut embedded = serde_json::to_value(envelope(1, 1)).expect("serialize embedded fixture");
    embedded["instruments"][0]["envelope"]["instrument"]["spec"]["__deep__"] = nested_json(20);
    let embedded_bytes = serde_json::to_vec(&embedded).expect("serialize nested embedded fixture");
    let embedded_error = Portfolio::from_materialization(
        &embedded_bytes,
        &cache,
        &LoadLimits::default().with_max_depth(16),
    )
    .expect_err("RawValue instrument nesting must respect max_depth before typed decode");
    assert!(matches!(
        embedded_error,
        Error::ContractLimitExceeded {
            ref what,
            found,
            limit: 16,
        } if what == "JSON depth" && found > 16
    ));
    assert_eq!(cache.decode_count(), 0);

    let mut within_default =
        serde_json::to_value(envelope(1, 1)).expect("serialize default-limit fixture");
    within_default["positions"][0]["meta"]["deep"] = nested_json(20);
    let within_default_bytes =
        serde_json::to_vec(&within_default).expect("serialize default-limit nested fixture");
    let (portfolio, _) =
        Portfolio::from_materialization(&within_default_bytes, &cache, &LoadLimits::default())
            .expect("default depth limit must accept ordinary nested metadata");
    assert_eq!(portfolio.positions().len(), 1);

    let mut quoted_delimiters =
        serde_json::to_value(envelope(1, 1)).expect("serialize quoted-delimiter fixture");
    quoted_delimiters["portfolio"]["meta"]["quoted"] =
        serde_json::json!(r#"objects {"nested":[1]} and escapes \"[{\\\"}]\" stay text"#);
    let quoted_bytes =
        serde_json::to_vec(&quoted_delimiters).expect("serialize quoted-delimiter fixture");
    Portfolio::from_materialization(
        &quoted_bytes,
        &cache,
        &LoadLimits::default().with_max_depth(8),
    )
    .expect("braces and brackets inside strings must not count toward JSON depth");
}

#[test]
fn schema_and_strict_instrument_envelopes_are_required() {
    let cache = InstrumentArtifactCache::new();
    let mut value = serde_json::to_value(envelope(1, 1)).expect("serialize fixture");
    value["schema"] = serde_json::json!("finstack_quant.portfolio_materialization/2");
    let bytes = serde_json::to_vec(&value).expect("serialize wrong schema");
    let report = report_from_error(
        Portfolio::from_materialization(&bytes, &cache, &LoadLimits::default())
            .expect_err("schema rejected"),
    );
    assert_eq!(report.diagnostics[0].code, "contract/structure-invalid");

    let mut value = serde_json::to_value(envelope(1, 1)).expect("serialize fixture");
    value["instruments"][0]["envelope"] = value["instruments"][0]["envelope"]["instrument"].take();
    value["instruments"][0]["content_hash"] = serde_json::Value::Null;
    let bytes = serde_json::to_vec(&value).expect("serialize bare instrument");
    let report = report_from_error(
        Portfolio::from_materialization(&bytes, &cache, &LoadLimits::default())
            .expect_err("bare instrument rejected"),
    );
    assert_eq!(report.diagnostics[0].code, "contract/structure-invalid");
}

#[test]
fn duplicate_and_missing_references_aggregate_with_bounded_diagnostics() {
    let cache = InstrumentArtifactCache::new();
    let mut bundle = envelope(2, 2);
    bundle.instruments[1].artifact_id = bundle.instruments[0].artifact_id.clone();
    bundle.positions[1].id = bundle.positions[0].id.clone();
    bundle.positions[0].artifact_id = "missing-0".to_string();
    bundle.positions[1].artifact_id = "missing-1".to_string();

    let report = report_from_error(
        load(
            &bundle,
            &cache,
            &LoadLimits::default().with_max_diagnostics(3),
        )
        .expect_err("invalid references"),
    );
    assert_eq!(report.diagnostics.len(), 3);
    assert!(report.truncated);
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "portfolio/duplicate-position-id"
            && diagnostic.message.contains("indices 0 and 1")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "portfolio/duplicate-artifact-id"
            && diagnostic.message.contains("indices 0 and 1")
    }));
    assert!(report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "portfolio/missing-artifact"
            && diagnostic.message.contains("position-")
            && diagnostic.message.contains("missing-")
    }));
    assert_eq!(cache.len(), 0);
}

#[test]
fn distinct_artifact_ids_with_identical_content_share_one_decode_and_warn() {
    let cache = InstrumentArtifactCache::new();
    let mut bundle = envelope(1, 1);
    let mut second_artifact = bundle.instruments[0].clone();
    second_artifact.artifact_id = "artifact-revision".to_string();
    bundle.instruments.push(second_artifact);
    let mut second_position = position(1, 0);
    second_position.artifact_id = "artifact-revision".to_string();
    bundle.positions.push(second_position);

    let (portfolio, report) =
        load(&bundle, &cache, &LoadLimits::default()).expect("duplicate content is recoverable");

    assert_eq!(portfolio.positions().len(), 2);
    assert!(Arc::ptr_eq(
        &portfolio.positions()[0].instrument,
        &portfolio.positions()[1].instrument
    ));
    assert_eq!(cache.decode_count(), 1);
    assert_eq!(cache.len(), 1);
    assert_eq!(report.cache_hits, 1);
    let warning = report
        .report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "portfolio/duplicate-artifact-content")
        .expect("duplicate content warning");
    assert_eq!(
        warning.severity,
        finstack_quant_core::contract::Severity::Warning
    );
    assert_eq!(warning.pointer.as_deref(), Some("/instruments/1/envelope"));
    assert_eq!(warning.revision_id.as_deref(), Some("artifact-revision"));
    assert_eq!(
        warning.artifact_hash.as_deref(),
        bundle.instruments[0].content_hash.as_deref()
    );
}

#[test]
fn duplicate_artifact_id_remains_a_fatal_error() {
    let cache = InstrumentArtifactCache::new();
    let mut bundle = envelope(1, 1);
    bundle.instruments.push(bundle.instruments[0].clone());

    let report = report_from_error(
        load(&bundle, &cache, &LoadLimits::default())
            .expect_err("duplicate artifact IDs must remain fatal"),
    );
    let duplicate = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "portfolio/duplicate-artifact-id")
        .expect("duplicate ID diagnostic");
    assert_eq!(
        duplicate.severity,
        finstack_quant_core::contract::Severity::Error
    );
    assert_eq!(
        duplicate.pointer.as_deref(),
        Some("/instruments/1/artifact_id")
    );
    assert_eq!(cache.decode_count(), 0);
}

#[test]
fn mixed_semantic_failures_aggregate_across_categories() {
    let mut bundle = envelope(3, 2);
    bundle.instruments[1].artifact_id = bundle.instruments[0].artifact_id.clone();
    bundle.positions[1].id = bundle.positions[0].id.clone();
    bundle.positions[0].artifact_id = "missing-0".to_string();
    bundle.positions[1].artifact_id = "missing-1".to_string();
    bundle.instruments[0].content_hash = Some(format!("sha256:{}", "0".repeat(64)));
    bundle.instruments[1].content_hash = None;
    bundle.instruments[2].content_hash = None;

    let bytes = serde_json::to_vec(&bundle).expect("serialize mixed-invalid fixture");
    let error = Portfolio::from_materialization(
        &bytes,
        &InstrumentArtifactCache::new(),
        &LoadLimits::default().with_max_diagnostics(7),
    )
    .expect_err("mixed invalid bundle");
    let report = report_from_error(error);
    assert_eq!(report.diagnostics.len(), 5);
    assert!(!report.truncated);
    for code in [
        "portfolio/duplicate-artifact-id",
        "portfolio/duplicate-position-id",
        "portfolio/missing-artifact",
        "portfolio/content-hash-mismatch",
    ] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code),
            "missing cross-category diagnostic {code}: {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn claimed_content_hash_is_verified() {
    let cache = InstrumentArtifactCache::new();
    let mut bundle = envelope(1, 1);
    bundle.instruments[0].content_hash = Some(format!("sha256:{}", "0".repeat(64)));

    let report = report_from_error(
        load(&bundle, &cache, &LoadLimits::default()).expect_err("hash mismatch"),
    );
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "portfolio/content-hash-mismatch"));
    assert_eq!(cache.len(), 0);
}

#[test]
fn producer_dependency_claim_is_cross_checked() {
    let cache = InstrumentArtifactCache::new();
    let mut bundle = envelope(1, 1);
    bundle.instruments[0].dependencies = Some(Default::default());

    let report = report_from_error(
        load(&bundle, &cache, &LoadLimits::default()).expect_err("dependency mismatch"),
    );
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "portfolio/dependencies-mismatch"));
}

#[test]
fn shared_artifacts_share_arc_and_input_order_is_preserved() {
    let cache = InstrumentArtifactCache::new();
    let mut bundle = envelope(2, 4);
    bundle.positions.swap(0, 3);
    bundle.positions.swap(1, 2);
    bundle.positions[1].artifact_id = bundle.positions[0].artifact_id.clone();
    bundle.positions[1].instrument_id = bundle.positions[0].instrument_id.clone();
    let expected_order: Vec<_> = bundle
        .positions
        .iter()
        .map(|position| position.id.clone())
        .collect();

    let (portfolio, report) = load(&bundle, &cache, &LoadLimits::default()).expect("materialize");
    let actual_order: Vec<_> = portfolio
        .positions()
        .iter()
        .map(|position| position.position_id.clone())
        .collect();
    assert_eq!(actual_order, expected_order);
    assert!(Arc::ptr_eq(
        &portfolio.positions()[0].instrument,
        &portfolio.positions()[1].instrument
    ));
    assert_eq!(report.unique_instruments, 2);
    assert_eq!(report.positions, 4);
    assert!(report.timing_available);
}

#[test]
fn five_thousand_positions_decode_fifty_unique_artifacts_once_and_warm_cache_hits() {
    let cache = InstrumentArtifactCache::with_capacity(50);
    let bundle = envelope(50, 5_000);

    let (cold, cold_report) =
        load(&bundle, &cache, &LoadLimits::default()).expect("cold materialization");
    assert_eq!(cold.positions().len(), 5_000);
    assert_eq!(cold_report.unique_instruments, 50);
    assert_eq!(cold_report.cache_hits, 0);
    assert_eq!(cache.len(), 50);
    assert_eq!(cache.decode_count(), 50);

    let (warm, warm_report) =
        load(&bundle, &cache, &LoadLimits::default()).expect("warm materialization");
    assert_eq!(warm.positions().len(), 5_000);
    assert_eq!(warm_report.unique_instruments, 50);
    assert_eq!(warm_report.cache_hits, 50);
    assert_eq!(cache.len(), 50);
    assert_eq!(cache.decode_count(), 50);
}

#[test]
fn materialization_roundtrip_matches_from_spec_visible_state() {
    let portable = Portfolio::from_spec(
        Portfolio::builder("portfolio")
            .name("Materialized Portfolio")
            .base_currency(Currency::USD)
            .as_of(date!(2025 - 01 - 01))
            .entity(Entity::new("entity"))
            .tag("desk", "rates")
            .position(
                Position::new(
                    "position-0",
                    "entity",
                    "DEP-0",
                    Arc::new(match deposit_envelope(0).instrument {
                        InstrumentJson::Deposit(deposit) => deposit,
                        _ => unreachable!("deposit fixture"),
                    }),
                    1.0,
                    PositionUnit::Units,
                )
                .expect("position"),
            )
            .build()
            .expect("portfolio")
            .to_spec(),
    )
    .expect("portable reconstruction");

    let materialization = portable
        .to_materialization()
        .expect("materialization export");
    let (fast, _) = load(
        &materialization,
        &InstrumentArtifactCache::new(),
        &LoadLimits::default(),
    )
    .expect("fast reconstruction");

    assert_eq!(
        serde_json::to_value(fast.to_spec()).expect("fast spec"),
        serde_json::to_value(portable.to_spec()).expect("portable spec")
    );
    assert!(fast.get_position("position-0").is_some());
}

#[test]
fn export_deduplicates_instruments_by_content_hash() {
    let instrument: Arc<dyn Instrument> = Arc::from(
        deposit_envelope(0)
            .instrument
            .into_boxed()
            .expect("deposit"),
    );
    let first = Position::new(
        "position-0",
        "entity",
        "DEP-0",
        Arc::clone(&instrument),
        1.0,
        PositionUnit::Units,
    )
    .expect("first position");
    let second = Position::new(
        "position-1",
        "entity",
        "DEP-0",
        instrument,
        2.0,
        PositionUnit::Units,
    )
    .expect("second position");
    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2025 - 01 - 01))
        .entity(Entity::new("entity"))
        .positions([first, second])
        .build()
        .expect("portfolio");

    let materialization = portfolio.to_materialization().expect("export");
    assert_eq!(materialization.instruments.len(), 1);
    assert_eq!(materialization.positions.len(), 2);
    assert_eq!(
        materialization.positions[0].artifact_id,
        materialization.positions[1].artifact_id
    );
}

#[derive(Clone)]
struct NonSerializableInstrument {
    id: String,
    attributes: Attributes,
}

finstack_quant_valuations::impl_empty_cashflow_provider!(
    NonSerializableInstrument,
    finstack_quant_cashflows::builder::CashflowRepresentation::NoResidual
);

impl Instrument for NonSerializableInstrument {
    /// Test mock: reads no market data.
    fn market_dependencies(
        &self,
    ) -> finstack_quant_core::Result<finstack_quant_valuations::instruments::MarketDependencies>
    {
        Ok(finstack_quant_valuations::instruments::MarketDependencies::new())
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn key(&self) -> InstrumentType {
        InstrumentType::Basket
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut Attributes {
        &mut self.attributes
    }

    fn clone_box(&self) -> Box<dyn Instrument> {
        Box::new(self.clone())
    }

    fn base_value(
        &self,
        _curves: &finstack_quant_core::market_data::context::MarketContext,
        _as_of: finstack_quant_core::dates::Date,
    ) -> finstack_quant_core::Result<Money> {
        Ok(Money::new(0.0, Currency::USD).expect("valid money fixture"))
    }
}

#[test]
fn export_fails_instead_of_emitting_null_for_non_serializable_instrument() {
    let instrument = NonSerializableInstrument {
        id: "opaque".to_string(),
        attributes: Attributes::new(),
    };
    let portfolio = Portfolio::builder("portfolio")
        .base_currency(Currency::USD)
        .as_of(date!(2025 - 01 - 01))
        .entity(Entity::new("entity"))
        .position(
            Position::new(
                "position",
                "entity",
                "opaque",
                Arc::new(instrument),
                1.0,
                PositionUnit::Units,
            )
            .expect("position"),
        )
        .build()
        .expect("portfolio");

    let error = portfolio
        .to_materialization()
        .expect_err("opaque instrument rejected");
    assert!(error.to_string().contains("opaque"));
}

#[test]
fn bounded_cache_evicts_without_invalidating_shared_arcs() {
    let cache = InstrumentArtifactCache::with_capacity(1);
    assert!(cache.is_empty());
    let (first, _) = load(&envelope(1, 1), &cache, &LoadLimits::default()).expect("first load");
    assert!(!cache.is_empty());
    let retained = Arc::clone(&first.positions()[0].instrument);

    load(&envelope(2, 2), &cache, &LoadLimits::default()).expect("evicting load");
    assert_eq!(cache.len(), 1);
    assert_eq!(retained.id(), "DEP-0");
}

#[test]
fn cache_byte_budget_evicts_least_recently_used_artifact() {
    let first = envelope(1, 1);
    let first_bytes = serde_json::to_vec(&first.instruments[0].envelope)
        .expect("serialize instrument envelope")
        .len();
    let cache = InstrumentArtifactCache::with_limits(10, first_bytes + 32);
    load(&first, &cache, &LoadLimits::default()).expect("first load");

    let mut second = envelope(1, 1);
    second.instruments[0] = artifact(1);
    second.positions[0] = position(0, 1);
    load(&second, &cache, &LoadLimits::default()).expect("second load");
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.decode_count(), 2);

    load(&first, &cache, &LoadLimits::default()).expect("first artifact reload");
    assert_eq!(
        cache.decode_count(),
        3,
        "first artifact must have been evicted"
    );
}

#[test]
fn concurrent_same_key_materialization_decodes_once() {
    let bytes = Arc::new(serde_json::to_vec(&envelope(1, 1)).expect("serialize fixture"));
    let cache = Arc::new(InstrumentArtifactCache::new());
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|_| {
            let bytes = Arc::clone(&bytes);
            let cache = Arc::clone(&cache);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                Portfolio::from_materialization(&bytes, &cache, &LoadLimits::default())
                    .expect("concurrent materialization");
            })
        })
        .collect();
    for worker in workers {
        worker.join().expect("worker must not panic");
    }
    assert_eq!(cache.decode_count(), 1);
    assert_eq!(cache.len(), 1);
}

#[test]
fn typed_artifact_rejects_unknown_instrument_fields_before_decode() {
    let bundle = envelope(2, 2);
    let mut value = serde_json::to_value(bundle).expect("serialize fixture");
    for artifact in value["instruments"]
        .as_array_mut()
        .expect("instrument array")
    {
        artifact["content_hash"] = serde_json::Value::Null;
        artifact["envelope"]["instrument"]["spec"]["__unknown__"] = serde_json::json!(true);
    }
    let bytes = serde_json::to_vec(&value).expect("serialize invalid fixtures");
    let cache = InstrumentArtifactCache::new();
    let report = report_from_error(
        Portfolio::from_materialization(&bytes, &cache, &LoadLimits::default())
            .expect_err("invalid instrument specs"),
    );
    assert_eq!(report.diagnostics[0].code, "contract/structure-invalid");
    assert_eq!(cache.decode_count(), 0);
}

#[test]
fn typed_artifact_field_roundtrips_without_materializing_as_null() {
    let bundle = envelope(1, 1);
    let bytes = serde_json::to_vec(&bundle).expect("serialize");
    let decoded: PortfolioMaterializationEnvelope =
        serde_json::from_slice(&bytes).expect("deserialize");
    assert_eq!(
        serde_json::to_value(&decoded.instruments[0].envelope).expect("serialize envelope")
            ["schema"],
        "finstack_quant.instrument/1"
    );
}
