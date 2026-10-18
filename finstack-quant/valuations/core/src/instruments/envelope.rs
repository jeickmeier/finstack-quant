//! The persisted instrument-envelope contract shared by every crate that
//! reads instrument JSON: the schema marker, the input-size cap and the
//! load-error mapping. The umbrella crate's `json_loader` owns the closed
//! `InstrumentJson` union; a family crate parses a single-variant envelope
//! of its own instrument with the same marker and limits.

use finstack_quant_core::contract::ContractError;
use serde::{Deserialize, Serialize};

/// Maximum permitted size of a JSON instrument definition, in bytes.
///
/// 16 MiB is far larger than any realistic single-instrument JSON payload
/// (the largest observed real-world instruments are well under 1 MiB), but
/// small enough to prevent unbounded allocations from malicious or
/// accidentally huge inputs.  The capped loaders enforce this limit before
/// handing bytes to the JSON parser, so a multi-gigabyte input can never
/// cause an OOM allocation.
pub const MAX_JSON_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Map bounded loading diagnostics into the ordinary pricing error surface.
///
/// # Arguments
///
/// * `error` - The contract-loader failure: a diagnostics report (each
///   diagnostic is rendered as `pointer: message`, joined with `; `) or a
///   resource-limit / parse error rendered through `Display`.
#[must_use]
pub fn instrument_load_error(error: ContractError) -> finstack_quant_core::Error {
    let message = match error {
        ContractError::Report(report) if !report.diagnostics.is_empty() => report
            .diagnostics
            .iter()
            .map(|diagnostic| match diagnostic.pointer.as_deref() {
                Some(pointer) if !pointer.is_empty() => {
                    format!("{pointer}: {}", diagnostic.message)
                }
                _ => diagnostic.message.clone(),
            })
            .collect::<Vec<_>>()
            .join("; "),
        error => error.to_string(),
    };
    finstack_quant_core::Error::Validation(format!("invalid instrument envelope JSON: {message}"))
}

/// Canonical schema marker for persisted instrument envelopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
pub enum InstrumentSchema {
    /// The sole supported instrument contract.
    #[serde(rename = "finstack_quant.instrument/1")]
    Instrument,
}

impl InstrumentSchema {
    /// The exact marker required by every persisted instrument envelope.
    pub const CURRENT: Self = Self::Instrument;

    /// Return the persisted schema marker.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instrument => "finstack_quant.instrument/1",
        }
    }
}

impl std::fmt::Display for InstrumentSchema {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}
