//! Validation framework for waterfall specifications.
//!
//! This module provides validation for waterfall specifications,
//! ensuring correctness before execution. It checks for:
//! - Duplicate tier/recipient IDs
//! - Invalid priority values
//! - Empty/impossible tier configurations

use crate::instruments::fixed_income::structured_credit::types::{
    AllocationMode, PaymentCalculation, PaymentType, WaterfallTier,
};
use finstack_quant_core::HashSet;

/// Validation error details.
#[derive(Debug, Clone, PartialEq, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
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

/// Structural validation of waterfall tiers: duplicate tier or recipient
/// ids, empty tiers, non-finite or negative weights and payment parameters,
/// and pro-rata tiers without positive total weight.
///
/// # Arguments
///
/// * `tiers` - Waterfall tiers in any order.
///
/// # Returns
///
/// Every violation found; empty when the tiers are valid.
pub fn validate_tiers(tiers: &[WaterfallTier]) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let mut seen_tier_ids = HashSet::default();
    for tier in tiers {
        if !seen_tier_ids.insert(&tier.id) {
            errors.push(ValidationError::DuplicateTierId {
                tier_id: tier.id.clone(),
            });
        }

        // Check for empty tiers (residual tiers may be empty; coverage-test
        // tiers carry tests instead of recipients).
        let is_test_tier = tier.payment_type == PaymentType::CoverageTest;
        if tier.recipients.is_empty() && !is_test_tier && tier.payment_type != PaymentType::Residual
        {
            errors.push(ValidationError::EmptyTier {
                tier_id: tier.id.clone(),
            });
        }
        if is_test_tier && tier.tests.is_empty() {
            errors.push(ValidationError::EmptyTier {
                tier_id: tier.id.clone(),
            });
        }
        for test in &tier.tests {
            if !test.trigger_level.is_finite() || test.trigger_level <= 0.0 {
                errors.push(ValidationError::InvalidWeight {
                    tier_id: tier.id.clone(),
                    recipient_id: test.id.clone(),
                    weight: test.trigger_level,
                });
            }
        }

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
            // `weight < 0.0` is FALSE for NaN, so a NaN weight passed
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
            // A non-finite payment PARAMETER reaches `Money::new` in
            // `calculate_payment_amount` and panics there. Validation is the
            // right place to reject it, with the tier and recipient named.
            let bad_amount = match &recipient.calculation {
                PaymentCalculation::FixedAmount { amount, .. }
                | PaymentCalculation::NetWacCarryover { amount, .. } => {
                    !amount.amount().is_finite()
                }
                PaymentCalculation::PercentageOfCollateral { rate, .. }
                | PaymentCalculation::PercentageOfSpecialServiced { rate, .. } => !rate.is_finite(),
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
                amount: Money::from((1000_i64, Currency::USD)),
                rounding: None,
            },
        ))
    }

    #[test]
    fn test_valid_waterfall_spec() {
        let tiers = vec![create_valid_tier("tier1", 1), create_valid_tier("tier2", 2)];

        assert!(validate_tiers(&tiers).is_empty());
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
