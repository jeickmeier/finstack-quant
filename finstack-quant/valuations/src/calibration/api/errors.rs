//! Structured error types for calibration envelope diagnostics.
//!
//! [`EnvelopeError`] is the canonical error type for static envelope validation
//! and runtime calibration failures. It implements `Display` (human-readable),
//! `serde::Serialize` (machine-readable JSON for Python/WASM bindings), and
//! `From<EnvelopeError> for finstack_quant_core::Error` for backwards-compatible
//! propagation through existing call sites that take `finstack_quant_core::Result`.

fn json_parse_loc(line: &Option<u32>, col: &Option<u32>) -> String {
    match (line, col) {
        (Some(l), Some(c)) => format!(" at line {l}, column {c}"),
        (Some(l), None) => format!(" at line {l}"),
        _ => String::new(),
    }
}

fn suggestion_hint(suggestion: &Option<String>) -> String {
    match suggestion {
        Some(s) => format!(" Did you mean '{s}'?"),
        None => String::new(),
    }
}

fn format_breakdown(breakdown: &[(String, usize)]) -> String {
    breakdown
        .iter()
        .map(|(c, n)| format!("{n} '{c}'"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_worst_quote(id: &Option<String>, residual: &Option<f64>) -> String {
    match (id, residual) {
        (Some(id), Some(r)) => format!(" Worst quote: '{id}' (residual {r:.3e})."),
        _ => String::new(),
    }
}

/// Errors surfaced when an envelope is invalid or calibration fails.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvelopeError {
    /// JSON parse failure (malformed envelope).
    #[error("JSON parse error{}: {message}", json_parse_loc(line, col))]
    JsonParse {
        /// Parser-provided error description.
        message: String,
        /// 1-based line number of the parse failure, when available.
        line: Option<u32>,
        /// 1-based column number of the parse failure, when available.
        col: Option<u32>,
    },
    /// A step's `kind` discriminator is not a recognized variant.
    #[error("step[{step_index}] '{step_id}': unknown kind '{found}'; expected one of: {}", expected_one_of.join(", "))]
    UnknownStepKind {
        /// Zero-based index of the offending step in `plan.steps`.
        step_index: usize,
        /// Step identifier from `plan.steps[i].id`.
        step_id: String,
        /// The unrecognized `kind` value found in the envelope.
        found: String,
        /// Closed list of recognized `kind` values.
        expected_one_of: Vec<String>,
    },
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
    /// A step's `quote_set` contains quotes of a class incompatible with the step.
    #[error("step[{step_index}] '{step_id}' (kind='{step_kind}'): expected quotes of class '{expected_class}', but found: {}", format_breakdown(breakdown))]
    QuoteClassMismatch {
        /// Zero-based index of the offending step.
        step_index: usize,
        /// Step identifier.
        step_id: String,
        /// Step kind.
        step_kind: String,
        /// The quote class the step expected (e.g. `"rates"`).
        expected_class: String,
        /// `(class, count)` breakdown of the actual quote classes present.
        breakdown: Vec<(String, usize)>,
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
    /// A JSON response payload could not be serialized.
    #[error("failed to serialize {target} as JSON: {message}")]
    JsonSerialize {
        /// Payload being serialized, e.g. `"ValidationReport"`.
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
            EnvelopeError::JsonParse { .. } => "json_parse",
            EnvelopeError::UnknownStepKind { .. } => "unknown_step_kind",
            EnvelopeError::MissingDependency { .. } => "missing_dependency",
            EnvelopeError::UndefinedQuoteSet { .. } => "undefined_quote_set",
            EnvelopeError::QuoteClassMismatch { .. } => "quote_class_mismatch",
            EnvelopeError::SolverNotConverged { .. } => "solver_not_converged",
            EnvelopeError::QuoteDataInvalid { .. } => "quote_data_invalid",
            EnvelopeError::DuplicateMarketDatumId { .. } => "duplicate_market_datum_id",
            EnvelopeError::QuoteIdNotInMarketData { .. } => "quote_id_not_in_market_data",
            EnvelopeError::JsonSerialize { .. } => "json_serialize",
        }
    }

    /// Step identifier associated with this error, if any.
    ///
    /// Returns `None` for variants that are not bound to a specific step
    /// (`JsonParse`, `StepCycle`).
    pub fn step_id(&self) -> Option<&str> {
        match self {
            EnvelopeError::UnknownStepKind { step_id, .. }
            | EnvelopeError::MissingDependency { step_id, .. }
            | EnvelopeError::UndefinedQuoteSet { step_id, .. }
            | EnvelopeError::QuoteClassMismatch { step_id, .. }
            | EnvelopeError::SolverNotConverged { step_id, .. }
            | EnvelopeError::QuoteDataInvalid { step_id, .. } => Some(step_id),
            EnvelopeError::JsonParse { .. }
            | EnvelopeError::DuplicateMarketDatumId { .. }
            | EnvelopeError::QuoteIdNotInMarketData { .. }
            | EnvelopeError::JsonSerialize { .. } => None,
        }
    }

    /// Serialize to pretty-printed JSON for cross-binding consumption.
    pub fn to_json(&self) -> String {
        match serde_json::to_string_pretty(self) {
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
