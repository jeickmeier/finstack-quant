//! Tests for structured credit waterfall validation helpers.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    get_validation_errors, is_valid_waterfall_spec, ValidationError,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AllocationMode, PaymentCalculation, PaymentType, Recipient, RecipientType, WaterfallTier,
};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    DiversionCondition, DiversionRule,
};

fn fixed_fee_recipient(id: &str) -> Recipient {
    Recipient::new(
        id,
        RecipientType::ServiceProvider("Trustee".to_string()),
        PaymentCalculation::FixedAmount {
            amount: Money::new(1.0, Currency::USD),
            rounding: None,
        },
    )
}

fn fee_tier(id: &str, priority: usize) -> WaterfallTier {
    WaterfallTier::new(id, priority, PaymentType::Fee).add_recipient(fixed_fee_recipient("r1"))
}

#[test]
fn test_duplicate_tier_ids() {
    let tiers = vec![fee_tier("tier-a", 1), fee_tier("tier-a", 2)];

    let errors = get_validation_errors(&tiers, &[], &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], ValidationError::DuplicateTierId { .. }));
}

#[test]
fn test_duplicate_recipient_ids_within_tier() {
    let tier = WaterfallTier::new("tier-a", 1, PaymentType::Fee)
        .add_recipient(fixed_fee_recipient("dup"))
        .add_recipient(fixed_fee_recipient("dup"));

    let errors = get_validation_errors(&[tier], &[], &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        ValidationError::DuplicateRecipientId { .. }
    ));
}

#[test]
fn test_empty_tier_is_invalid_except_residual() {
    let empty_fee = WaterfallTier::new("tier-a", 1, PaymentType::Fee);
    let errors = get_validation_errors(&[empty_fee], &[], &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], ValidationError::EmptyTier { .. }));

    let empty_residual = WaterfallTier::new("tier-b", 1, PaymentType::Residual);
    let residual_errors = get_validation_errors(&[empty_residual], &[], &[]);
    assert!(residual_errors.is_empty());
}

#[test]
fn test_negative_recipient_weight_is_invalid() {
    let tier = WaterfallTier::new("tier-a", 1, PaymentType::Fee)
        .add_recipient(fixed_fee_recipient("r1").with_weight(-0.25));

    let errors = get_validation_errors(&[tier], &[], &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(errors[0], ValidationError::InvalidWeight { .. }));
}

#[test]
fn test_pro_rata_requires_positive_total_weight() {
    let tier = WaterfallTier::new("tier-a", 1, PaymentType::Interest)
        .allocation_mode(AllocationMode::ProRata)
        .add_recipient(fixed_fee_recipient("r1").with_weight(0.0))
        .add_recipient(fixed_fee_recipient("r2").with_weight(0.0));

    let errors = get_validation_errors(&[tier], &[], &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        ValidationError::InvalidProRataWeights { .. }
    ));
}

#[test]
fn test_missing_coverage_test_reference() {
    let tiers = vec![fee_tier("tier-a", 1)];
    let rules = vec![DiversionRule::on_test_failure(
        "rule-1",
        "tier-a",
        "tier-a",
        "missing-test",
        1,
    )];

    let errors = get_validation_errors(&tiers, &rules, &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        ValidationError::MissingTestReference { .. }
    ));
}

#[test]
fn test_invalid_diversion_tiers() {
    let tiers = vec![fee_tier("tier-a", 1)];
    let rules = vec![DiversionRule::new(
        "rule-1",
        "missing-source",
        "missing-target",
        DiversionCondition::Always,
        1,
    )];

    let errors = get_validation_errors(&tiers, &rules, &[]);
    assert_eq!(errors.len(), 2);
    assert!(errors
        .iter()
        .all(|err| matches!(err, ValidationError::InvalidDiversionTier { .. })));
}

#[test]
fn test_circular_diversion_detection() {
    let tiers = vec![fee_tier("tier-a", 1), fee_tier("tier-b", 2)];
    let rules = vec![
        DiversionRule::new("rule-1", "tier-a", "tier-b", DiversionCondition::Always, 1),
        DiversionRule::new("rule-2", "tier-b", "tier-a", DiversionCondition::Always, 2),
    ];

    let errors = get_validation_errors(&tiers, &rules, &[]);
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        errors[0],
        ValidationError::CircularDiversion { .. }
    ));
}

#[test]
fn test_valid_spec_shortcut() {
    let tiers = vec![fee_tier("tier-a", 1), fee_tier("tier-b", 2)];
    assert!(is_valid_waterfall_spec(&tiers, &[], &[]));
}
