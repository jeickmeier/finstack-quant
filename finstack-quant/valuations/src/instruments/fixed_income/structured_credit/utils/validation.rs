//! Validation framework for waterfall specifications.
//!
//! This module provides validation for waterfall specifications,
//! ensuring correctness before execution. It checks for:
//! - Duplicate tier/recipient IDs
//! - Invalid priority values
//! - Empty/impossible tier configurations

use crate::instruments::fixed_income::structured_credit::types::{
    AllocationMode, PaymentCalculation, PaymentType, Waterfall, WaterfallTier,
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
/// for validation (tiers, coverage tests).
pub(crate) struct WaterfallSpec {
    /// Tiers.
    pub(crate) tiers: Vec<WaterfallTier>,
}

impl WaterfallSpec {
    /// Create a new waterfall spec.
    pub(crate) fn new(tiers: Vec<WaterfallTier>) -> Self {
        Self { tiers }
    }
}

impl WaterfallValidator for WaterfallSpec {
    fn validate(&self) -> Result<()> {
        let mut errors = Vec::new();

        errors.extend(validate_tiers(&self.tiers));

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

            // Check recipient weights.
            //
            // SC-m10: `weight < 0.0` is FALSE for NaN, so a NaN weight passed
            // validation and reached `allocate_pro_rata`, where it poisons the
            // weight total and every share derived from it. `Money::new` then
            // panics on the non-finite result (core/src/money/types.rs), so a
            // malformed weight took down the pricing call rather than being
            // reported. Testing for non-finite explicitly closes that.
            if let Some(weight) = recipient.weight {
                if !weight.is_finite() || weight < 0.0 {
                    errors.push(ValidationError::InvalidWeight {
                        tier_id: tier.id.clone(),
                        recipient_id: recipient.id.clone(),
                        weight,
                    });
                }
            }
            // SC-m10: a non-finite payment PARAMETER reaches `Money::new` in
            // `calculate_payment_amount` and panics there. Validation is the
            // right place to reject it, with the tier and recipient named.
            let bad_amount = match &recipient.calculation {
                PaymentCalculation::FixedAmount { amount, .. } => !amount.amount().is_finite(),
                PaymentCalculation::PercentageOfCollateral { rate, .. } => !rate.is_finite(),
                PaymentCalculation::CappedTrancheInterest { cap_rate, .. } => !cap_rate.is_finite(),
                PaymentCalculation::ReserveReplenishment { target_balance } => {
                    !target_balance.amount().is_finite()
                }
                PaymentCalculation::TranchePrincipal { target_balance, .. } => target_balance
                    .as_ref()
                    .is_some_and(|b| !b.amount().is_finite()),
                _ => false,
            };
            if bad_amount {
                errors.push(ValidationError::InvalidWeight {
                    tier_id: tier.id.clone(),
                    recipient_id: recipient.id.clone(),
                    weight: f64::NAN,
                });
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

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Quick validation helper that returns true if spec is valid.
///
/// # Arguments
///
/// * `tiers` - Ordered waterfall allocation tiers to validate for references,
///   ordering, and allocation invariants.
///
/// SC-m20: this previously also took `diversion_rules` and `coverage_test_ids`.
/// Both existed only to validate the `DiversionEngine` rule graph, a
/// declarative diversion mechanism that was never wired to waterfall
/// execution — the live path uses `tier.divertible` plus the coverage tests.
/// The engine and its validation are removed; what remains validates the tiers
/// that actually govern allocation.
pub fn is_valid_waterfall_spec(tiers: &[WaterfallTier]) -> bool {
    let spec = WaterfallSpec::new(tiers.to_vec());
    spec.validate().is_ok()
}

/// Get validation errors as a list.
///
/// # Arguments
///
/// * `tiers` - Ordered waterfall allocation tiers to validate for references,
///   ordering, and allocation invariants.
///
/// SC-m20: this previously also took `diversion_rules` and `coverage_test_ids`.
/// Both existed only to validate the `DiversionEngine` rule graph, a
/// declarative diversion mechanism that was never wired to waterfall
/// execution — the live path uses `tier.divertible` plus the coverage tests.
/// The engine and its validation are removed; what remains validates the tiers
/// that actually govern allocation.
pub fn get_validation_errors(tiers: &[WaterfallTier]) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    errors.extend(validate_tiers(tiers));
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

        let spec = WaterfallSpec::new(tiers);
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
