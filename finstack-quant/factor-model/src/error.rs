use super::{FactorId, MarketDependency};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Errors produced by factor-model workflows.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum FactorModelError {
    /// No factor matched a dependency for a position.
    #[error("No factor matched dependency {dependency:?} for position '{position_id}'")]
    UnmatchedDependency {
        /// Position identifier.
        position_id: String,
        /// Dependency that could not be matched.
        dependency: MarketDependency,
    },
    /// Covariance or loadings referenced a factor that was not supplied.
    #[error("Factor '{factor_id}' referenced but not found")]
    MissingFactor {
        /// Missing factor identifier.
        factor_id: FactorId,
    },
    /// Covariance matrix failed validation.
    #[error("Invalid covariance matrix: {reason}")]
    InvalidCovariance {
        /// Reason the covariance matrix is invalid.
        reason: String,
    },
    /// Repricing under a factor move failed.
    #[error("Repricing failed for position '{position_id}' under factor '{factor_id}': {reason}")]
    RepricingFailed {
        /// Position identifier.
        position_id: String,
        /// Factor that triggered repricing.
        factor_id: FactorId,
        /// Underlying source error message.
        reason: String,
    },
    /// Multiple factors matched where only one was allowed.
    #[error("Ambiguous factor match for position '{position_id}': {candidates:?}")]
    AmbiguousMatch {
        /// Position identifier.
        position_id: String,
        /// Candidate factor identifiers.
        candidates: Vec<FactorId>,
    },
}

/// Policy for handling dependencies that do not match any factor.
///
/// Serializes in `snake_case` (matching the crate-wide convention and this
/// type's own `Display`/`FromStr`); the legacy PascalCase wire forms are
/// still accepted on input via serde aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UnmatchedPolicy {
    /// Fail immediately when any dependency is unmatched.
    ///
    /// Use this in production risk runs where dropping unmapped risk would be a
    /// control failure.
    #[serde(alias = "Strict")]
    Strict,
    /// Roll unmatched risk into a residual bucket.
    ///
    /// Use this when the engine should preserve total exposure while making the
    /// unmatched component explicit as residual risk.
    #[default]
    #[serde(alias = "Residual")]
    Residual,
    /// Continue but surface a warning to the caller.
    ///
    /// Suitable for exploratory workflows where visibility matters but a hard
    /// failure would be too disruptive.
    #[serde(alias = "Warn")]
    Warn,
}

impl fmt::Display for UnmatchedPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Strict => write!(f, "strict"),
            Self::Residual => write!(f, "residual"),
            Self::Warn => write!(f, "warn"),
        }
    }
}

impl crate::parse::NormalizedEnum for UnmatchedPolicy {
    const VARIANTS: &'static [(&'static str, Self)] = &[
        ("strict", Self::Strict),
        ("error", Self::Strict),
        ("residual", Self::Residual),
        ("warn", Self::Warn),
        ("ignore", Self::Warn),
    ];
}

impl FromStr for UnmatchedPolicy {
    type Err = finstack_quant_core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
            .map_err(|e| finstack_quant_core::Error::Validation(format!("UnmatchedPolicy: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_parses_to(label: &str, expected: UnmatchedPolicy) {
        assert!(matches!(label.parse::<UnmatchedPolicy>(), Ok(value) if value == expected));
    }

    #[test]
    fn test_error_display_missing_factor() {
        let error = FactorModelError::MissingFactor {
            factor_id: FactorId::new("USD-Rates"),
        };
        let message = format!("{error}");
        assert!(message.contains("USD-Rates"));
    }

    #[test]
    fn test_unmatched_policy_default() {
        assert_eq!(UnmatchedPolicy::default(), UnmatchedPolicy::Residual);
    }

    #[test]
    fn test_unmatched_policy_serde() {
        let policy = UnmatchedPolicy::Strict;
        let json_result = serde_json::to_string(&policy);
        assert!(json_result.is_ok());
        let Ok(json) = json_result else {
            return;
        };

        let back_result: Result<UnmatchedPolicy, _> = serde_json::from_str(&json);
        assert!(back_result.is_ok());
        let Ok(back) = back_result else {
            return;
        };
        assert_eq!(policy, back);
    }

    #[test]
    fn test_unmatched_policy_fromstr_display_roundtrip() {
        for (input, expected) in [
            ("strict", UnmatchedPolicy::Strict),
            ("error", UnmatchedPolicy::Strict),
            ("residual", UnmatchedPolicy::Residual),
            ("warn", UnmatchedPolicy::Warn),
            ("ignore", UnmatchedPolicy::Warn),
        ] {
            assert_parses_to(input, expected);
        }

        for variant in [
            UnmatchedPolicy::Strict,
            UnmatchedPolicy::Residual,
            UnmatchedPolicy::Warn,
        ] {
            let display = variant.to_string();
            assert!(matches!(display.parse::<UnmatchedPolicy>(), Ok(value) if value == variant));
        }
    }

    #[test]
    fn test_unmatched_policy_fromstr_rejects_unknown() {
        assert!("unknown".parse::<UnmatchedPolicy>().is_err());
    }

    #[test]
    fn unmatched_policy_serializes_snake_case_and_reads_legacy_pascal_case() {
        let json = serde_json::to_string(&UnmatchedPolicy::Strict).unwrap_or_default();
        assert_eq!(json, "\"strict\"");
        // Legacy PascalCase wire form still accepted on input.
        let legacy: Result<UnmatchedPolicy, _> = serde_json::from_str("\"Residual\"");
        assert!(matches!(legacy, Ok(UnmatchedPolicy::Residual)));
    }
}
