//! Validation framework for waterfall specifications.
//!
//! This module provides validation for waterfall specifications,
//! ensuring correctness before execution. It checks for:
//! - Duplicate tier/recipient IDs
//! - Invalid priority values
//! - Empty/impossible tier configurations

use crate::structured_credit::types::{
    AllocationMode, PaymentCalculation, PaymentType, RecipientType, WaterfallTier,
};
use finstack_quant_core::FxHashSet;

/// Validation error details.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
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
    /// A debt class is paid by a calculation that is not a note entitlement.
    #[error(
        "Recipient '{recipient_id}' in tier '{tier_id}' pays tranche '{tranche_id}' with a \
         {calculation} calculation, which a note has no claim to. Pay its coupon with \
         tranche_interest, its balance with tranche_principal or residual_cash, and fee or \
         reserve formulas to a service provider, manager fee or reserve account. Model a class \
         that takes excess cash with equity seniority."
    )]
    NonNoteCalculation {
        /// Tier id.
        tier_id: String,
        /// Recipient id.
        recipient_id: String,
        /// Tranche the recipient pays.
        tranche_id: String,
        /// Name of the rejected calculation.
        calculation: &'static str,
    },
    /// A recipient pays one tranche the interest or principal of another.
    #[error(
        "Recipient '{recipient_id}' in tier '{tier_id}' pays tranche '{tranche_id}' an amount \
         calculated for tranche '{calculated_for}'"
    )]
    TrancheMismatch {
        /// Tier id.
        tier_id: String,
        /// Recipient id.
        recipient_id: String,
        /// Tranche the recipient pays.
        tranche_id: String,
        /// Tranche the calculation names.
        calculated_for: String,
    },
}

/// What a tranche recipient may be paid: `Ok` with the tranche a note calculation
/// is sized on (`None` for residual cash), or `Err` naming a calculation no note
/// has a claim to.
///
/// A note is owed interest at its rate and the return of its balance. A fixed or
/// percentage amount is a fee formula with no basis as either, and paid to a note
/// it would be cash booked as neither interest nor principal.
fn non_note_calculation(calculation: &PaymentCalculation) -> Result<Option<&str>, &'static str> {
    match calculation {
        PaymentCalculation::TrancheInterest { tranche_id, .. }
        | PaymentCalculation::CappedTrancheInterest { tranche_id, .. }
        | PaymentCalculation::TranchePrincipal { tranche_id, .. }
        | PaymentCalculation::NetWacCarryover { tranche_id, .. } => Ok(Some(tranche_id)),
        PaymentCalculation::ResidualCash => Ok(None),
        PaymentCalculation::FixedAmount { .. } => Err("fixed_amount"),
        PaymentCalculation::PercentageOfCollateral { .. } => Err("percentage_of_collateral"),
        PaymentCalculation::PercentageOfSpecialServiced { .. } => {
            Err("percentage_of_special_serviced")
        }
        PaymentCalculation::IncentiveFee { .. } => Err("incentive_fee"),
        PaymentCalculation::ReserveReplenishment { .. } => Err("reserve_replenishment"),
    }
}

/// Structural validation of waterfall tiers: duplicate tier or recipient
/// ids, empty tiers, non-finite or negative weights and payment parameters,
/// pro-rata tiers without positive total weight, and tranche recipients paid
/// anything but their own interest, carryover, principal or residual cash.
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

    let mut seen_tier_ids = FxHashSet::default();
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

        let mut seen_recipient_ids = FxHashSet::default();
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
            if let RecipientType::Tranche(tranche_id) = &recipient.recipient_type {
                match non_note_calculation(&recipient.calculation) {
                    Err(calculation) => errors.push(ValidationError::NonNoteCalculation {
                        tier_id: tier.id.clone(),
                        recipient_id: recipient.id.clone(),
                        tranche_id: tranche_id.clone(),
                        calculation,
                    }),
                    Ok(Some(calculated_for)) if calculated_for != tranche_id => {
                        errors.push(ValidationError::TrancheMismatch {
                            tier_id: tier.id.clone(),
                            recipient_id: recipient.id.clone(),
                            tranche_id: tranche_id.clone(),
                            calculated_for: calculated_for.to_string(),
                        });
                    }
                    Ok(_) => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structured_credit::types::{PaymentCalculation, Recipient};
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

    fn paying_class_b(calculation: PaymentCalculation) -> Vec<ValidationError> {
        validate_tiers(&[
            WaterfallTier::new("tier", 1, PaymentType::Residual).add_recipient(Recipient::new(
                "class_b",
                RecipientType::Tranche("CLASS_B".to_string()),
                calculation,
            )),
        ])
    }

    /// A note is owed interest and principal, so every fee or reserve formula
    /// is rejected for it, and the error says what to use instead.
    #[test]
    fn a_debt_class_cannot_be_paid_a_fee_or_reserve_calculation() {
        let usd = Money::from((50_000_i64, Currency::USD));
        for (calculation, name) in [
            (
                PaymentCalculation::FixedAmount {
                    amount: usd,
                    rounding: None,
                },
                "fixed_amount",
            ),
            (
                PaymentCalculation::PercentageOfCollateral {
                    rate: 0.01,
                    annualized: true,
                    day_count: None,
                    rounding: None,
                },
                "percentage_of_collateral",
            ),
            (
                PaymentCalculation::PercentageOfSpecialServiced {
                    rate: 0.01,
                    annualized: true,
                    day_count: None,
                    rounding: None,
                },
                "percentage_of_special_serviced",
            ),
            (
                PaymentCalculation::IncentiveFee {
                    hurdle_irr: 0.1,
                    share: 0.2,
                },
                "incentive_fee",
            ),
            (
                PaymentCalculation::ReserveReplenishment {
                    target_balance: usd,
                },
                "reserve_replenishment",
            ),
        ] {
            let errors = paying_class_b(calculation);
            assert_eq!(errors.len(), 1, "{name}");
            assert!(matches!(
                errors[0],
                ValidationError::NonNoteCalculation { calculation, .. } if calculation == name
            ));
            let message = errors[0].to_string();
            assert!(message.contains("CLASS_B"), "{message}");
            assert!(message.contains("tranche_principal"), "{message}");
            assert!(message.contains("equity seniority"), "{message}");
        }
    }

    #[test]
    fn a_debt_class_can_be_paid_its_own_claims_and_residual_cash() {
        for calculation in [
            PaymentCalculation::TrancheInterest {
                tranche_id: "CLASS_B".to_string(),
                rounding: None,
            },
            PaymentCalculation::CappedTrancheInterest {
                tranche_id: "CLASS_B".to_string(),
                cap_rate: 0.05,
                rounding: None,
            },
            PaymentCalculation::TranchePrincipal {
                tranche_id: "CLASS_B".to_string(),
                target_balance: None,
                rounding: None,
            },
            PaymentCalculation::ResidualCash,
        ] {
            assert_eq!(paying_class_b(calculation), Vec::new());
        }
    }

    /// Cash to one class sized on another retires or defers the wrong balance.
    #[test]
    fn a_tranche_cannot_be_paid_another_tranches_amount() {
        for calculation in [
            PaymentCalculation::TrancheInterest {
                tranche_id: "CLASS_A".to_string(),
                rounding: None,
            },
            PaymentCalculation::TranchePrincipal {
                tranche_id: "CLASS_A".to_string(),
                target_balance: None,
                rounding: None,
            },
        ] {
            let errors = paying_class_b(calculation);
            assert!(matches!(
                &errors[..],
                [ValidationError::TrancheMismatch { calculated_for, .. }] if calculated_for == "CLASS_A"
            ));
        }
    }

    /// Fee and reserve calculations keep their own recipients.
    #[test]
    fn a_fee_recipient_can_use_a_fee_calculation() {
        assert!(validate_tiers(&[create_valid_tier("tier", 1)]).is_empty());
    }
}
