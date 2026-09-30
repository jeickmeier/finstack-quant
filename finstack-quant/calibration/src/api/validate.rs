//! Static envelope validator and dependency-graph utilities.
//!
//! [`validate`] runs all structural checks (missing dependencies, undefined
//! `quote_set`s) and returns a [`CalibrationValidationReport`] listing every error found
//! plus the dependency graph of the steps. No solver is invoked — the
//! validator runs in microseconds.
//!
//! Genuine cycles cannot occur in the current `CalibrationPlan` model:
//! steps execute in declared order and read only from `market_data` /
//! `prior_market` or curves produced by *earlier* steps. A self-referential
//! step would surface as a [`EnvelopeError::MissingDependency`] rather than a
//! cycle.
//!
//! [`dry_run`] parses a JSON envelope and returns the typed report;
//! [`dry_run_json`] is its JSON-string wire twin for cross-binding
//! consumption (Python / WASM). [`parse_envelope`] and [`validate_fail_fast`]
//! are the shared strict-load and fail-fast policies every host entry point
//! uses.

// `EnvelopeError` is intentionally large (carries available-IDs lists, etc.)
// because the cross-binding consumers want all the diagnostic context in
// one shot. Boxing the error would harm ergonomics on a cold error path.
#![allow(clippy::result_large_err)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};

use crate::api::errors::EnvelopeError;
use crate::api::schema::{CalibrationEnvelope, CalibrationStep, CALIBRATION_CONTRACT};
use crate::quotes::market_quote::MarketQuote;
#[cfg(test)]
use crate::quotes::{cds::CdsQuote, rates::RateQuote};
#[cfg(test)]
use finstack_quant_core::contract::ContractError;
use finstack_quant_core::contract::{
    Diagnostic, LoadLimits, LoadPhase, Severity, ValidationReport as ContractValidationReport,
};

/// Result of [`validate`]. Always contains the dependency graph; `errors` is
/// empty when the envelope is structurally valid.
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationValidationReport {
    /// All errors found in a single pass; empty if the envelope is valid.
    pub errors: Vec<EnvelopeError>,
    /// Topological view of the steps' inputs and outputs.
    pub dependency_graph: DependencyGraph,
}

/// Static dependency graph derived from a [`CalibrationEnvelope`].
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    /// Curve / surface IDs available at the start of execution, contributed
    /// by `market_data` and `prior_market`.
    pub initial_ids: Vec<String>,
    /// Per-step inputs and outputs in declared order.
    pub nodes: Vec<DependencyNode>,
}

/// A single step's view of the dependency graph.
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts_export", ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    /// Zero-based index in `plan.steps`.
    pub step_index: usize,
    /// Step identifier.
    pub step_id: String,
    /// Step kind (`"discount"`, `"forward"`, ...).
    pub kind: String,
    /// Curve / surface IDs the step depends on. Each must be either in
    /// `initial_ids` or produced by an earlier step.
    pub reads: Vec<String>,
    /// Curve / surface ID(s) the step produces.
    pub writes: Vec<String>,
}

/// Run all static validation checks.
///
/// Always returns a [`CalibrationValidationReport`]; inspect `errors` to see what failed.
/// The validator is solver-free — runs in microseconds, suitable as a
/// pre-flight check before invoking [`engine::calibrate`](super::engine::calibrate).
///
/// # Arguments
///
/// * `envelope` - Typed calibration plan, market data, prior market objects,
///   quote sets, and solver settings to validate without executing solvers.
pub fn validate(envelope: &CalibrationEnvelope) -> CalibrationValidationReport {
    let mut errors = Vec::new();
    let initial_ids = collect_initial_ids(envelope);
    let nodes = build_nodes(&envelope.plan.steps);

    check_step_id_uniqueness(envelope, &mut errors);
    check_quote_sets(envelope, &mut errors);
    check_market_data_uniqueness(envelope, &mut errors);
    check_quote_sets_resolve(envelope, &mut errors);
    check_quote_data(envelope, &mut errors);
    check_dependencies(&initial_ids, &nodes, &mut errors);

    let mut sorted_initial: Vec<String> = initial_ids.into_iter().collect();
    sorted_initial.sort();

    CalibrationValidationReport {
        errors,
        dependency_graph: DependencyGraph {
            initial_ids: sorted_initial,
            nodes,
        },
    }
}

pub(crate) fn append_contract_diagnostics(
    report: &mut ContractValidationReport,
    errors: impl IntoIterator<Item = EnvelopeError>,
    limits: &LoadLimits,
) {
    for error in errors {
        report.push_bounded(limits, contract_diagnostic(&error));
    }
}

fn contract_diagnostic(error: &EnvelopeError) -> Diagnostic {
    let pointer = match error {
        EnvelopeError::MissingDependency { step_index, .. } => {
            format!("/plan/steps/{step_index}")
        }
        EnvelopeError::DuplicateStepId {
            duplicate_index, ..
        } => format!("/plan/steps/{duplicate_index}/id"),
        EnvelopeError::UndefinedQuoteSet { step_index, .. } => {
            format!("/plan/steps/{step_index}/quote_set")
        }
        EnvelopeError::QuoteDataInvalid { .. } | EnvelopeError::DuplicateMarketDatumId { .. } => {
            "/market_data".to_string()
        }
        EnvelopeError::QuoteIdNotInMarketData { quote_set, .. }
        | EnvelopeError::QuoteSetConflict { quote_set } => {
            format!("/plan/quote_sets/{}", escape_json_pointer(quote_set))
        }
        EnvelopeError::ConflictingMarketDatum { .. } => "/market_data".to_string(),
        EnvelopeError::JsonSerialize { .. }
        | EnvelopeError::StrictLoad { .. }
        | EnvelopeError::SolverNotConverged { .. } => "/".to_string(),
    };
    Diagnostic::new(
        format!("calibration/{}", error.kind_str().replace('_', "-")),
        LoadPhase::Semantic,
        Severity::Error,
        error.to_string(),
    )
    .with_pointer(pointer)
    .with_contract(CALIBRATION_CONTRACT.id)
}

fn escape_json_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

/// Return the first static validation error of `envelope`, if any.
///
/// This is the fail-fast policy shared by [`validate_calibration_json`],
/// [`engine::calibrate`](super::engine::calibrate) and host bindings;
/// [`validate`] reports every error instead.
///
/// # Arguments
///
/// * `envelope` - Typed calibration envelope to check with the same static,
///   solver-free rules as [`validate`].
///
/// # Errors
///
/// Returns the first [`EnvelopeError`] of [`validate`]'s report (duplicate
/// step ids, undefined or unresolved quote sets, duplicate or invalid market
/// data, missing dependencies).
pub fn validate_fail_fast(envelope: &CalibrationEnvelope) -> Result<(), EnvelopeError> {
    match validate(envelope).errors.into_iter().next() {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Parse a JSON envelope and run [`validate`] without invoking a solver.
///
/// # Arguments
///
/// * `envelope_json` - UTF-8 JSON calibration-envelope document to parse and
///   run through static, solver-free validation.
///
/// # Errors
///
/// Returns [`EnvelopeError::StrictLoad`] if strict contract ingestion
/// rejects the envelope. Static findings are returned in the report, never
/// as an error.
pub fn dry_run(envelope_json: &str) -> Result<CalibrationValidationReport, EnvelopeError> {
    Ok(validate(&parse_envelope(envelope_json)?))
}

/// JSON-string wire twin of [`dry_run`].
///
/// Returns the report serialized as pretty-printed JSON.
///
/// # Arguments
///
/// * `envelope_json` - UTF-8 JSON calibration-envelope document to parse and
///   run through static, solver-free validation.
///
/// # Errors
///
/// Returns [`EnvelopeError::StrictLoad`] if strict contract ingestion
/// rejects the envelope, or [`EnvelopeError::JsonSerialize`] if the report
/// cannot be serialized.
pub fn dry_run_json(envelope_json: &str) -> Result<String, EnvelopeError> {
    dry_run(envelope_json)?.to_json_pretty()
}

/// Validate a calibration envelope JSON string and return its canonical form.
///
/// This is the Rust-owned implementation used by host-language bindings. It
/// centralizes the parse and pretty-serialization path so Python and WASM do
/// not each reimplement the same validation logic.
///
/// Static validation is fail-fast ([`validate_fail_fast`]): the first
/// envelope error is returned. [`dry_run`] lists every static error without
/// solving.
///
/// # Arguments
///
/// * `envelope_json` - UTF-8 JSON calibration-envelope document to parse,
///   statically validate, and reserialize in canonical pretty JSON form.
///
/// # Errors
///
/// Returns [`EnvelopeError::StrictLoad`] if strict loading rejects the
/// document, the first static validation error, or
/// [`EnvelopeError::JsonSerialize`] if the envelope cannot be serialized.
pub fn validate_calibration_json(envelope_json: &str) -> Result<String, EnvelopeError> {
    let envelope = parse_envelope(envelope_json)?;
    validate_fail_fast(&envelope)?;
    serialize_pretty_json(&envelope, "CalibrationEnvelope")
}

impl CalibrationValidationReport {
    /// Serialize this report as pretty-printed JSON.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError::JsonSerialize`] if serialization fails.
    pub fn to_json_pretty(&self) -> Result<String, EnvelopeError> {
        serialize_pretty_json(self, "CalibrationValidationReport")
    }
}

pub(crate) fn serialize_pretty_json<T: Serialize>(
    value: &T,
    target: &str,
) -> Result<String, EnvelopeError> {
    serde_json::to_string_pretty(value).map_err(|err| EnvelopeError::JsonSerialize {
        target: target.to_string(),
        message: err.to_string(),
    })
}

/// Strictly parse a JSON envelope, enforcing its calibration schema marker.
///
/// Uses [`LoadLimits::default`] and the calibration contract; this is the one
/// strict-load path every host entry point shares.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON calibration-envelope document in the canonical v1
///   flat-market shape.
///
/// # Errors
///
/// Returns [`EnvelopeError::StrictLoad`] (carrying structured diagnostics)
/// for malformed JSON, a missing, malformed or unsupported schema marker,
/// unknown fields, resource limits, or an invalid v1 envelope structure.
pub fn parse_envelope(json: &str) -> Result<CalibrationEnvelope, EnvelopeError> {
    parse_envelope_with_report(json).map(|(envelope, _report)| envelope)
}

/// Parse a calibration envelope and return its contract diagnostics.
///
/// # Arguments
///
/// * `json` - UTF-8 JSON calibration-envelope document to parse and enforce
///   against [`CALIBRATION_CONTRACT`].
///
/// # Errors
///
/// Returns [`EnvelopeError::StrictLoad`] for malformed JSON, a missing or
/// malformed schema marker, an unsupported schema version, or an invalid v1
/// envelope structure.
pub(crate) fn parse_envelope_with_report(
    json: &str,
) -> Result<(CalibrationEnvelope, ContractValidationReport), EnvelopeError> {
    CalibrationEnvelope::from_slice_strict(json.as_bytes(), &LoadLimits::default())
        .map_err(|error| EnvelopeError::strict_load(&error))
}

fn collect_initial_ids(envelope: &CalibrationEnvelope) -> HashSet<String> {
    let mut ids = HashSet::new();
    for object in &envelope.prior_market {
        ids.insert(object.id().to_string());
    }
    for datum in &envelope.market_data {
        if !datum.is_quote() {
            ids.insert(datum.id().to_string());
        }
    }
    ids
}

fn build_nodes(steps: &[CalibrationStep]) -> Vec<DependencyNode> {
    steps
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            let io = step.params.io();
            DependencyNode {
                step_index: idx,
                step_id: step.id.clone(),
                kind: io.kind.to_string(),
                reads: io.reads,
                writes: io.writes,
            }
        })
        .collect()
}

fn check_step_id_uniqueness(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let mut first_indices = HashMap::new();
    for (index, step) in envelope.plan.steps.iter().enumerate() {
        if let Some(first_index) = first_indices.get(&step.id) {
            errors.push(EnvelopeError::DuplicateStepId {
                step_id: step.id.clone(),
                first_index: *first_index,
                duplicate_index: index,
            });
        } else {
            first_indices.insert(step.id.clone(), index);
        }
    }
}

fn check_quote_sets(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let mut available: Vec<String> = envelope.plan.quote_sets.keys().cloned().collect();
    available.sort();
    for (idx, step) in envelope.plan.steps.iter().enumerate() {
        if !envelope.plan.quote_sets.contains_key(&step.quote_set) {
            errors.push(EnvelopeError::UndefinedQuoteSet {
                step_index: idx,
                step_id: step.id.clone(),
                ref_name: step.quote_set.clone(),
                available: available.clone(),
                suggestion: closest_match(&step.quote_set, &available),
            });
        }
    }
}

fn check_market_data_uniqueness(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    use std::collections::HashMap;
    let mut seen: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for datum in &envelope.market_data {
        let key = if datum.is_quote() {
            "quote"
        } else {
            datum.kind_name()
        };
        if !seen.entry(key).or_default().insert(datum.id().to_string()) {
            errors.push(EnvelopeError::DuplicateMarketDatumId {
                datum_kind: key.to_string(),
                id: datum.id().to_string(),
            });
        }
    }
}

fn check_quote_sets_resolve(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let quote_ids: HashSet<&str> = envelope
        .market_data
        .iter()
        .filter(|d| d.is_quote())
        .map(|d| d.id())
        .collect();
    for (set_name, ids) in &envelope.plan.quote_sets {
        for qid in ids {
            if !quote_ids.contains(qid.as_str()) {
                errors.push(EnvelopeError::QuoteIdNotInMarketData {
                    quote_set: set_name.clone(),
                    id: qid.to_string(),
                });
            }
        }
    }
}

fn check_quote_data(envelope: &CalibrationEnvelope, errors: &mut Vec<EnvelopeError>) {
    let quote_by_id: std::collections::HashMap<&str, MarketQuote> = envelope
        .market_data
        .iter()
        .filter_map(|datum| datum.as_quote().map(|quote| (datum.id(), quote)))
        .collect();

    for step in &envelope.plan.steps {
        let Some(quote_ids) = envelope.plan.quote_sets.get(&step.quote_set) else {
            continue;
        };
        for quote_id in quote_ids {
            if let Some(quote) = quote_by_id.get(quote_id.as_str()) {
                validate_quote_payload(&step.id, quote, errors);
            }
        }
    }
}

fn validate_quote_payload(step_id: &str, quote: &MarketQuote, errors: &mut Vec<EnvelopeError>) {
    if let Err(error) = quote.validate() {
        errors.push(EnvelopeError::QuoteDataInvalid {
            step_id: step_id.to_string(),
            quote_id: quote.id().to_string(),
            reason: error.to_string(),
        });
    }
}

fn check_dependencies(
    initial_ids: &HashSet<String>,
    nodes: &[DependencyNode],
    errors: &mut Vec<EnvelopeError>,
) {
    let mut available: BTreeSet<String> = initial_ids.iter().cloned().collect();
    for node in nodes {
        for read_id in &node.reads {
            if !available.contains(read_id) {
                errors.push(EnvelopeError::MissingDependency {
                    step_index: node.step_index,
                    step_id: node.step_id.clone(),
                    step_kind: node.kind.clone(),
                    missing_id: read_id.clone(),
                    missing_kind: "curve".to_string(),
                    available: available.iter().cloned().collect(),
                });
            }
        }
        for write_id in &node.writes {
            available.insert(write_id.clone());
        }
    }
}

/// Closest-match suggestion for a misspelled identifier (Levenshtein ≤ 3).
fn closest_match(target: &str, candidates: &[String]) -> Option<String> {
    candidates
        .iter()
        .map(|c| (c, levenshtein(target, c)))
        .filter(|(_, d)| *d > 0 && *d <= 3)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c.clone())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::market_datum::{MarketDatum, PriceDatum};
    use crate::api::schema::{
        CalibrationPlan, CalibrationResultEnvelope, CalibrationSchema, CalibrationStep,
        DiscountCurveParams, StepParams,
    };
    use crate::quotes::ids::{Pillar, QuoteId};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::types::IndexId;
    use finstack_quant_valuations::market::conventions::ids::{
        CdsConventionKey, CdsDocClause, IrFutureContractId,
    };

    fn empty_envelope(id: &str) -> CalibrationEnvelope {
        CalibrationEnvelope {
            schema_url: None,
            schema: crate::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: id.to_string(),
                description: None,
                quote_sets: Default::default(),
                steps: Vec::new(),
                settings: Default::default(),
            },
            market_data: Vec::new(),
            prior_market: Vec::new(),
        }
    }

    fn discount_step(id: &str, quote_set: &str, curve_id: &str) -> CalibrationStep {
        // Use serde to construct DiscountCurveParams since several fields rely
        // on serde defaults (interpolation, conventions, etc.).
        let params: DiscountCurveParams = serde_json::from_value(serde_json::json!({
            "curve_id": curve_id,
            "currency": "USD",
            "base_date": "2026-05-08",
        }))
        .expect("default discount params deserialize");
        CalibrationStep {
            id: id.to_string(),
            quote_set: quote_set.to_string(),
            params: StepParams::Discount(params),
        }
    }

    fn semantically_invalid_envelope() -> CalibrationEnvelope {
        let mut envelope = empty_envelope("invalid");
        envelope
            .plan
            .steps
            .push(discount_step("first", "missing-first", "USD-OIS"));
        envelope
            .plan
            .steps
            .push(discount_step("second", "missing-second", "USD-SOFR"));
        envelope
    }

    fn duplicate_price_envelope() -> CalibrationEnvelope {
        let mut envelope = empty_envelope("duplicate-market-data");
        envelope.market_data = vec![
            MarketDatum::Price(PriceDatum {
                id: "SPX".into(),
                scalar: MarketScalar::Unitless(5_000.0),
            }),
            MarketDatum::Price(PriceDatum {
                id: "SPX".into(),
                scalar: MarketScalar::Unitless(5_001.0),
            }),
        ];
        envelope
    }

    fn nested_json(depth: usize) -> serde_json::Value {
        (0..depth).fold(
            serde_json::json!("leaf"),
            |value, _| serde_json::json!({"nested": value}),
        )
    }

    #[test]
    fn empty_envelope_validates_clean() {
        let env = empty_envelope("smoke");
        let report = validate(&env);
        assert!(report.errors.is_empty());
        assert!(report.dependency_graph.nodes.is_empty());
    }

    #[test]
    fn request_strict_loader_aggregates_bounded_semantic_diagnostics() {
        let envelope = semantically_invalid_envelope();
        let bytes = serde_json::to_vec(&envelope).expect("serialize invalid request");
        let (_loaded, report) = CalibrationEnvelope::from_slice_strict(
            &bytes,
            &LoadLimits::default().with_max_diagnostics(1),
        )
        .expect("bounded strict loading returns semantic diagnostics");
        assert!(report.truncated);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(
            report.diagnostics[0].code,
            "calibration/undefined-quote-set"
        );
        assert_eq!(
            report.diagnostics[0].pointer.as_deref(),
            Some("/plan/steps/0/quote_set")
        );
    }

    #[test]
    fn validate_json_and_execute_reject_semantically_invalid_envelopes() {
        let semantic = semantically_invalid_envelope();
        let json = serde_json::to_string(&semantic).expect("serialize semantic fixture");
        let error = validate_calibration_json(&json)
            .expect_err("validate_calibration_json must enforce semantic validation");
        assert!(matches!(error, EnvelopeError::UndefinedQuoteSet { .. }));

        let duplicate = duplicate_price_envelope();
        let error = crate::api::engine::calibrate(&duplicate)
            .expect_err("execute path must not bypass envelope validation");
        assert!(matches!(
            error,
            crate::api::engine::ExecuteError::Envelope {
                error: EnvelopeError::DuplicateMarketDatumId { .. },
                ..
            }
        ));
    }

    #[test]
    fn undefined_quote_set_is_caught_with_suggestion() {
        let mut env = empty_envelope("test");
        env.plan
            .quote_sets
            .insert("usd_quotes".to_string(), Vec::new());
        env.plan
            .steps
            .push(discount_step("d", "usd_quotess", "USD-OIS"));

        let report = validate(&env);
        let err = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::UndefinedQuoteSet { .. }))
            .expect("undefined quote_set error");
        match err {
            EnvelopeError::UndefinedQuoteSet {
                ref_name,
                suggestion,
                ..
            } => {
                assert_eq!(ref_name, "usd_quotess");
                assert_eq!(suggestion.as_deref(), Some("usd_quotes"));
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn dry_run_returns_pretty_json() {
        let env = empty_envelope("smoke");
        let json = serde_json::to_string(&env).expect("serialize");
        let report_json = dry_run_json(&json).expect("dry_run_json");
        assert!(report_json.contains("\"errors\""));
        assert!(report_json.contains("\"dependency_graph\""));
        let report = dry_run(&json).expect("dry_run");
        assert!(report.errors.is_empty());
        assert_eq!(
            report.to_json_pretty().expect("pretty report"),
            report_json,
            "the JSON twin must serialize the typed report"
        );
    }

    fn deposit(id: &str, rate: f64) -> MarketDatum {
        MarketDatum::RateQuote(RateQuote::Deposit {
            id: QuoteId::new(id),
            index: IndexId::new("USD-SOFR-1M"),
            pillar: Pillar::Tenor("1M".parse().expect("tenor")),
            rate,
        })
    }

    #[test]
    fn from_attached_steps_derives_quote_sets_and_dedups_market_data() {
        let shared = deposit("USD-DEP-1M", 0.05);
        let envelope = CalibrationEnvelope::from_attached_steps(
            "attached".to_string(),
            None,
            Default::default(),
            Default::default(),
            vec![
                (discount_step("a", "set", "USD-OIS"), vec![shared.clone()]),
                (discount_step("b", "set", "USD-OIS-2"), vec![shared]),
                (discount_step("c", "other", "USD-OIS-3"), Vec::new()),
            ],
        )
        .expect("identical attached quotes are collected once");
        assert_eq!(envelope.plan.id, "attached");
        assert_eq!(envelope.plan.steps.len(), 3);
        assert_eq!(
            envelope.plan.quote_sets.get("set"),
            Some(&vec![QuoteId::new("USD-DEP-1M")])
        );
        assert!(!envelope.plan.quote_sets.contains_key("other"));
        assert_eq!(envelope.market_data.len(), 1);
        assert!(envelope.prior_market.is_empty());
    }

    #[test]
    fn from_attached_steps_rejects_quote_set_conflict() {
        let error = CalibrationEnvelope::from_attached_steps(
            "p".to_string(),
            None,
            Default::default(),
            Default::default(),
            vec![
                (
                    discount_step("a", "set", "USD-OIS"),
                    vec![deposit("Q1", 0.05)],
                ),
                (
                    discount_step("b", "set", "USD-OIS-2"),
                    vec![deposit("Q2", 0.05)],
                ),
            ],
        )
        .expect_err("same set name with different ids must conflict");
        assert_eq!(
            error,
            EnvelopeError::QuoteSetConflict {
                quote_set: "set".to_string()
            }
        );
        assert_eq!(error.kind_str(), "quote_set_conflict");
    }

    #[test]
    fn from_attached_steps_rejects_conflicting_payloads() {
        let error = CalibrationEnvelope::from_attached_steps(
            "p".to_string(),
            None,
            Default::default(),
            Default::default(),
            vec![
                (
                    discount_step("a", "set_a", "USD-OIS"),
                    vec![deposit("Q1", 0.05)],
                ),
                (
                    discount_step("b", "set_b", "USD-OIS-2"),
                    vec![deposit("Q1", 0.06)],
                ),
            ],
        )
        .expect_err("one id with two payloads must conflict");
        assert_eq!(
            error,
            EnvelopeError::ConflictingMarketDatum {
                id: "Q1".to_string()
            }
        );
        assert!(error.to_string().contains("conflicting attached payloads"));
    }

    #[test]
    fn validate_fail_fast_returns_first_static_error() {
        let mut env = empty_envelope("fail-fast");
        env.plan
            .steps
            .push(discount_step("a", "missing_a", "USD-OIS"));
        env.plan
            .steps
            .push(discount_step("b", "missing_b", "EUR-OIS"));
        let first = validate(&env).errors.into_iter().next().expect("errors");
        assert_eq!(validate_fail_fast(&env), Err(first));
        assert_eq!(validate_fail_fast(&empty_envelope("ok")), Ok(()));
    }

    #[test]
    fn missing_dependency_for_forward_step_without_discount() {
        // Build a forward step that reads "USD-OIS" without a producing step
        // and without a prior discount curve — should surface MissingDependency.
        let mut env = empty_envelope("missing");
        env.plan
            .quote_sets
            .insert("fwd_quotes".to_string(), Vec::new());
        let params: crate::api::schema::ForwardCurveParams =
            serde_json::from_value(serde_json::json!({
                "curve_id": "USD-3M-LIBOR",
                "currency": "USD",
                "base_date": "2026-05-08",
                "tenor_years": 0.25,
                "discount_curve_id": "USD-OIS",
            }))
            .expect("forward params");
        env.plan.steps.push(CalibrationStep {
            id: "fwd".to_string(),
            quote_set: "fwd_quotes".to_string(),
            params: StepParams::Forward(params),
        });

        let report = validate(&env);
        let missing = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::MissingDependency { .. }))
            .expect("missing dependency error");
        if let EnvelopeError::MissingDependency { missing_id, .. } = missing {
            assert_eq!(missing_id, "USD-OIS");
        }
    }

    #[test]
    fn validate_reports_non_finite_quote_payload() {
        let mut env = empty_envelope("bad-quote");
        env.plan
            .quote_sets
            .insert("rates".to_string(), vec![QuoteId::new("USD-DEP-1M")]);
        env.plan
            .steps
            .push(discount_step("discount", "rates", "USD-OIS"));
        env.market_data
            .push(MarketDatum::RateQuote(RateQuote::Deposit {
                id: QuoteId::new("USD-DEP-1M"),
                index: IndexId::new("USD-SOFR-1M"),
                pillar: Pillar::Tenor("1M".parse().expect("tenor")),
                rate: f64::NAN,
            }));

        let report = validate(&env);
        let err = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::QuoteDataInvalid { .. }))
            .expect("quote data error");
        if let EnvelopeError::QuoteDataInvalid {
            step_id,
            quote_id,
            reason,
        } = err
        {
            assert_eq!(step_id, "discount");
            assert_eq!(quote_id, "USD-DEP-1M");
            assert!(reason.contains("rate must be finite"));
        }
    }

    #[test]
    fn missing_or_non_finite_futures_convexity_is_envelope_error() {
        let expiry = finstack_quant_core::dates::Date::from_calendar_date(
            2026,
            finstack_quant_core::dates::Month::March,
            17,
        )
        .expect("valid expiry");
        let mut env = empty_envelope("bad-convexity");
        env.plan
            .quote_sets
            .insert("rates".to_string(), vec![QuoteId::new("USD-FUT")]);
        env.plan
            .steps
            .push(discount_step("discount", "rates", "USD-OIS"));
        env.market_data
            .push(MarketDatum::RateQuote(RateQuote::Futures {
                id: QuoteId::new("USD-FUT"),
                contract: IrFutureContractId::new("CME:SR3"),
                expiry,
                price: 96.50,
                convexity_adjustment: f64::NAN,
            }));

        let report = validate(&env);
        let err = report
            .errors
            .iter()
            .find(|e| matches!(e, EnvelopeError::QuoteDataInvalid { .. }))
            .expect("non-finite convexity must be QuoteDataInvalid");
        if let EnvelopeError::QuoteDataInvalid {
            step_id,
            quote_id,
            reason,
        } = err
        {
            assert_eq!(step_id, "discount");
            assert_eq!(quote_id, "USD-FUT");
            assert!(reason.contains("convexity_adjustment must be finite"));
        }

        env.market_data.clear();
        env.market_data
            .push(MarketDatum::RateQuote(RateQuote::Futures {
                id: QuoteId::new("USD-FUT"),
                contract: IrFutureContractId::new("CME:SR3"),
                expiry,
                price: 96.50,
                convexity_adjustment: 0.0,
            }));
        let mut value = serde_json::to_value(&env).expect("serialize envelope");
        let quote = value["market_data"]
            .as_array_mut()
            .expect("market_data array")
            .iter_mut()
            .find(|datum| datum.get("id").and_then(|id| id.as_str()) == Some("USD-FUT"))
            .expect("futures quote");
        quote
            .as_object_mut()
            .expect("quote object")
            .remove("convexity_adjustment");
        let error = parse_envelope(&value.to_string())
            .expect_err("missing convexity_adjustment must fail parse");
        assert!(
            matches!(error, EnvelopeError::StrictLoad { .. }),
            "missing convexity must be a strict load error, got {error}"
        );
    }

    #[test]
    fn dry_run_reports_invalid_quote_payload() {
        let mut env = empty_envelope("bad-quote-json");
        env.plan
            .quote_sets
            .insert("cds".to_string(), vec![QuoteId::new("CDS-ACME-5Y")]);
        env.plan
            .steps
            .push(discount_step("discount", "cds", "USD-OIS"));
        env.market_data
            .push(MarketDatum::CdsQuote(CdsQuote::CdsParSpread {
                id: QuoteId::new("CDS-ACME-5Y"),
                entity: "ACME".to_string(),
                convention: CdsConventionKey {
                    currency: Currency::USD,
                    doc_clause: CdsDocClause::Cr14,
                },
                pillar: Pillar::Tenor("5Y".parse().expect("tenor")),
                spread_bp: -1.0,
                recovery_rate: 0.40,
            }));

        let json = serde_json::to_string(&env).expect("serialize");
        let report_json = dry_run_json(&json).expect("dry_run_json");
        assert!(report_json.contains("\"kind\": \"quote_data_invalid\""));
        assert!(report_json.contains("\"quote_id\": \"CDS-ACME-5Y\""));
    }

    #[test]
    fn json_parse_error_surfaces_as_strict_load_diagnostic() {
        let error = dry_run("not json").expect_err("malformed JSON");
        let EnvelopeError::StrictLoad { message, .. } = error else {
            unreachable!("malformed JSON should produce a strict load error");
        };
        assert!(!message.is_empty());
    }

    #[test]
    fn calibration_schema_accepts_only_the_exact_v1_marker() {
        // schema-rejection-test
        let current = serde_json::to_string(&empty_envelope("current")).expect("serialize");
        let (parsed, report) =
            parse_envelope_with_report(&current).expect("current schema is supported");
        assert_eq!(parsed.schema, CalibrationSchema::Calibration);
        assert!(report.diagnostics.is_empty());

        for rejected in [
            "finstack_quant.calibration/0",
            "finstack_quant.calibration/2",
        ] {
            let mut value = serde_json::to_value(empty_envelope("rejected")).expect("serialize");
            value["schema"] = serde_json::json!(rejected);
            assert!(parse_envelope_with_report(&value.to_string()).is_err());
        }
    }

    #[test]
    fn calibration_schema_rejects_missing_zero_future_and_malformed_values() {
        // schema-rejection-test
        let base = serde_json::to_value(empty_envelope("invalid")).expect("serialize");
        let cases = [
            None,
            Some("finstack_quant.calibration/0"),
            Some("finstack_quant.calibration/2"),
            Some("finstack_quant.calibration/not-a-version"),
            Some("other.calibration/1"),
        ];
        for schema in cases {
            let mut value = base.clone();
            match schema {
                Some(schema) => value["schema"] = serde_json::json!(schema),
                None => {
                    value
                        .as_object_mut()
                        .expect("envelope object")
                        .remove("schema");
                }
            }
            assert!(matches!(
                parse_envelope(&value.to_string()).expect_err("schema must fail"),
                EnvelopeError::StrictLoad { .. }
            ));
        }
    }

    #[test]
    fn calibration_result_strict_loader_enforces_current_missing_zero_future_and_malformed_schema()
    {
        // schema-rejection-test
        let envelope = crate::api::engine::calibrate(&empty_envelope("result"))
            .expect("empty calibration executes");
        let base = serde_json::to_value(envelope).expect("result serializes");
        let cases = [
            (
                "current",
                Some(serde_json::json!("finstack_quant.calibration/1")),
                None,
            ),
            ("missing", None, Some("contract/version-missing")),
            (
                "zero",
                Some(serde_json::json!("finstack_quant.calibration/0")),
                Some("contract/version-unsupported"),
            ),
            (
                "future",
                Some(serde_json::json!("finstack_quant.calibration/2")),
                Some("contract/version-unsupported"),
            ),
            (
                "malformed",
                Some(serde_json::json!(
                    "finstack_quant.calibration/not-a-version"
                )),
                Some("contract/schema-malformed"),
            ),
            (
                "wrong-type",
                Some(serde_json::json!(2)),
                Some("contract/schema-malformed"),
            ),
        ];

        for (name, schema, expected_code) in cases {
            let mut value = base.clone();
            match schema {
                Some(schema) => value["schema"] = schema,
                None => {
                    value
                        .as_object_mut()
                        .expect("result envelope object")
                        .remove("schema");
                }
            }
            let bytes = serde_json::to_vec(&value).expect("case serializes");
            match expected_code {
                None => {
                    let (loaded, report) = CalibrationResultEnvelope::from_slice_strict(
                        &bytes,
                        &LoadLimits::default(),
                    )
                    .expect("current calibration result schema should load");
                    assert_eq!(loaded.schema, CalibrationSchema::Calibration);
                    assert!(report.diagnostics.is_empty());
                }
                Some(expected_code) => {
                    let error = CalibrationResultEnvelope::from_slice_strict(
                        &bytes,
                        &LoadLimits::default(),
                    )
                    .expect_err("invalid result schema must fail");
                    let ContractError::Report(report) = error else {
                        unreachable!("{name} must return a structured report");
                    };
                    assert_eq!(report.diagnostics[0].code, expected_code, "{name}");
                }
            }
        }

        let bytes = serde_json::to_vec(&base).expect("result serializes");
        let limits = LoadLimits::default().with_max_bytes(bytes.len() - 1);
        let error = CalibrationResultEnvelope::from_slice_strict(&bytes, &limits)
            .expect_err("strict result loading enforces byte limits");
        assert!(matches!(
            error,
            ContractError::LimitExceeded { what: "bytes", .. }
        ));
    }

    #[test]
    fn calibration_result_strict_loader_validates_nested_final_market() {
        let envelope = crate::api::engine::calibrate(&empty_envelope("result-market"))
            .expect("empty calibration executes");
        let base = serde_json::to_value(envelope).expect("result serializes");

        for (name, version, expected_code) in [
            ("missing", None, "contract/version-missing"),
            (
                "future",
                Some(serde_json::json!(3)),
                "contract/version-unsupported",
            ),
            (
                "invalid",
                Some(serde_json::json!("two")),
                "contract/structure-invalid",
            ),
        ] {
            let mut value = base.clone();
            match version {
                Some(version) => value["result"]["final_market"]["schema_version"] = version,
                None => {
                    value["result"]["final_market"]
                        .as_object_mut()
                        .expect("final_market object")
                        .remove("schema_version");
                }
            }
            let bytes = serde_json::to_vec(&value).expect("nested version case serializes");
            let error =
                CalibrationResultEnvelope::from_slice_strict(&bytes, &LoadLimits::default())
                    .expect_err("nested final market version must fail");
            let ContractError::Report(report) = error else {
                unreachable!("{name} nested version must return diagnostics");
            };
            let diagnostic = report
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == expected_code)
                .expect("nested version report should contain the expected diagnostic code");
            assert_eq!(
                diagnostic.pointer.as_deref(),
                Some("/result/final_market/schema_version"),
                "{name}: {diagnostic:?}"
            );
        }

        let mut restore_failure = base;
        restore_failure["result"]["final_market"]["collateral"] =
            serde_json::json!({"USD-CSA": "MISSING"});
        let bytes = serde_json::to_vec(&restore_failure).expect("restore case serializes");
        let error = CalibrationResultEnvelope::from_slice_strict(&bytes, &LoadLimits::default())
            .expect_err("invalid nested market references must fail");
        let ContractError::Report(report) = error else {
            unreachable!("restore failure must return diagnostics");
        };
        let diagnostic = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "market-context/missing-collateral-curve")
            .expect("restore diagnostic");
        assert_eq!(
            diagnostic.pointer.as_deref(),
            Some("/result/final_market/collateral/USD-CSA")
        );
    }

    #[test]
    fn calibration_result_nested_market_depth_is_bounded() {
        let envelope = crate::api::engine::calibrate(&empty_envelope("result-depth"))
            .expect("empty calibration executes");
        let mut value = serde_json::to_value(envelope).expect("result serializes");
        value["result"]["final_market"]["hierarchy"] = nested_json(20);
        let bytes = serde_json::to_vec(&value).expect("nested result serializes");

        let error = CalibrationResultEnvelope::from_slice_strict(
            &bytes,
            &LoadLimits::default()
                .with_max_depth(16)
                .with_max_diagnostics(1),
        )
        .expect_err("nested final market depth must be enforced");
        let ContractError::Report(report) = error else {
            unreachable!("nested depth must return bounded structured diagnostics");
        };
        assert_eq!(report.diagnostics.len(), 1);
        let depth = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == "contract/depth-limit")
            .expect("nested depth diagnostic");
        assert_eq!(depth.pointer.as_deref(), Some("/result/final_market"));
    }
    #[test]
    fn duplicate_step_ids_are_structured_validation_errors() {
        let mut envelope = empty_envelope("duplicate-step");
        envelope
            .plan
            .steps
            .push(discount_step("duplicate", "quotes-a", "USD-OIS"));
        envelope
            .plan
            .steps
            .push(discount_step("duplicate", "quotes-b", "USD-SOFR"));

        let report = validate(&envelope);
        let duplicate = report
            .errors
            .iter()
            .find(|error| matches!(error, EnvelopeError::DuplicateStepId { .. }))
            .expect("duplicate step error");
        assert_eq!(duplicate.kind_str(), "duplicate_step_id");
        assert_eq!(duplicate.step_id(), Some("duplicate"));
        assert!(duplicate.to_json().contains("\"duplicate_index\":1"));
    }
}

#[cfg(test)]
mod canonical_validation_tests {
    use super::*;
    use crate::api::market_datum::{MarketDatum, PriceDatum};
    use crate::api::schema::CalibrationPlan;
    use finstack_quant_core::market_data::scalars::MarketScalar;
    use finstack_quant_core::HashMap;

    #[test]
    fn duplicate_price_id_is_flagged() {
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: crate::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: "t".into(),
                description: None,
                quote_sets: Default::default(),
                steps: vec![],
                settings: Default::default(),
            },
            market_data: vec![
                MarketDatum::Price(PriceDatum {
                    id: "AAPL".into(),
                    scalar: MarketScalar::Unitless(100.0),
                }),
                MarketDatum::Price(PriceDatum {
                    id: "AAPL".into(),
                    scalar: MarketScalar::Unitless(101.0),
                }),
            ],
            prior_market: vec![],
        };
        let report = validate(&envelope);
        assert!(
            report.errors.iter().any(|e| matches!(
                e,
                EnvelopeError::DuplicateMarketDatumId { datum_kind, id }
                    if datum_kind == "price" && id == "AAPL"
            )),
            "expected DuplicateMarketDatumId, got: {:?}",
            report.errors
        );
    }

    #[test]
    fn unresolved_quote_id_is_flagged() {
        use crate::quotes::ids::QuoteId;
        let mut quote_sets = HashMap::default();
        quote_sets.insert("usd".to_string(), vec![QuoteId::new("MISSING")]);
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: crate::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: "t".into(),
                description: None,
                quote_sets: quote_sets.into_iter().collect(),
                steps: vec![],
                settings: Default::default(),
            },
            market_data: vec![],
            prior_market: vec![],
        };
        let report = validate(&envelope);
        assert!(
            report.errors.iter().any(|e| matches!(
                e,
                EnvelopeError::QuoteIdNotInMarketData { quote_set, id }
                    if quote_set == "usd" && id == "MISSING"
            )),
            "expected QuoteIdNotInMarketData, got: {:?}",
            report.errors
        );
    }

    #[test]
    fn validate_calibration_json_returns_canonical_envelope() {
        let envelope = CalibrationEnvelope {
            schema_url: None,
            schema: crate::api::schema::CalibrationSchema::CURRENT,
            plan: CalibrationPlan {
                id: "canonical".into(),
                description: None,
                quote_sets: Default::default(),
                steps: vec![],
                settings: Default::default(),
            },
            market_data: vec![],
            prior_market: vec![],
        };
        let json = serde_json::to_string(&envelope).expect("serialize envelope");

        let canonical = validate_calibration_json(&json).expect("canonical envelope");
        let reparsed: CalibrationEnvelope =
            serde_json::from_str(&canonical).expect("canonical JSON should parse");

        assert_eq!(
            reparsed.schema,
            crate::api::schema::CalibrationSchema::CURRENT
        );
        assert_eq!(reparsed.plan.id, "canonical");
    }
}
