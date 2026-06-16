//! Validation framework for waterfall specifications.
//!
//! This module provides validation for waterfall specifications,
//! ensuring correctness before execution. It checks for:
//! - Duplicate tier/recipient IDs
//! - Invalid priority values
//! - Empty/impossible tier configurations
//! - Missing test references in diversion rules
//! - Circular diversion dependencies

use crate::instruments::fixed_income::structured_credit::pricing::diversion::{
    DiversionCondition, DiversionEngine, DiversionRule,
};
use crate::instruments::fixed_income::structured_credit::types::{
    AllocationMode, PaymentType, Waterfall, WaterfallTier,
};
use finstack_quant_core::HashSet;
use finstack_quant_core::Result;

// ============================================================================
// VALIDATION ERRORS
// ============================================================================

/// Validation error details.
#[derive(
    Debug,
    Clone,
    PartialEq,
    thiserror::Error,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[non_exhaustive]
pub enum ValidationError {
    /// Duplicate tier ID.
    #[error("Duplicate tier ID: {tier_id}")]
    DuplicateTierId {
        /// Tier id.
        tier_id: String,
    },
    /// Duplicate recipient ID within a tier.
    #[error("Duplicate recipient ID '{recipient_id}' in tier '{tier_id}'")]
    DuplicateRecipientId {
        /// Tier id.
        tier_id: String,
        /// Recipient id.
        recipient_id: String,
    },
    /// Invalid priority (must be > 0).
    #[error("Invalid priority {priority} for tier '{tier_id}' (must be > 0)")]
    InvalidPriority {
        /// Tier id.
        tier_id: String,
        /// Priority.
        priority: usize,
    },
    /// Tier has no recipients.
    #[error("Tier '{tier_id}' has no recipients")]
    EmptyTier {
        /// Tier id.
        tier_id: String,
    },
    /// Missing coverage test reference.
    #[error("Diversion rule '{rule_id}' references missing test '{test_id}'")]
    MissingTestReference {
        /// Test id.
        test_id: String,
        /// Rule id.
        rule_id: String,
    },
    /// Invalid recipient weight (must be >= 0).
    #[error(
        "Invalid weight {weight} for recipient '{recipient_id}' in tier '{tier_id}' (must be >= 0)"
    )]
    InvalidWeight {
        /// Tier id.
        tier_id: String,
        /// Recipient id.
        recipient_id: String,
        /// Weight.
        weight: f64,
    },
    /// Pro-rata tier with invalid total weight.
    #[error("Pro-rata tier '{tier_id}' has invalid total weight {total_weight} (must be > 0)")]
    InvalidProRataWeights {
        /// Tier id.
        tier_id: String,
        /// Total weight.
        total_weight: f64,
    },
    /// Circular diversion reference.
    #[error("Circular diversion detected: {cycle_path}")]
    CircularDiversion {
        /// Cycle path.
        cycle_path: String,
    },
    /// Diversion references non-existent tier.
    #[error("Diversion rule '{rule_id}' references non-existent tier '{tier_id}'")]
    InvalidDiversionTier {
        /// Rule id.
        rule_id: String,
        /// Tier id.
        tier_id: String,
    },
}

// ============================================================================
// WATERFALL VALIDATOR TRAIT
// ============================================================================

/// Trait for validating waterfall specifications.
pub(crate) trait WaterfallValidator {
    /// Validate the waterfall specification.
    ///
    /// Returns Ok(()) if valid, or Err with validation errors.
    fn validate(&self) -> Result<()>;
}

// ============================================================================
// WATERFALL SPEC VALIDATION
// ============================================================================

/// Waterfall specification for validation.
///
/// This is a simplified representation that includes just the fields needed
/// for validation (tiers, diversion rules, coverage tests).
pub(crate) struct WaterfallSpec {
    /// Tiers.
    pub(crate) tiers: Vec<WaterfallTier>,
    /// Diversion rules.
    pub(crate) diversion_rules: Vec<DiversionRule>,
    /// Coverage test IDs.
    pub(crate) coverage_test_ids: Vec<String>,
}

impl WaterfallSpec {
    /// Create a new waterfall spec.
    pub(crate) fn new(
        tiers: Vec<WaterfallTier>,
        diversion_rules: Vec<DiversionRule>,
        coverage_test_ids: Vec<String>,
    ) -> Self {
        Self {
            tiers,
            diversion_rules,
            coverage_test_ids,
        }
    }
}

impl WaterfallValidator for WaterfallSpec {
    fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        errors.extend(validate_tiers(&self.tiers));
        errors.extend(validate_diversion_rules(
            &self.diversion_rules,
            &self.tiers,
            &self.coverage_test_ids,
        ));

        if !errors.is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Waterfall validation failed with {} error(s): {}",
                errors.len(),
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }

        Ok(())
    }
}

impl WaterfallValidator for Waterfall {
    fn validate(&self) -> Result<()> {
        let errors = validate_tiers(&self.tiers);
        if !errors.is_empty() {
            return Err(finstack_quant_core::Error::Validation(format!(
                "Waterfall validation failed with {} error(s): {}",
                errors.len(),
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            )));
        }
        Ok(())
    }
}

/// Validate tier specifications.
fn validate_tiers(tiers: &[WaterfallTier]) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let mut seen_tier_ids = HashSet::default();
    for tier in tiers {
        if !seen_tier_ids.insert(&tier.id) {
            errors.push(ValidationError::DuplicateTierId {
                tier_id: tier.id.clone(),
            });
        }

        // Check for empty tiers (except residual tiers which can have ResidualCash recipient)
        if tier.recipients.is_empty() && tier.payment_type != PaymentType::Residual {
            errors.push(ValidationError::EmptyTier {
                tier_id: tier.id.clone(),
            });
        }

        // Validate recipient IDs within tier
        let mut seen_recipient_ids = HashSet::default();
        for recipient in &tier.recipients {
            if !seen_recipient_ids.insert(&recipient.id) {
                errors.push(ValidationError::DuplicateRecipientId {
                    tier_id: tier.id.clone(),
                    recipient_id: recipient.id.clone(),
                });
            }

            // Check recipient weights
            if let Some(weight) = recipient.weight {
                if weight < 0.0 {
                    errors.push(ValidationError::InvalidWeight {
                        tier_id: tier.id.clone(),
                        recipient_id: recipient.id.clone(),
                        weight,
                    });
                }
            }
        }

        // For pro-rata tiers, validate total weight
        if tier.allocation_mode == AllocationMode::ProRata {
            let total_weight: f64 = tier
                .recipients
                .iter()
                .map(|r| r.weight.unwrap_or(1.0))
                .sum();

            if total_weight <= 0.0 {
                errors.push(ValidationError::InvalidProRataWeights {
                    tier_id: tier.id.clone(),
                    total_weight,
                });
            }
        }
    }

    errors
}

/// Validate diversion rules.
fn validate_diversion_rules(
    rules: &[DiversionRule],
    tiers: &[WaterfallTier],
    coverage_test_ids: &[String],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let tier_ids: HashSet<_> = tiers.iter().map(|t| t.id.as_str()).collect();
    let test_ids: HashSet<_> = coverage_test_ids.iter().map(|s| s.as_str()).collect();

    for rule in rules {
        // Check that source and target tiers exist
        if !tier_ids.contains(rule.source_tier_id.as_str()) {
            errors.push(ValidationError::InvalidDiversionTier {
                rule_id: rule.id.clone(),
                tier_id: rule.source_tier_id.clone(),
            });
        }

        if !tier_ids.contains(rule.target_tier_id.as_str()) {
            errors.push(ValidationError::InvalidDiversionTier {
                rule_id: rule.id.clone(),
                tier_id: rule.target_tier_id.clone(),
            });
        }

        // Check that coverage test references are valid
        if let DiversionCondition::CoverageTestFailed { test_id } = &rule.condition {
            if !test_ids.contains(test_id.as_str()) {
                errors.push(ValidationError::MissingTestReference {
                    test_id: test_id.clone(),
                    rule_id: rule.id.clone(),
                });
            }
        }
    }

    // Check for circular dependencies using DiversionEngine
    let diversion_engine = rules.iter().fold(DiversionEngine::new(), |engine, rule| {
        engine.add_rule(rule.clone())
    });

    if let Err(e) = diversion_engine.validate() {
        let err_msg = e.to_string();
        if err_msg.contains("Circular diversion") {
            errors.push(ValidationError::CircularDiversion {
                cycle_path: err_msg,
            });
        }
    }

    errors
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Quick validation helper that returns true if spec is valid.
pub fn is_valid_waterfall_spec(
    tiers: &[WaterfallTier],
    diversion_rules: &[DiversionRule],
    coverage_test_ids: &[String],
) -> bool {
    let spec = WaterfallSpec::new(
        tiers.to_vec(),
        diversion_rules.to_vec(),
        coverage_test_ids.to_vec(),
    );
    spec.validate().is_ok()
}

/// Get validation errors as a list.
pub fn get_validation_errors(
    tiers: &[WaterfallTier],
    diversion_rules: &[DiversionRule],
    coverage_test_ids: &[String],
) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    errors.extend(validate_tiers(tiers));
    errors.extend(validate_diversion_rules(
        diversion_rules,
        tiers,
        coverage_test_ids,
    ));
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::fixed_income::structured_credit::types::{
        PaymentCalculation, Recipient, RecipientType,
    };
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::money::Money;

    fn create_valid_tier(id: &str, priority: usize) -> WaterfallTier {
        WaterfallTier::new(id, priority, PaymentType::Fee).add_recipient(Recipient::new(
            "recipient1",
            RecipientType::ServiceProvider("Trustee".into()),
            PaymentCalculation::FixedAmount {
                amount: Money::new(1000.0, Currency::USD),
                rounding: None,
            },
        ))
    }

    #[test]
    fn test_valid_waterfall_spec() {
        let tiers = vec![create_valid_tier("tier1", 1), create_valid_tier("tier2", 2)];
        let rules = vec![];
        let tests = vec![];

        let spec = WaterfallSpec::new(tiers, rules, tests);
        assert!(spec.validate().is_ok());
    }

    #[test]
    fn test_duplicate_tier_id() {
        let tiers = vec![create_valid_tier("tier1", 1), create_valid_tier("tier1", 2)];

        let errors = validate_tiers(&tiers);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::DuplicateTierId { .. }));
    }

    #[test]
    fn test_empty_tier() {
        let empty_tier = WaterfallTier::new("empty", 1, PaymentType::Fee);
        let tiers = vec![empty_tier];

        let errors = validate_tiers(&tiers);
        assert_eq!(errors.len(), 1);
        assert!(matches!(errors[0], ValidationError::EmptyTier { .. }));
    }
}
