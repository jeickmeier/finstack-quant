//! Versioned, content-addressed portfolio materialization fast path.
//!
//! [`crate::portfolio::PortfolioSpec`] remains the portable embedded
//! interchange format. This module is the native bulk-loading path for
//! normalized stores: an allocation-free preflight enforces collection limits,
//! then the outer document is parsed once into its canonical serde type. Unique
//! instrument envelopes are validated before cache lookup, and positions share
//! cached [`std::sync::Arc`] instrument instances.

mod cache;
mod envelope;
mod report;

use std::fmt;
use std::sync::Arc;

use crate::dependencies::DependencyIndex;
use crate::error::Error;
use crate::portfolio::Portfolio;
use crate::position::Position;
use crate::types::PositionId;
use cache::{CacheKey, CachedArtifact};
use finstack_quant_core::canonical::CANONICAL_VERSION;
use finstack_quant_core::contract::{
    check_json_limits, ContractDescriptor, ContractError, Diagnostic, LoadLimits, LoadPhase,
    Severity, ValidationReport,
};
use finstack_quant_core::{HashMap, HashSet};
use finstack_quant_valuations::instruments::{InstrumentEnvelope, MarketDependencies};
use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

pub use cache::InstrumentArtifactCache;
pub use envelope::{
    InstrumentArtifact, MaterializedPosition, MaterializerInfo, PortfolioHeader,
    PortfolioMaterializationEnvelope, PortfolioMaterializationSchema,
};
pub use report::{MaterializationPhases, MaterializationReport};

/// Persistence contract for [`PortfolioMaterializationEnvelope`].
pub const PORTFOLIO_MATERIALIZATION_CONTRACT: ContractDescriptor =
    ContractDescriptor::new("finstack_quant.portfolio_materialization");

type MaterializationInput = PortfolioMaterializationEnvelope;

struct PreparedArtifact {
    input_index: usize,
    artifact_id: String,
    content_hash: String,
    instrument_version: u32,
    envelope: InstrumentEnvelope,
    encoded_bytes: usize,
    claimed_dependencies: Option<MarketDependencies>,
}

struct ValidatedInput {
    bundle: MaterializationInput,
    decoded: HashMap<String, CachedArtifact>,
    diagnostics: ValidationReport,
    parse_nanos: Option<u64>,
    validation_nanos: Option<u64>,
    decode_nanos: Option<u64>,
    cache_hits: usize,
    dependency_count: usize,
}

/// Allocation-free preflight counts for resource-bounded collections.
///
/// All other fields are ignored here and validated by the canonical typed
/// deserialization immediately afterward. A malformed document is likewise
/// left to that canonical pass so its structured diagnostic remains unchanged.
#[derive(Deserialize)]
struct MaterializationCollectionCounts {
    #[serde(default)]
    instruments: SequenceCount,
    #[serde(default)]
    positions: SequenceCount,
    #[serde(default)]
    portfolio: PortfolioCollectionCounts,
}

#[derive(Default, Deserialize)]
struct PortfolioCollectionCounts {
    #[serde(default)]
    books: MapCount,
}

#[derive(Default)]
struct MapCount(usize);

impl<'de> Deserialize<'de> for MapCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct MapCountVisitor;
        impl<'de> Visitor<'de> for MapCountVisitor {
            type Value = MapCount;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut count = 0usize;
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {
                    count = count.saturating_add(1);
                }
                Ok(MapCount(count))
            }
        }
        deserializer.deserialize_map(MapCountVisitor)
    }
}

#[derive(Default)]
struct SequenceCount(usize);

impl<'de> Deserialize<'de> for SequenceCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SequenceCountVisitor;

        impl<'de> Visitor<'de> for SequenceCountVisitor {
            type Value = SequenceCount;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON array")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut count = 0usize;
                while sequence.next_element::<IgnoredAny>()?.is_some() {
                    count = count.saturating_add(1);
                }
                Ok(SequenceCount(count))
            }
        }

        deserializer.deserialize_seq(SequenceCountVisitor)
    }
}

impl Portfolio {
    /// Materialize a portfolio from one strict, versioned bulk bundle.
    ///
    /// The outer bundle is checked by allocation-free lexical depth and
    /// collection-count scans, then parsed directly into its typed form once.
    /// Embedded instrument envelopes are serde-validated before cache lookup.
    /// Count and byte limits, references, duplicate IDs, and claimed content
    /// hashes are checked before any runtime instrument is decoded. Each cache
    /// miss is decoded sequentially once, and all positions referencing it
    /// share the same [`Arc`].
    ///
    /// Use [`Self::from_spec`] for portable embedded interchange where each
    /// position carries its own optional instrument definition.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON encoding of a
    ///   [`PortfolioMaterializationEnvelope`].
    /// * `cache` - Shared bounded cache for content-addressed runtime
    ///   instruments.
    /// * `limits` - Resource policy bounding bytes, artifacts, positions, and
    ///   retained diagnostics.
    ///
    /// # Returns
    ///
    /// A validated portfolio with rebuilt indices and a phase-instrumented
    /// materialization report.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ContractLimitExceeded`] when a configured resource
    /// limit is exceeded, [`Error::MaterializationFailed`] for malformed JSON,
    /// contract, hash, reference, dependency, decode, or final portfolio
    /// validation findings, and [`Error::Core`] when canonical hashing fails.
    pub fn from_materialization(
        bytes: &[u8],
        cache: &InstrumentArtifactCache,
        limits: &LoadLimits,
    ) -> crate::Result<(Portfolio, MaterializationReport)> {
        let ValidatedInput {
            bundle,
            decoded,
            mut diagnostics,
            parse_nanos,
            validation_nanos,
            decode_nanos,
            cache_hits,
            dependency_count,
        } = validate_and_decode(bytes, cache, limits)?;

        let build_started = report::start_timer();
        let book_by_position = position_book_index(&bundle);
        let mut positions = Vec::with_capacity(bundle.positions.len());
        for (index, materialized) in bundle.positions.iter().enumerate() {
            let Some(artifact) = decoded.get(&materialized.artifact_id) else {
                // Reference validation guarantees this cannot happen unless a
                // prior decode failed, which has already returned above.
                diagnostics.push_bounded(limits, missing_artifact_diagnostic(index, materialized));
                continue;
            };
            validate_position_semantics(index, materialized, artifact, limits, &mut diagnostics);

            match Position::new(
                materialized.id.clone(),
                materialized.entity_id.clone(),
                materialized.instrument_id.clone(),
                Arc::clone(&artifact.instrument),
                materialized.quantity,
                materialized.unit,
            ) {
                Ok(mut position) => {
                    position.attributes.clone_from(&materialized.attributes);
                    position.meta.clone_from(&materialized.meta);
                    position.book_id = book_by_position.get(&materialized.id).cloned();
                    positions.push(position);
                }
                Err(error) => diagnostics.push_bounded(
                    limits,
                    Diagnostic::new(
                        "portfolio/position-invalid",
                        LoadPhase::Build,
                        Severity::Error,
                        error.to_string(),
                    )
                    .with_pointer(format!("/positions/{index}"))
                    .with_position_id(materialized.id.to_string()),
                ),
            }
        }
        let build_nanos = report::elapsed_nanos(build_started);

        if diagnostics.has_errors() {
            return Err(Error::MaterializationFailed(Box::new(diagnostics)));
        }

        let index_started = report::start_timer();
        let mut portfolio = Portfolio {
            id: bundle.portfolio.id,
            name: bundle.portfolio.name,
            base_currency: bundle.portfolio.base_currency,
            as_of: bundle.portfolio.as_of,
            entities: bundle.portfolio.entities,
            positions,
            position_index: HashMap::default(),
            dependency_index: DependencyIndex::default(),
            books: bundle.portfolio.books,
            tags: bundle.portfolio.tags,
            meta: bundle.portfolio.meta,
            evaluation_state_id: 0,
        };
        portfolio.rebuild_index();
        if let Err(error) = portfolio.validate() {
            diagnostics.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/invariants-invalid",
                    LoadPhase::Build,
                    Severity::Error,
                    error.to_string(),
                ),
            );
            return Err(Error::MaterializationFailed(Box::new(diagnostics)));
        }
        let index_nanos = report::elapsed_nanos(index_started);

        let timing_available = parse_nanos.is_some()
            && validation_nanos.is_some()
            && decode_nanos.is_some()
            && build_nanos.is_some()
            && index_nanos.is_some();
        let report = MaterializationReport {
            report: diagnostics,
            unique_instruments: bundle.instruments.len(),
            positions: bundle.positions.len(),
            dependencies: dependency_count,
            cache_hits,
            input_bytes: bytes.len(),
            timing_available,
            phase_nanos: MaterializationPhases {
                parse: parse_nanos.unwrap_or_default(),
                validate_versions: validation_nanos.unwrap_or_default(),
                decode_instruments: decode_nanos.unwrap_or_default(),
                build_positions: build_nanos.unwrap_or_default(),
                index_build: index_nanos.unwrap_or_default(),
            },
        };
        Ok((portfolio, report))
    }

    /// Strictly validate a materialization bundle without building a portfolio.
    ///
    /// This path parses the outer bundle, enforces resource and contract
    /// versions, verifies content hashes and references, resolves every
    /// artifact ID while decoding identical content only once, checks claimed
    /// dependencies and instrument semantics, and validates position values.
    /// It deliberately does not construct
    /// [`Position`] values, rebuild portfolio indices, or call
    /// [`Portfolio::validate`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON materialization bundle.
    /// * `cache` - Shared bounded artifact cache used while decoding.
    /// * `limits` - Resource limits for bytes, counts, and diagnostics.
    ///
    /// # Returns
    ///
    /// Validation counters and timings. `build_positions` and `index_build`
    /// are always zero because those phases are outside this API boundary.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ContractLimitExceeded`] for resource-limit failures,
    /// [`Error::MaterializationFailed`] for malformed or semantically invalid
    /// data, and [`Error::Core`] when canonical hashing fails.
    pub fn validate_materialization(
        bytes: &[u8],
        cache: &InstrumentArtifactCache,
        limits: &LoadLimits,
    ) -> crate::Result<MaterializationReport> {
        let ValidatedInput {
            bundle,
            decoded,
            mut diagnostics,
            parse_nanos,
            validation_nanos,
            decode_nanos,
            cache_hits,
            dependency_count,
        } = validate_and_decode(bytes, cache, limits)?;
        let semantic_started = report::start_timer();
        let _book_by_position = position_book_index(&bundle);
        for (index, position) in bundle.positions.iter().enumerate() {
            let Some(artifact) = decoded.get(&position.artifact_id) else {
                diagnostics.push_bounded(limits, missing_artifact_diagnostic(index, position));
                continue;
            };
            validate_position_semantics(index, position, artifact, limits, &mut diagnostics);
        }
        let semantic_nanos = report::elapsed_nanos(semantic_started);
        if diagnostics.has_errors() {
            return Err(Error::MaterializationFailed(Box::new(diagnostics)));
        }

        Ok(MaterializationReport {
            report: diagnostics,
            unique_instruments: bundle.instruments.len(),
            positions: bundle.positions.len(),
            dependencies: dependency_count,
            cache_hits,
            input_bytes: bytes.len(),
            timing_available: parse_nanos.is_some()
                && validation_nanos.is_some()
                && decode_nanos.is_some()
                && semantic_nanos.is_some(),
            phase_nanos: MaterializationPhases {
                parse: parse_nanos.unwrap_or_default(),
                validate_versions: validation_nanos
                    .unwrap_or_default()
                    .saturating_add(semantic_nanos.unwrap_or_default()),
                decode_instruments: decode_nanos.unwrap_or_default(),
                build_positions: 0,
                index_build: 0,
            },
        })
    }

    /// Export a runtime portfolio as a deduplicated materialization bundle.
    ///
    /// Instrument artifacts are emitted in first-position order and
    /// deduplicated by canonical envelope content hash. Instruments that do
    /// not implement
    /// [`finstack_quant_valuations::instruments::Instrument::to_instrument_json`]
    /// cause an error instead of producing a lossy `null` payload.
    ///
    /// Use [`Self::to_spec`] for the portable per-position interchange format.
    ///
    /// # Returns
    ///
    /// A strict current-version materialization envelope.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] when the runtime portfolio is invalid, an instrument
    /// cannot be serialized, canonical hashing or dependency extraction fails,
    /// or a strict raw instrument envelope cannot be encoded.
    pub fn to_materialization(&self) -> crate::Result<PortfolioMaterializationEnvelope> {
        self.validate()?;

        let mut instruments = Vec::new();
        let mut artifact_id_by_hash: HashMap<String, String> = HashMap::default();
        let mut positions = Vec::with_capacity(self.positions.len());
        let books = self.books.clone();

        for position in &self.positions {
            let instrument_json = position.instrument.to_instrument_json().ok_or_else(|| {
                Error::invalid_input(format!(
                    "instrument '{}' on position '{}' does not support materialization serialization",
                    position.instrument_id, position.position_id
                ))
            })?;
            let envelope = InstrumentEnvelope::new(instrument_json);
            let content_hash = envelope.content_hash().map_err(Error::Core)?;
            let artifact_id = if let Some(existing) = artifact_id_by_hash.get(&content_hash) {
                existing.clone()
            } else {
                let artifact_id = content_hash.clone();
                let dependencies = position
                    .instrument
                    .market_dependencies()
                    .map_err(Error::Core)?;
                instruments.push(InstrumentArtifact {
                    artifact_id: artifact_id.clone(),
                    content_hash: Some(content_hash.clone()),
                    envelope,
                    dependencies: Some(dependencies),
                });
                artifact_id_by_hash.insert(content_hash, artifact_id.clone());
                artifact_id
            };

            positions.push(MaterializedPosition {
                id: position.position_id.clone(),
                entity_id: position.entity_id.clone(),
                instrument_id: position.instrument_id.clone(),
                artifact_id,
                quantity: position.quantity,
                unit: position.unit,
                attributes: position.attributes.clone(),
                meta: position.meta.clone(),
            });
        }

        Ok(PortfolioMaterializationEnvelope {
            schema: PortfolioMaterializationSchema::Materialization,
            portfolio: PortfolioHeader {
                id: self.id.clone(),
                name: self.name.clone(),
                base_currency: self.base_currency,
                as_of: self.as_of,
                entities: self.entities.clone(),
                books,
                tags: self.tags.clone(),
                meta: self.meta.clone(),
            },
            instruments,
            positions,
            materializer: None,
        })
    }
}

fn validate_and_decode(
    bytes: &[u8],
    cache: &InstrumentArtifactCache,
    limits: &LoadLimits,
) -> crate::Result<ValidatedInput> {
    let parse_started = report::start_timer();
    check_json_limits(bytes, limits).map_err(materialization_contract_error)?;
    enforce_materialization_collection_limits(bytes, limits)?;
    let bundle: MaterializationInput = serde_json::from_slice(bytes).map_err(|error| {
        let mut report = ValidationReport::default();
        let diagnostic = match error.classify() {
            serde_json::error::Category::Data => Diagnostic::new(
                "contract/structure-invalid",
                LoadPhase::Structure,
                Severity::Error,
                error.to_string(),
            ),
            serde_json::error::Category::Io
            | serde_json::error::Category::Syntax
            | serde_json::error::Category::Eof => Diagnostic::from_parse_error(&error, None),
        };
        report.push_bounded(limits, diagnostic);
        Error::MaterializationFailed(Box::new(report))
    })?;
    let parse_nanos = report::elapsed_nanos(parse_started);

    let validation_started = report::start_timer();
    let mut diagnostics = ValidationReport::default();
    validate_references(&bundle, limits, &mut diagnostics);
    let prepared = prepare_artifacts(&bundle, limits, &mut diagnostics)?;
    if diagnostics.has_errors() {
        return Err(Error::MaterializationFailed(Box::new(diagnostics)));
    }
    let validation_nanos = report::elapsed_nanos(validation_started);

    let decode_started = report::start_timer();
    let mut decoded: HashMap<String, CachedArtifact> = HashMap::default();
    let mut cache_hits = 0usize;
    let mut dependency_count = 0usize;
    for artifact in prepared {
        let PreparedArtifact {
            input_index,
            artifact_id,
            content_hash,
            instrument_version,
            envelope,
            encoded_bytes,
            claimed_dependencies,
        } = artifact;
        let key = CacheKey::new(content_hash.clone(), instrument_version, CANONICAL_VERSION);
        let decode_result = cache.get_or_decode(key, encoded_bytes, || {
            let instrument: Arc<dyn finstack_quant_valuations::instruments::Instrument> =
                Arc::from(envelope.instrument.into_boxed().map_err(Error::Core)?);
            let dependencies = instrument.market_dependencies().map_err(Error::Core)?;
            Ok(CachedArtifact {
                instrument,
                dependencies,
            })
        });

        let (cached, hit) = match decode_result {
            Ok(result) => result,
            Err(error) => {
                diagnostics.push_bounded(
                    limits,
                    Diagnostic::new(
                        "portfolio/instrument-invalid",
                        LoadPhase::Build,
                        Severity::Error,
                        error.to_string(),
                    )
                    .with_pointer(format!("/instruments/{input_index}/envelope"))
                    .with_revision_id(artifact_id),
                );
                continue;
            }
        };
        if hit {
            cache_hits += 1;
        }
        dependency_count = dependency_count
            .saturating_add(crate::dependencies::flatten_dependencies(&cached.dependencies).len());
        if let Some(claimed) = claimed_dependencies {
            if claimed != cached.dependencies {
                diagnostics.push_bounded(
                    limits,
                    Diagnostic::new(
                        "portfolio/dependencies-mismatch",
                        LoadPhase::Semantic,
                        Severity::Error,
                        format!(
                            "artifact '{artifact_id}' dependency claim does not match runtime extraction"
                        ),
                    )
                    .with_revision_id(artifact_id.clone())
                    .with_artifact_hash(content_hash),
                );
            }
        }
        decoded.insert(artifact_id, cached);
    }
    let decode_nanos = report::elapsed_nanos(decode_started);
    if diagnostics.has_errors() {
        return Err(Error::MaterializationFailed(Box::new(diagnostics)));
    }

    Ok(ValidatedInput {
        bundle,
        decoded,
        diagnostics,
        parse_nanos,
        validation_nanos,
        decode_nanos,
        cache_hits,
        dependency_count,
    })
}

fn enforce_materialization_collection_limits(
    bytes: &[u8],
    limits: &LoadLimits,
) -> crate::Result<()> {
    let Ok(counts) = serde_json::from_slice::<MaterializationCollectionCounts>(bytes) else {
        return Ok(());
    };
    enforce_count_limit("artifacts", counts.instruments.0, limits.max_artifacts)?;
    enforce_count_limit("positions", counts.positions.0, limits.max_positions)?;
    enforce_count_limit("books", counts.portfolio.books.0, crate::book::MAX_BOOKS)
}

fn validate_position_semantics(
    index: usize,
    position: &MaterializedPosition,
    artifact: &CachedArtifact,
    limits: &LoadLimits,
    diagnostics: &mut ValidationReport,
) {
    if artifact.instrument.id() != position.instrument_id {
        diagnostics.push_bounded(
            limits,
            Diagnostic::new(
                "portfolio/instrument-id-mismatch",
                LoadPhase::Semantic,
                Severity::Error,
                format!(
                    "position '{}' names instrument '{}' but artifact '{}' contains '{}'",
                    position.id,
                    position.instrument_id,
                    position.artifact_id,
                    artifact.instrument.id()
                ),
            )
            .with_pointer(format!("/positions/{index}/instrument_id"))
            .with_position_id(position.id.to_string())
            .with_instrument_id(position.instrument_id.clone())
            .with_revision_id(position.artifact_id.clone()),
        );
    }
    let invalid_quantity = !position.quantity.is_finite()
        || (matches!(position.unit, crate::position::PositionUnit::Percentage)
            && position.quantity.abs() > 100.0);
    if invalid_quantity {
        diagnostics.push_bounded(
            limits,
            Diagnostic::new(
                "portfolio/position-invalid",
                LoadPhase::Semantic,
                Severity::Error,
                format!(
                    "invalid quantity {} for position '{}'",
                    position.quantity, position.id
                ),
            )
            .with_pointer(format!("/positions/{index}/quantity"))
            .with_position_id(position.id.to_string()),
        );
    }
}

fn enforce_count_limit(what: &'static str, found: usize, limit: usize) -> crate::Result<()> {
    if found > limit {
        return Err(Error::contract_limit_exceeded(what, found, limit));
    }
    Ok(())
}

fn materialization_contract_error(error: ContractError) -> Error {
    match error {
        ContractError::LimitExceeded { what, found, limit } => {
            Error::contract_limit_exceeded(what, found, limit)
        }
        ContractError::Report(report) => Error::MaterializationFailed(report),
        ContractError::Core(error) => Error::Core(error),
        other => Error::invalid_input(other.to_string()),
    }
}

fn validate_references(
    bundle: &MaterializationInput,
    limits: &LoadLimits,
    report: &mut ValidationReport,
) {
    let mut artifacts: HashMap<&str, usize> = HashMap::default();
    for (index, artifact) in bundle.instruments.iter().enumerate() {
        if let Some(first) = artifacts.insert(&artifact.artifact_id, index) {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/duplicate-artifact-id",
                    LoadPhase::Semantic,
                    Severity::Error,
                    format!(
                        "duplicate artifact_id '{}' at indices {first} and {index}",
                        artifact.artifact_id
                    ),
                )
                .with_pointer(format!("/instruments/{index}/artifact_id"))
                .with_revision_id(artifact.artifact_id.clone()),
            );
        }
    }

    let mut positions: HashMap<&PositionId, usize> = HashMap::default();
    for (index, position) in bundle.positions.iter().enumerate() {
        if let Some(first) = positions.insert(&position.id, index) {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/duplicate-position-id",
                    LoadPhase::Semantic,
                    Severity::Error,
                    format!(
                        "duplicate position id '{}' at indices {first} and {index}",
                        position.id
                    ),
                )
                .with_pointer(format!("/positions/{index}/id"))
                .with_position_id(position.id.to_string()),
            );
        }
        if !artifacts.contains_key(position.artifact_id.as_str()) {
            report.push_bounded(limits, missing_artifact_diagnostic(index, position));
        }
    }
    validate_portfolio_envelope_invariants(bundle, &positions, limits, report);
}

fn validate_portfolio_envelope_invariants(
    bundle: &MaterializationInput,
    positions: &HashMap<&PositionId, usize>,
    limits: &LoadLimits,
    report: &mut ValidationReport,
) {
    for (entity_id, entity) in &bundle.portfolio.entities {
        if entity_id != &entity.id {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/entity-id-mismatch",
                    LoadPhase::Semantic,
                    Severity::Error,
                    format!(
                        "entity map key '{entity_id}' does not match embedded id '{}'",
                        entity.id
                    ),
                )
                .with_pointer(format!("/portfolio/entities/{entity_id}/id")),
            );
        }
    }
    for (index, position) in bundle.positions.iter().enumerate() {
        if !bundle.portfolio.entities.contains_key(&position.entity_id) {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/unknown-entity",
                    LoadPhase::Semantic,
                    Severity::Error,
                    format!(
                        "position '{}' references unknown entity '{}'",
                        position.id, position.entity_id
                    ),
                )
                .with_pointer(format!("/positions/{index}/entity_id"))
                .with_position_id(position.id.to_string()),
            );
        }
    }

    let position_ids: HashSet<_> = positions.keys().copied().collect();
    crate::book::validate_books(&bundle.portfolio.books, &position_ids, |diagnostic| {
        report.push_bounded(limits, diagnostic);
    });
}

fn missing_artifact_diagnostic(index: usize, position: &MaterializedPosition) -> Diagnostic {
    Diagnostic::new(
        "portfolio/missing-artifact",
        LoadPhase::Semantic,
        Severity::Error,
        format!(
            "position '{}' references missing artifact '{}'",
            position.id, position.artifact_id
        ),
    )
    .with_pointer(format!("/positions/{index}/artifact_id"))
    .with_position_id(position.id.to_string())
    .with_revision_id(position.artifact_id.clone())
}

fn prepare_artifacts(
    bundle: &MaterializationInput,
    limits: &LoadLimits,
    report: &mut ValidationReport,
) -> crate::Result<Vec<PreparedArtifact>> {
    let mut prepared = Vec::with_capacity(bundle.instruments.len());
    let mut hashes: HashMap<String, (usize, String)> = HashMap::default();

    for (index, artifact) in bundle.instruments.iter().enumerate() {
        let content_hash = artifact.envelope.content_hash().map_err(Error::Core)?;
        let hash_valid = artifact.content_hash.as_ref().is_none_or(|claimed| {
            if claimed == &content_hash {
                return true;
            }
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/content-hash-mismatch",
                    LoadPhase::Hash,
                    Severity::Error,
                    format!(
                        "artifact '{}' claims hash '{}' but canonical envelope hash is '{}'",
                        artifact.artifact_id, claimed, content_hash
                    ),
                )
                .with_pointer(format!("/instruments/{index}/content_hash"))
                .with_revision_id(artifact.artifact_id.clone())
                .with_artifact_hash(content_hash.clone()),
            );
            false
        });

        if let Some((first_index, first_id)) =
            hashes.insert(content_hash.clone(), (index, artifact.artifact_id.clone()))
        {
            report.push_bounded(
                limits,
                Diagnostic::new(
                    "portfolio/duplicate-artifact-content",
                    LoadPhase::Semantic,
                    Severity::Warning,
                    format!(
                        "artifacts '{first_id}' and '{}' at indices {first_index} and {index} have identical content",
                        artifact.artifact_id
                    ),
                )
                .with_pointer(format!("/instruments/{index}/envelope"))
                .with_revision_id(artifact.artifact_id.clone())
                .with_artifact_hash(content_hash.clone()),
            );
        }

        if hash_valid {
            let encoded_bytes = serde_json::to_vec(&artifact.envelope)
                .map_err(|error| Error::invalid_input(error.to_string()))?
                .len();
            prepared.push(PreparedArtifact {
                input_index: index,
                artifact_id: artifact.artifact_id.clone(),
                content_hash,
                instrument_version: ContractDescriptor::VERSION,
                envelope: artifact.envelope.clone(),
                encoded_bytes,
                claimed_dependencies: artifact.dependencies.clone(),
            });
        }
    }

    Ok(prepared)
}

fn position_book_index(
    bundle: &PortfolioMaterializationEnvelope,
) -> HashMap<PositionId, crate::book::BookId> {
    let mut by_position = HashMap::default();
    for (book_id, book) in &bundle.portfolio.books {
        for position_id in &book.position_ids {
            by_position.insert(position_id.clone(), book_id.clone());
        }
    }
    by_position
}
