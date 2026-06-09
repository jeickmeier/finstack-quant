use super::{FactorId, MarketDependency};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Errors produced by factor-model workflows.
#[derive(Debug)]
pub enum FactorModelError {
    /// No factor matched a dependency for a position.
    UnmatchedDependency {
        /// Position identifier.
        position_id: String,
        /// Dependency that could not be matched.
        dependency: MarketDependency,
    },
    /// Covariance or loadings referenced a factor that was not supplied.
    MissingFactor {
        /// Missing factor identifier.
        factor_id: FactorId,
    },
    /// Covariance matrix failed validation.
    InvalidCovariance {
        /// Reason the covariance matrix is invalid.
        reason: String,
    },
    /// Repricing under a factor move failed.
    RepricingFailed {
        /// Position identifier.
        position_id: String,
        /// Factor that triggered repricing.
        factor_id: FactorId,
        /// Underlying source error.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Multiple factors matched where only one was allowed.
    AmbiguousMatch {
        /// Position identifier.
        position_id: String,
        /// Candidate factor identifiers.
        candidates: Vec<FactorId>,
    },
}

impl fmt::Display for FactorModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnmatchedDependency {
                position_id,
                dependency,
            } => write!(
                f,
                "No factor matched dependency {dependency:?} for position '{position_id}'"
            ),
            Self::MissingFactor { factor_id } => {
                write!(f, "Factor '{factor_id}' referenced but not found")
            }
            Self::InvalidCovariance { reason } => {
                write!(f, "Invalid covariance matrix: {reason}")
            }
            Self::RepricingFailed {
                position_id,
                factor_id,
                source,
            } => write!(
                f,
                "Repricing failed for position '{position_id}' under factor '{factor_id}': {source}"
            ),
            Self::AmbiguousMatch {
                position_id,
                candidates,
            } => write!(
                f,
                "Ambiguous factor match for position '{position_id}': {candidates:?}"
            ),
        }
    }
}

impl std::error::Error for FactorModelError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RepricingFailed { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Policy for handling dependencies that do not match any factor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum UnmatchedPolicy {
    /// Fail immediately when any dependency is unmatched.
    ///
    /// Use this in production risk runs where dropping unmapped risk would be a
    /// control failure.
    Strict,
    /// Roll unmatched risk into a residual bucket.
    ///
    /// Use this when the engine should preserve total exposure while making the
    /// unmatched component explicit as residual risk.
    #[default]
    Residual,
    /// Continue but surface a warning to the caller.
    ///
    /// Suitable for exploratory workflows where visibility matters but a hard
    /// failure would be too disruptive.
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
    type Err = finstack_core::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        crate::parse::parse_normalized_enum(s)
            .map_err(|_| finstack_core::InputError::Invalid.into())
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
}
