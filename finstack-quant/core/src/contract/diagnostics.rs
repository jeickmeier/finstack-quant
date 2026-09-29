//! Structured, bounded diagnostics for persisted artifact loading.

use serde::{Deserialize, Serialize};

use super::LoadLimits;

/// Stage of persisted artifact loading that produced a diagnostic.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LoadPhase {
    /// Source bytes were parsed into a JSON document.
    Parse,
    /// Contract identity and version policy were checked.
    Version,
    /// Document shape and required fields were checked.
    Structure,
    /// Domain invariants and references were checked.
    Semantic,
    /// The validated artifact was converted to canonical JSON bytes.
    Canonicalize,
    /// A content digest was calculated or checked.
    Hash,
    /// Runtime domain objects were built from the validated payload.
    Build,
}

/// Severity assigned to a persisted artifact diagnostic.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A finding that prevents successful loading.
    Error,
    /// A recoverable finding that callers should inspect.
    Warning,
}

/// One structured finding produced while loading a persisted artifact.
///
/// Field names and enum representations are a persisted JSON contract. Unknown
/// fields are rejected during deserialization.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct Diagnostic {
    /// Stable machine-readable code, such as `"contract/version-unsupported"`.
    pub code: String,
    /// Loading stage that produced the finding.
    pub phase: LoadPhase,
    /// Whether the finding is fatal or recoverable.
    pub severity: Severity,
    /// JSON Pointer locating the finding in the source document, when known.
    pub pointer: Option<String>,
    /// Human-readable explanation with source details such as parse line and column.
    pub message: String,
    /// Stable persisted contract identifier, when the finding is contract-specific.
    pub contract: Option<String>,
    /// Version expected by the loader, when version context is available.
    pub expected_version: Option<u32>,
    /// Version found in the payload, when one was present.
    pub actual_version: Option<u32>,
    /// Canonical artifact digest, including its algorithm prefix, when available.
    pub artifact_hash: Option<String>,
    /// Storage revision identifier associated with the artifact, when available.
    pub revision_id: Option<String>,
    /// Instrument identifier associated with the finding, when available.
    pub instrument_id: Option<String>,
    /// Portfolio-position identifier associated with the finding, when available.
    pub position_id: Option<String>,
}

impl Diagnostic {
    /// Create a diagnostic with no optional context fields.
    ///
    /// # Arguments
    ///
    /// * `code` - Stable machine-readable code used by callers to classify the
    ///   finding, such as `"contract/version-unsupported"`.
    /// * `phase` - Loading stage that produced the finding.
    /// * `severity` - Whether the finding prevents loading or is recoverable.
    /// * `message` - Human-readable explanation suitable for logs and reports.
    pub fn new(
        code: impl Into<String>,
        phase: LoadPhase,
        severity: Severity,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            phase,
            severity,
            pointer: None,
            message: message.into(),
            contract: None,
            expected_version: None,
            actual_version: None,
            artifact_hash: None,
            revision_id: None,
            instrument_id: None,
            position_id: None,
        }
    }

    /// Create a fatal parse diagnostic that preserves JSON source location.
    ///
    /// The message records the originating `serde_json` error's one-based line
    /// and column. The diagnostic uses the stable code `"contract/parse-error"`
    /// and the [`LoadPhase::Parse`] phase.
    ///
    /// # Arguments
    ///
    /// * `error` - Originating JSON parser error whose message, line, and column
    ///   identify the malformed source.
    /// * `pointer` - Optional JSON Pointer associated with the parse context;
    ///   use `None` when parsing failed before a path could be identified.
    pub fn from_parse_error(error: &serde_json::Error, pointer: Option<String>) -> Self {
        Self::new(
            "contract/parse-error",
            LoadPhase::Parse,
            Severity::Error,
            format!(
                "JSON parse error at line {}, column {}: {error}",
                error.line(),
                error.column()
            ),
        )
        .with_optional_pointer(pointer)
    }

    /// Attach a JSON Pointer into the source document.
    ///
    /// # Arguments
    ///
    /// * `pointer` - RFC 6901 JSON Pointer locating the finding.
    #[must_use]
    pub fn with_pointer(mut self, pointer: impl Into<String>) -> Self {
        self.pointer = Some(pointer.into());
        self
    }

    /// Attach the persisted contract identifier.
    ///
    /// # Arguments
    ///
    /// * `contract` - Stable contract identifier, without a version suffix.
    #[must_use]
    pub fn with_contract(mut self, contract: impl Into<String>) -> Self {
        self.contract = Some(contract.into());
        self
    }

    /// Attach the contract version expected by the loader.
    ///
    /// # Arguments
    ///
    /// * `expected_version` - Positive schema version required or preferred by
    ///   the loading operation.
    #[must_use]
    pub fn with_expected_version(mut self, expected_version: u32) -> Self {
        self.expected_version = Some(expected_version);
        self
    }

    /// Attach the contract version found in the payload.
    ///
    /// # Arguments
    ///
    /// * `actual_version` - Schema version read from the persisted payload.
    #[must_use]
    pub fn with_actual_version(mut self, actual_version: u32) -> Self {
        self.actual_version = Some(actual_version);
        self
    }

    /// Attach the canonical artifact digest.
    ///
    /// # Arguments
    ///
    /// * `artifact_hash` - Digest including its algorithm prefix, such as
    ///   `"sha256:<64-hex>"`.
    #[must_use]
    pub fn with_artifact_hash(mut self, artifact_hash: impl Into<String>) -> Self {
        self.artifact_hash = Some(artifact_hash.into());
        self
    }

    /// Attach the storage revision identifier.
    ///
    /// # Arguments
    ///
    /// * `revision_id` - Revision identifier assigned by the persistence layer.
    #[must_use]
    pub fn with_revision_id(mut self, revision_id: impl Into<String>) -> Self {
        self.revision_id = Some(revision_id.into());
        self
    }

    /// Attach the instrument identifier associated with the finding.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Stable instrument identifier from the source
    ///   artifact.
    #[must_use]
    pub fn with_instrument_id(mut self, instrument_id: impl Into<String>) -> Self {
        self.instrument_id = Some(instrument_id.into());
        self
    }

    /// Attach the portfolio-position identifier associated with the finding.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Stable position identifier from the source artifact.
    #[must_use]
    pub fn with_position_id(mut self, position_id: impl Into<String>) -> Self {
        self.position_id = Some(position_id.into());
        self
    }

    fn with_optional_pointer(mut self, pointer: Option<String>) -> Self {
        self.pointer = pointer;
        self
    }
}

/// Bounded collection of diagnostics produced by a validation operation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts_export", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ValidationReport {
    /// Individual findings retained up to the configured diagnostic limit.
    pub diagnostics: Vec<Diagnostic>,
    /// Whether one or more additional findings were omitted due to the limit.
    ///
    /// A truncated report with no retained diagnostics is treated as fatal
    /// because the wire format cannot distinguish omitted warnings from errors.
    pub truncated: bool,
}

impl ValidationReport {
    /// Return whether the report represents one or more fatal findings.
    ///
    /// A retained error is fatal. A truncated report with an empty diagnostic
    /// list is also fatal by fail-closed policy because a zero-capacity report
    /// cannot encode the severity of omitted findings.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.error_count() != 0
    }

    /// Retain a diagnostic without exceeding the configured report bound.
    ///
    /// Once the bound is reached, [`Self::truncated`] is set. If the incoming
    /// finding is an error and the full report contains only warnings, one
    /// retained warning is replaced so fatal status survives JSON round-trips.
    /// Once an error is retained, additional findings are summarized only by
    /// `truncated`. A limit of zero retains no findings and therefore follows
    /// [`Self::has_errors`]'s fail-closed empty-truncated policy.
    ///
    /// # Arguments
    ///
    /// * `limits` - Resource policy whose `max_diagnostics` field bounds the
    ///   retained findings.
    /// * `d` - Structured finding to retain if capacity remains.
    pub fn push_bounded(&mut self, limits: &LoadLimits, d: Diagnostic) {
        if self.diagnostics.len() < limits.max_diagnostics {
            self.diagnostics.push(d);
            return;
        }

        self.truncated = true;
        let has_retained_error = self
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        if d.severity == Severity::Error && !has_retained_error {
            if let Some(replace) = self.diagnostics.last_mut() {
                *replace = d;
            }
        }
    }

    fn error_count(&self) -> usize {
        let retained_errors = self
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .count();
        if retained_errors == 0 && self.truncated && self.diagnostics.is_empty() {
            1
        } else {
            retained_errors
        }
    }
}

impl ValidationReport {
    /// One-line summary used as the message of every report-carrying error:
    /// `validation failed with N error(s)`, counting error-severity
    /// diagnostics only.
    #[must_use]
    pub fn summary(&self) -> String {
        format!("validation failed with {} error(s)", self.error_count())
    }
}

impl std::fmt::Display for ValidationReport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error_count().fmt(formatter)
    }
}

/// Failure produced while parsing, validating, or materializing a persisted contract.
#[non_exhaustive]
#[derive(thiserror::Error, Debug)]
pub enum ContractError {
    /// Payload version is zero or outside the descriptor's supported range.
    #[error("unsupported contract version: found {found}, supported {min}..={max} for {contract}")]
    UnsupportedVersion {
        /// Stable contract identifier.
        contract: String,
        /// Unsupported version found in the payload.
        found: u32,
        /// Lowest supported version.
        min: u32,
        /// Highest supported version.
        max: u32,
    },
    /// A strict contract payload omitted its version marker.
    #[error(
        "missing contract version for {contract}; strict loading requires an explicit version"
    )]
    MissingVersion {
        /// Stable contract identifier whose version was missing.
        contract: String,
    },
    /// A schema marker was missing, malformed, or named another contract.
    #[error("malformed contract schema string {value:?}; expected \"{expected}/<version>\"")]
    MalformedSchema {
        /// Rejected schema marker.
        value: String,
        /// Stable contract identifier expected before the version suffix.
        expected: String,
    },
    /// A configured resource bound was exceeded.
    #[error("input exceeds limit: {what} {found} > {limit}")]
    LimitExceeded {
        /// Resource whose bound was exceeded.
        what: &'static str,
        /// Observed resource count or size.
        found: usize,
        /// Configured maximum count or size.
        limit: usize,
    },
    /// Structured validation produced one or more fatal findings.
    #[error("{}", .0.summary())]
    Report(Box<ValidationReport>),
    /// A core-domain operation failed while processing the contract.
    #[error(transparent)]
    Core(#[from] crate::Error),
}
