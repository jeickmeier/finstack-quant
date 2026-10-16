//! Identity and version policy for persisted contracts.

use std::fmt::Display;

use serde::de::DeserializeOwned;

use super::{
    deserialize_json_value, parse_json_value, ContractError, Diagnostic, LoadLimits, LoadPhase,
    Severity, ValidationReport,
};

/// Identity and version policy for one persisted contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractDescriptor {
    /// Stable contract identifier, such as `"finstack_quant.instrument"`.
    pub id: &'static str,
}

impl ContractDescriptor {
    /// The sole version emitted and accepted by all persisted contracts.
    pub const VERSION: u32 = 1;

    /// Create a strict v1 descriptor.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable, exact contract identifier used before the slash in
    ///   schema strings.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self { id }
    }

    /// Return the current schema marker as `"<contract>/<version>"`.
    #[must_use]
    pub fn schema_string(&self) -> String {
        format!("{}/{}", self.id, Self::VERSION)
    }

    /// Parse and validate an exact schema marker for this contract.
    ///
    /// The contract identifier must match exactly. Any version other than v1,
    /// missing components, and non-decimal version text are rejected.
    ///
    /// # Arguments
    ///
    /// * `s` - Schema marker in the exact `"<contract>/<version>"` format.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::MalformedSchema`] when the marker is malformed
    /// or names another contract, and [`ContractError::UnsupportedVersion`]
    /// when its parsed version is not v1.
    pub fn parse_schema(&self, s: &str) -> Result<u32, ContractError> {
        let expected = self.id.to_string();
        let Some(version_text) = s
            .strip_prefix(self.id)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return Err(ContractError::MalformedSchema {
                value: s.to_string(),
                expected,
            });
        };

        if version_text.is_empty() || version_text.contains('/') {
            return Err(ContractError::MalformedSchema {
                value: s.to_string(),
                expected,
            });
        }

        let version = version_text
            .parse::<u32>()
            .map_err(|_| ContractError::MalformedSchema {
                value: s.to_string(),
                expected,
            })?;
        self.validate_version(version)
    }

    /// Resolve a required payload version.
    ///
    /// # Arguments
    ///
    /// * `found` - Explicit payload version, or `None` when the payload omitted
    ///   its version marker.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::MissingVersion`] under strict missing-version
    /// policy, or [`ContractError::UnsupportedVersion`] when the resolved
    /// version is not v1.
    pub fn resolve(&self, found: Option<u32>) -> Result<u32, ContractError> {
        match found {
            Some(version) => self.validate_version(version),
            None => Err(ContractError::MissingVersion {
                contract: self.id.to_string(),
            }),
        }
    }

    /// Resolve a required explicit numeric version and return structured errors.
    ///
    /// # Arguments
    ///
    /// * `found` - Explicit numeric version read from the payload, or `None`
    ///   when its version key was absent.
    /// * `pointer` - JSON Pointer locating the numeric version key.
    /// * `limits` - Resource policy bounding retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::Report`] with stable code
    /// `"contract/version-missing"` or `"contract/version-unsupported"`.
    pub fn resolve_strict(
        &self,
        found: Option<u32>,
        pointer: &str,
        limits: &LoadLimits,
    ) -> Result<u32, ContractError> {
        self.resolve(found)
            .map_err(|error| self.diagnostic_error(error, pointer, limits))
    }

    /// Parse a required schema marker and return structured contract errors.
    ///
    /// # Arguments
    ///
    /// * `found` - Exact schema marker from the payload, or `None` when the
    ///   schema key was absent.
    /// * `pointer` - JSON Pointer locating the schema marker.
    /// * `limits` - Resource policy bounding retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::Report`] with stable code
    /// `"contract/version-missing"`, `"contract/version-unsupported"`, or
    /// `"contract/schema-malformed"`.
    pub fn parse_schema_strict(
        &self,
        found: Option<&str>,
        pointer: &str,
        limits: &LoadLimits,
    ) -> Result<u32, ContractError> {
        found
            .map_or_else(|| self.resolve(None), |schema| self.parse_schema(schema))
            .map_err(|error| self.diagnostic_error(error, pointer, limits))
    }

    /// Decode a persisted document whose root `schema` key names this contract.
    ///
    /// The bytes are parsed under `limits`, the root `/schema` string is
    /// checked with [`Self::parse_schema_strict`], and the whole document is
    /// then deserialized into `T`. Domain validation of `T` is left to the
    /// caller; report its failure with [`Self::semantic_error`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON document carrying a root `schema`
    ///   marker in the exact `"<contract>/<version>"` format.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::LimitExceeded`] for byte or depth overflows,
    /// and [`ContractError::Report`] for malformed JSON, a non-string marker
    /// (`"contract/structure-invalid"`), a missing, malformed, or unsupported
    /// marker, or a document that does not match `T`.
    pub fn decode<T>(&self, bytes: &[u8], limits: &LoadLimits) -> Result<T, ContractError>
    where
        T: DeserializeOwned,
    {
        let value = parse_json_value(bytes, limits)?;
        let schema = match value.get("schema") {
            Some(schema) => Some(deserialize_json_value::<String>(schema.clone(), limits)?),
            None => None,
        };
        self.parse_schema_strict(schema.as_deref(), "/schema", limits)?;
        deserialize_json_value(value, limits)
    }

    /// Decode a persisted document whose root `schema_version` key carries
    /// this contract's numeric version.
    ///
    /// The bytes are parsed under `limits`, the root `/schema_version`
    /// integer is checked with [`Self::resolve_strict`], and the whole
    /// document is then deserialized into `T`. Domain validation of `T` is
    /// left to the caller; report its failure with [`Self::semantic_error`].
    ///
    /// # Arguments
    ///
    /// * `bytes` - Complete UTF-8 JSON document carrying a root
    ///   `schema_version` integer.
    /// * `limits` - Resource policy bounding input size, JSON depth, and
    ///   retained diagnostics.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::LimitExceeded`] for byte or depth overflows,
    /// and [`ContractError::Report`] for malformed JSON, a non-integer version
    /// (pointed at `/schema_version`), a missing or unsupported version, or a
    /// document that does not match `T`.
    pub fn decode_versioned<T>(&self, bytes: &[u8], limits: &LoadLimits) -> Result<T, ContractError>
    where
        T: DeserializeOwned,
    {
        let value = parse_json_value(bytes, limits)?;
        let version = match value.get("schema_version") {
            Some(version) => Some(
                deserialize_json_value::<u32>(version.clone(), limits).map_err(
                    |error| match error {
                        ContractError::Report(mut report) => {
                            for diagnostic in &mut report.diagnostics {
                                if diagnostic.pointer.is_none() {
                                    diagnostic.pointer = Some("/schema_version".to_string());
                                }
                            }
                            ContractError::Report(report)
                        }
                        other => other,
                    },
                )?,
            ),
            None => None,
        };
        self.resolve_strict(version, "/schema_version", limits)?;
        deserialize_json_value(value, limits)
    }

    /// Report a failed domain validation of a decoded artifact.
    ///
    /// # Arguments
    ///
    /// * `error` - Domain validation failure whose display text becomes the
    ///   diagnostic message.
    /// * `limits` - Resource policy bounding retained diagnostics.
    ///
    /// # Returns
    ///
    /// [`ContractError::Report`] holding one `"contract/semantic-invalid"`
    /// error in the [`LoadPhase::Semantic`] phase, tagged with this contract.
    #[must_use]
    pub fn semantic_error(&self, error: impl Display, limits: &LoadLimits) -> ContractError {
        let mut report = ValidationReport::default();
        report.push_bounded(
            limits,
            Diagnostic::new(
                "contract/semantic-invalid",
                LoadPhase::Semantic,
                Severity::Error,
                error.to_string(),
            )
            .with_contract(self.id),
        );
        ContractError::Report(Box::new(report))
    }

    fn validate_version(&self, version: u32) -> Result<u32, ContractError> {
        if version == Self::VERSION {
            return Ok(version);
        }

        Err(ContractError::UnsupportedVersion {
            contract: self.id.to_string(),
            found: version,
            min: Self::VERSION,
            max: Self::VERSION,
        })
    }

    fn diagnostic_error(
        &self,
        error: ContractError,
        pointer: &str,
        limits: &LoadLimits,
    ) -> ContractError {
        let diagnostic = match &error {
            ContractError::MissingVersion { .. } => Diagnostic::new(
                "contract/version-missing",
                LoadPhase::Version,
                Severity::Error,
                error.to_string(),
            )
            .with_pointer(pointer)
            .with_contract(self.id)
            .with_expected_version(Self::VERSION),
            ContractError::UnsupportedVersion { found, .. } => Diagnostic::new(
                "contract/version-unsupported",
                LoadPhase::Version,
                Severity::Error,
                error.to_string(),
            )
            .with_pointer(pointer)
            .with_contract(self.id)
            .with_expected_version(Self::VERSION)
            .with_actual_version(*found),
            ContractError::MalformedSchema { .. } => Diagnostic::new(
                "contract/schema-malformed",
                LoadPhase::Version,
                Severity::Error,
                error.to_string(),
            )
            .with_pointer(pointer)
            .with_contract(self.id)
            .with_expected_version(Self::VERSION),
            _ => return error,
        };
        let mut report = ValidationReport::default();
        report.push_bounded(limits, diagnostic);
        ContractError::Report(Box::new(report))
    }
}
