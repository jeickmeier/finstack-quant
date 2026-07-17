//! Problem, decision-space, constraint, and solver types for portfolio optimization.
//!
use super::types::MetricExpr;
use super::universe::PositionFilter;
use serde::{Deserialize, Serialize};

/// Inequality/equality operator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Inequality {
    /// Less‑than or equal: `lhs <= rhs`.
    Le,
    /// Greater‑than or equal: `lhs >= rhs`.
    Ge,
    /// Equality: `lhs == rhs`.
    Eq,
}

/// Declarative constraint specification.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Constraint {
    /// General metric bound, e.g. duration `<= 4.0`.
    MetricBound {
        /// Human‑readable label for debugging/diagnostics.
        label: Option<String>,
        /// Metric expression on the left‑hand side.
        metric: MetricExpr,
        /// Operator (<=, >=, ==).
        op: Inequality,
        /// Right‑hand side constant.
        rhs: f64,
    },

    /// Weight bounds for all positions matching the filter.
    WeightBounds {
        /// Human‑readable label for debugging/diagnostics.
        label: Option<String>,
        /// Filter to select positions for this constraint.
        filter: PositionFilter,
        /// Inclusive minimum weight.
        min: f64,
        /// Inclusive maximum weight.
        max: f64,
    },

    /// Maximum turnover constraint: `Σ |w_new - w_current| <= max_turnover`.
    MaxTurnover {
        /// Human‑readable label for debugging/diagnostics.
        label: Option<String>,
        /// Maximum allowed turnover (sum of absolute weight changes).
        max_turnover: f64,
    },

    /// Budget/normalization constraint: usually `∑ w_i == 1.0`.
    Budget {
        /// Right‑hand side constant (typically 1.0 for normalization).
        rhs: f64,
    },
}

/// Error returned when constraint parameters are invalid.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintValidationError {
    /// Description of the validation failure.
    pub message: String,
}

impl std::fmt::Display for ConstraintValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Constraint validation error: {}", self.message)
    }
}

impl std::error::Error for ConstraintValidationError {}

impl Constraint {
    /// Get the constraint label (for diagnostics).
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::MetricBound { label, .. }
            | Self::WeightBounds { label, .. }
            | Self::MaxTurnover { label, .. } => label.as_deref(),
            Self::Budget { .. } => Some("budget"),
        }
    }

    /// Attach a diagnostic label to this constraint.
    ///
    /// Chain after a constructor, e.g.
    /// `Constraint::exposure_limit("rating", "CCC", 0.2)?.with_label("ccc_limit")`.
    ///
    /// No-op for [`Constraint::Budget`], which always reports the fixed label
    /// `"budget"` via [`Constraint::label`].
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let label_str = label.into();
        match &mut self {
            Self::MetricBound { label, .. }
            | Self::WeightBounds { label, .. }
            | Self::MaxTurnover { label, .. } => {
                *label = Some(label_str);
            }
            Self::Budget { .. } => {}
        }
        self
    }

    /// Shorthand for attribute exposure limit: `sum w_i * I[attr == value] <= max_share`.
    ///
    /// # Errors
    ///
    /// Returns [`ConstraintValidationError`] if `max_share` is not in `[0, 1]`.
    pub fn exposure_limit(
        key: impl Into<String>,
        value: impl Into<String>,
        max_share: f64,
    ) -> Result<Self, ConstraintValidationError> {
        if !(0.0..=1.0).contains(&max_share) {
            return Err(ConstraintValidationError {
                message: format!("max_share must be in [0, 1], got {max_share}"),
            });
        }
        Ok(Self::MetricBound {
            label: None,
            metric: super::types::MetricExpr::WeightedSum {
                metric: super::types::PerPositionMetric::AttributeIndicator(
                    crate::types::AttributeTest::text_eq(key, value),
                ),
                filter: None,
            },
            op: Inequality::Le,
            rhs: max_share,
        })
    }

    /// Shorthand for attribute exposure minimum: `sum w_i * I[attr == value] >= min_share`.
    ///
    /// # Errors
    ///
    /// Returns [`ConstraintValidationError`] if `min_share` is not in `[0, 1]`.
    pub fn exposure_minimum(
        key: impl Into<String>,
        value: impl Into<String>,
        min_share: f64,
    ) -> Result<Self, ConstraintValidationError> {
        if !(0.0..=1.0).contains(&min_share) {
            return Err(ConstraintValidationError {
                message: format!("min_share must be in [0, 1], got {min_share}"),
            });
        }
        Ok(Self::MetricBound {
            label: None,
            metric: super::types::MetricExpr::WeightedSum {
                metric: super::types::PerPositionMetric::AttributeIndicator(
                    crate::types::AttributeTest::text_eq(key, value),
                ),
                filter: None,
            },
            op: Inequality::Ge,
            rhs: min_share,
        })
    }

    /// Create a weight bounds constraint with validation.
    ///
    /// # Arguments
    ///
    /// * `filter` - Filter to select positions
    /// * `min` - Inclusive minimum weight
    /// * `max` - Inclusive maximum weight
    ///
    /// # Errors
    ///
    /// Returns [`ConstraintValidationError`] if `min > max`.
    pub fn weight_bounds(
        filter: PositionFilter,
        min: f64,
        max: f64,
    ) -> Result<Self, ConstraintValidationError> {
        if min > max {
            return Err(ConstraintValidationError {
                message: format!("weight bounds min ({}) must be <= max ({})", min, max),
            });
        }
        Ok(Self::WeightBounds {
            label: None,
            filter,
            min,
            max,
        })
    }

    /// Create a max turnover constraint with validation.
    ///
    /// # Arguments
    ///
    /// * `max_turnover` - Maximum allowed turnover (must be non-negative)
    ///
    /// # Errors
    ///
    /// Returns [`ConstraintValidationError`] if `max_turnover` is negative.
    pub fn max_turnover(max_turnover: f64) -> Result<Self, ConstraintValidationError> {
        if max_turnover < 0.0 {
            return Err(ConstraintValidationError {
                message: format!("max_turnover must be non-negative, got {}", max_turnover),
            });
        }
        Ok(Self::MaxTurnover {
            label: None,
            max_turnover,
        })
    }

    /// Create a budget (normalization) constraint with validation.
    ///
    /// # Arguments
    ///
    /// * `rhs` - Weight sum target (typically 1.0 for fully-invested portfolios).
    ///
    /// # Errors
    ///
    /// Returns [`ConstraintValidationError`] if `rhs` is NaN, infinite, or negative.
    pub fn budget(rhs: f64) -> Result<Self, ConstraintValidationError> {
        if !rhs.is_finite() || rhs < 0.0 {
            return Err(ConstraintValidationError {
                message: format!("budget rhs must be finite and non-negative, got {rhs}"),
            });
        }
        Ok(Self::Budget { rhs })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exposure_limit_validation() {
        // Valid: 0.0
        assert!(Constraint::exposure_limit("rating", "CCC", 0.0).is_ok());

        // Valid: 1.0
        assert!(Constraint::exposure_limit("rating", "CCC", 1.0).is_ok());

        // Valid: 0.5
        assert!(Constraint::exposure_limit("rating", "CCC", 0.5).is_ok());

        // Invalid: negative
        let result = Constraint::exposure_limit("rating", "CCC", -0.1);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("max_share"));

        // Invalid: > 1.0
        let result = Constraint::exposure_limit("rating", "CCC", 1.5);
        assert!(result.is_err());
    }

    #[test]
    fn test_exposure_minimum_validation() {
        // Valid
        assert!(Constraint::exposure_minimum("rating", "IG", 0.5).is_ok());

        // Invalid: negative
        let result = Constraint::exposure_minimum("rating", "IG", -0.1);
        assert!(result.is_err());

        // Invalid: > 1.0
        let result = Constraint::exposure_minimum("rating", "IG", 1.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_weight_bounds_validation() {
        // Valid: min < max
        assert!(Constraint::weight_bounds(PositionFilter::All, 0.0, 0.1).is_ok());

        // Valid: min == max
        assert!(Constraint::weight_bounds(PositionFilter::All, 0.05, 0.05).is_ok());

        // Invalid: min > max
        let result = Constraint::weight_bounds(PositionFilter::All, 0.2, 0.1);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("min"));
    }

    #[test]
    fn test_max_turnover_validation() {
        // Valid
        assert!(Constraint::max_turnover(0.5).is_ok());
        assert!(Constraint::max_turnover(0.0).is_ok());

        // Invalid: negative
        let result = Constraint::max_turnover(-0.1);
        assert!(result.is_err());
    }
}
