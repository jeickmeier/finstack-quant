//! Structured error types for calibration envelope diagnostics.
//!
//! [`EnvelopeError`] is the canonical error type for static envelope validation
//! and runtime calibration failures. It implements `Display` (human-readable),
//! `serde::Serialize` (machine-readable JSON for Python/WASM bindings), and
//! `From<EnvelopeError> for finstack_quant_core::Error` for callers that use
//! the workspace-wide result type.

fn suggestion_hint(suggestion: &Option<String>) -> String {
    match suggestion {
        Some(s) => format!(" Did you mean '{s}'?"),
        None => String::new(),
    }
}

fn format_worst_quote(id: &Option<String>, residual: &Option<f64>) -> String {
    match (id, residual) {
        (Some(id), Some(r)) => format!(" Worst quote: '{id}' (residual {r:.3e})."),
        _ => String::new(),
    }
}

/// One structured finding from the bounded strict loader.
///
/// Mirrors the host-independent fields of
/// [`finstack_quant_core::contract::Diagnostic`] so the Python and WASM
/// bindings can attach the findings as plain records.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub struct StrictLoadDiagnostic {
    /// Stable machine-readable code (e.g. `parse/invalid-json`,
    /// `calibration/undefined-quote-set`).
    pub code: String,
    /// Load phase that produced the finding (`parse`, `version`, `structure`,
    /// `semantic`, ...).
    pub phase: String,
    /// Severity label (`error` or `warning`).
    pub severity: String,
    /// RFC 6901 JSON pointer into the request document, when known.
    pub pointer: Option<String>,
    /// Human-readable description of the finding.
    pub message: String,
    /// Contract identifier the finding was evaluated against, when known.
    pub contract: Option<String>,
    /// Expected contract version, when the finding is version-related.
    pub expected_version: Option<u32>,
    /// Version actually found, when the finding is version-related.
    pub actual_version: Option<u32>,
}

impl StrictLoadDiagnostic {
    /// Project a core contract diagnostic onto the host-facing record.
    ///
    /// # Arguments
    ///
    /// * `diagnostic` - Bounded-loader finding to flatten; phase and severity
    ///   are rendered as their snake-case labels.
    pub fn from_contract(diagnostic: &finstack_quant_core::contract::Diagnostic) -> Self {
        use finstack_quant_core::contract::{LoadPhase, Severity};
        let phase = match diagnostic.phase {
            LoadPhase::Parse => "parse",
            LoadPhase::Version => "version",
            LoadPhase::Structure => "structure",
            LoadPhase::Semantic => "semantic",
            #[allow(unreachable_patterns)]
            _ => "unknown",
        };
        let severity = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            #[allow(unreachable_patterns)]
            _ => "unknown",
        };
        Self {
            code: diagnostic.code.clone(),
            phase: phase.to_string(),
            severity: severity.to_string(),
            pointer: diagnostic.pointer.clone(),
            message: diagnostic.message.clone(),
            contract: diagnostic.contract.clone(),
            expected_version: diagnostic.expected_version,
            actual_version: diagnostic.actual_version,
        }
    }
}

impl EnvelopeError {
    /// Build a [`EnvelopeError::StrictLoad`] from a contract failure.
    ///
    /// Report failures expand every retained diagnostic into the message
    /// (`pointer: message` per line) and into the structured
    /// `diagnostics` list; every other contract error keeps its display
    /// string and an empty diagnostics list.
    ///
    /// # Arguments
    ///
    /// * `error` - Contract failure returned by the bounded strict loader.
    pub fn strict_load(error: &finstack_quant_core::contract::ContractError) -> Self {
        use finstack_quant_core::contract::ContractError;
        match error {
            ContractError::Report(report) => {
                let diagnostics: Vec<StrictLoadDiagnostic> = report
                    .diagnostics
                    .iter()
                    .map(StrictLoadDiagnostic::from_contract)
                    .collect();
                let mut message = format!(
                    "validation failed with {} structured diagnostic(s)",
                    diagnostics.len()
                );
                for diagnostic in &diagnostics {
                    let pointer = diagnostic.pointer.as_deref().unwrap_or("/");
                    message.push_str(&format!("\n  {pointer}: {}", diagnostic.message));
                }
                if report.truncated {
                    message.push_str("\n  (further diagnostics truncated by load limits)");
                }
                Self::StrictLoad {
                    message,
                    diagnostics,
                }
            }
            other => Self::StrictLoad {
                message: other.to_string(),
                diagnostics: Vec::new(),
            },
        }
    }
}

/// Errors surfaced when an envelope is invalid or calibration fails.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvelopeError {
    /// A step references a curve / surface ID that's not produced by an
    /// earlier step or carried in `market_data` / `prior_market`.
    #[error("step[{step_index}] '{step_id}' (kind='{step_kind}'): missing {missing_kind} dependency '{missing_id}'. Available: [{}]", available.join(", "))]
    MissingDependency {
        /// Zero-based index of the offending step in `plan.steps`.
        step_index: usize,
        /// Step identifier.
        step_id: String,
        /// Step kind (e.g. `"forward"`, `"hazard"`).
        step_kind: String,
        /// The missing curve/surface identifier referenced by the step.
        missing_id: String,
        /// Kind of the missing dependency (e.g. `"discount"`, `"surface"`).
        missing_kind: String,
        /// Identifiers available at the time the step would run.
        available: Vec<String>,
    },
    /// A step's `quote_set` field references a name not in `plan.quote_sets`.
    #[error("step[{step_index}] '{step_id}': quote_set '{ref_name}' is not defined in plan.quote_sets. Available: [{}].{}", available.join(", "), suggestion_hint(suggestion))]
    UndefinedQuoteSet {
        /// Zero-based index of the offending step.
        step_index: usize,
        /// Step identifier.
        step_id: String,
        /// The missing `quote_set` name as referenced by the step.
        ref_name: String,
        /// Defined `quote_set` names in the plan.
        available: Vec<String>,
        /// Closest-match suggestion (Levenshtein distance ≤ 3), if any.
        suggestion: Option<String>,
    },
    /// Two calibration steps use the same audit identifier.
    #[error("step[{duplicate_index}] duplicates step ID '{step_id}' first declared at step[{first_index}]")]
    DuplicateStepId {
        /// Duplicated step identifier.
        step_id: String,
        /// Zero-based index of the first declaration.
        first_index: usize,
        /// Zero-based index of the conflicting declaration.
        duplicate_index: usize,
    },
    /// A solver step did not converge to within tolerance.
    #[error("step '{step_id}' did not converge: max residual {max_residual:.3e} > tolerance {tolerance:.3e} after {iterations} iterations.{}", format_worst_quote(worst_quote_id, worst_quote_residual))]
    SolverNotConverged {
        /// Step identifier.
        step_id: String,
        /// Largest absolute residual at termination.
        max_residual: f64,
        /// Configured solver tolerance.
        tolerance: f64,
        /// Iterations performed before termination.
        iterations: u32,
        /// Identifier of the worst-fitting quote, if known.
        worst_quote_id: Option<String>,
        /// Residual of the worst-fitting quote, if known.
        worst_quote_residual: Option<f64>,
    },
    /// Quote data fails domain validation (NaN, out-of-range, etc.).
    #[error("step '{step_id}': quote '{quote_id}' is invalid: {reason}")]
    QuoteDataInvalid {
        /// Step identifier consuming the quote.
        step_id: String,
        /// Quote identifier that failed validation.
        quote_id: String,
        /// Human-readable reason describing the validation failure.
        reason: String,
    },
    /// Two entries in `market_data` share the same `(kind, id)` (or same id
    /// within the quote namespace shared by the eight `*_quote` kinds).
    #[error("market_data contains duplicate id '{id}' within kind '{datum_kind}'")]
    DuplicateMarketDatumId {
        /// `"quote"` (shared namespace for the eight `*_quote` variants) or
        /// the specific datum kind name for non-quote variants.
        ///
        /// Renamed to `datum_kind` in the Rust struct because the enum's serde
        /// tag is already named `kind`; the JSON payload uses `datum_kind`.
        datum_kind: String,
        /// The duplicated identifier.
        id: String,
    },
    /// A quote ID listed in `plan.quote_sets[name]` doesn't resolve to any
    /// quote-kind entry in `market_data`.
    #[error("quote_set '{quote_set}' references id '{id}', which is not present in market_data as a quote")]
    QuoteIdNotInMarketData {
        /// The named quote set in `plan.quote_sets`.
        quote_set: String,
        /// The unresolved quote identifier.
        id: String,
    },
    /// Two steps (or a step and an explicit quote set) define the same
    /// quote-set name with different quote ids.
    #[error("quote set '{quote_set}' is attached by more than one step with different quotes")]
    QuoteSetConflict {
        /// The conflicting quote-set name.
        quote_set: String,
    },
    /// One quote id is attached to steps with two different payloads.
    #[error("quote id '{id}' has conflicting attached payloads")]
    ConflictingMarketDatum {
        /// The quote identifier attached with conflicting payloads.
        id: String,
    },
    /// Strict bounded contract loading rejected the request or result.
    ///
    /// `message` already lists every retained diagnostic (JSON pointer and
    /// message); `diagnostics` carries the same findings structurally so hosts
    /// can surface them as records without parsing the message.
    #[error("strict calibration contract load failed: {message}")]
    StrictLoad {
        /// Bounded parser or semantic-validation summary, including each
        /// retained diagnostic's pointer and message.
        message: String,
        /// Structured contract diagnostics retained by the bounded loader;
        /// empty when the failure carried no per-pointer findings.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        diagnostics: Vec<StrictLoadDiagnostic>,
    },
    /// A JSON response payload could not be serialized.
    #[error("failed to serialize {target} as JSON: {message}")]
    JsonSerialize {
        /// Payload being serialized, e.g. `"CalibrationValidationReport"`.
        target: String,
        /// Serializer-provided error description.
        message: String,
    },
}

impl EnvelopeError {
    /// Snake-case discriminator matching the `kind` tag of the serialized form.
    ///
    /// Useful for cross-binding consumers that want to pattern-match on the
    /// error kind without parsing the full JSON payload.
    pub fn kind_str(&self) -> &'static str {
        match self {
            EnvelopeError::MissingDependency { .. } => "missing_dependency",
            EnvelopeError::UndefinedQuoteSet { .. } => "undefined_quote_set",
            EnvelopeError::DuplicateStepId { .. } => "duplicate_step_id",
            EnvelopeError::SolverNotConverged { .. } => "solver_not_converged",
            EnvelopeError::QuoteDataInvalid { .. } => "quote_data_invalid",
            EnvelopeError::DuplicateMarketDatumId { .. } => "duplicate_market_datum_id",
            EnvelopeError::QuoteIdNotInMarketData { .. } => "quote_id_not_in_market_data",
            EnvelopeError::QuoteSetConflict { .. } => "quote_set_conflict",
            EnvelopeError::ConflictingMarketDatum { .. } => "conflicting_market_datum",
            EnvelopeError::JsonSerialize { .. } => "json_serialize",
            EnvelopeError::StrictLoad { .. } => "strict_load",
        }
    }

    /// Step identifier associated with this error, if any.
    ///
    /// Returns `None` for variants that are not bound to a specific step
    /// (for example, [`EnvelopeError::StrictLoad`]).
    pub fn step_id(&self) -> Option<&str> {
        match self {
            EnvelopeError::MissingDependency { step_id, .. }
            | EnvelopeError::UndefinedQuoteSet { step_id, .. }
            | EnvelopeError::DuplicateStepId { step_id, .. }
            | EnvelopeError::SolverNotConverged { step_id, .. }
            | EnvelopeError::QuoteDataInvalid { step_id, .. } => Some(step_id),
            EnvelopeError::DuplicateMarketDatumId { .. }
            | EnvelopeError::QuoteIdNotInMarketData { .. }
            | EnvelopeError::QuoteSetConflict { .. }
            | EnvelopeError::ConflictingMarketDatum { .. }
            | EnvelopeError::JsonSerialize { .. }
            | EnvelopeError::StrictLoad { .. } => None,
        }
    }

    /// Serialize to compact JSON for cross-binding consumption.
    pub fn to_json(&self) -> String {
        match serde_json::to_string(self) {
            Ok(json) => json,
            Err(err) => serde_json::json!({
                "kind": "json_serialize",
                "target": "EnvelopeError",
                "message": err.to_string(),
            })
            .to_string(),
        }
    }
}

impl From<EnvelopeError> for finstack_quant_core::Error {
    fn from(err: EnvelopeError) -> Self {
        let category = err.kind_str().to_string();
        finstack_quant_core::Error::Calibration {
            message: err.to_string(),
            category,
        }
    }
}
