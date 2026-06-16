//! Model parameter snapshots for instrument-level attribution.

use crate::instruments::fixed_income::convertible::ConversionSpec;
use finstack_quant_cashflows::builder::{DefaultModelSpec, PrepaymentModelSpec, RecoveryModelSpec};

/// Snapshot of extractable model parameters from an instrument.
///
/// Different instrument types have different model parameters that affect
/// pricing. This enum captures the relevant parameters for each type.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub enum ModelParamsSnapshot {
    /// Structured credit parameters (prepayment, default, recovery).
    StructuredCredit {
        /// Prepayment model specification.
        prepayment_spec: PrepaymentModelSpec,
        /// Default model specification.
        default_spec: DefaultModelSpec,
        /// Recovery model specification.
        recovery_spec: RecoveryModelSpec,
    },

    /// Convertible bond parameters (conversion ratio, policies).
    Convertible {
        /// Conversion specification for convertible bonds.
        conversion_spec: ConversionSpec,
    },

    /// No extractable model parameters.
    None,
}
