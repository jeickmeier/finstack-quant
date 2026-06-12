//! Unit tests for Tranche and TrancheStructure components.
//!
//! Tests cover:
//! - Tranche creation and validation
//! - Attachment/detachment points
//! - Loss allocation calculations
//! - Tranche structure validation
//! - Senior/subordination calculations
//! - Coverage triggers

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, Tenor};
use finstack_core::money::Money;
use finstack_core::types::CreditRating;
use finstack_core::types::CurveId;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    CoverageTrigger, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure, TriggerConsequence,
};
use rust_decimal_macros::dec;
use time::Month;

fn maturity_date() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

// ============================================================================
// Tranche Creation Tests
// ============================================================================

#[test]
fn test_tranche_creation_basic() {
    // Arrange & Act
    let tranche = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.12 },
        maturity_date(),
    )
    .unwrap();

    // Assert
    assert_eq!(tranche.id.as_str(), "EQUITY");
    assert_eq!(tranche.attachment_point, 0.0);
    assert_eq!(tranche.detachment_point, 10.0);
    assert_eq!(tranche.seniority, TrancheSeniority::Equity);
    assert_eq!(tranche.original_balance.amount(), 10_000_000.0);
    assert_eq!(tranche.current_balance.amount(), 10_000_000.0);
}

#[test]
fn test_tranche_creation_validates_attachment_points() {
    // Arrange & Act: Invalid attachment > detachment
    let result = Tranche::new(
        "INVALID",
        10.0,
        5.0, // detachment < attachment
        TrancheSeniority::Mezzanine,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    );

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_tranche_creation_validates_negative_attachment() {
    // Arrange & Act
    let result = Tranche::new(
        "INVALID",
        -5.0, // Negative attachment
        10.0,
        TrancheSeniority::Senior,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    );

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_tranche_creation_validates_detachment_over_100() {
    // Arrange & Act
    let result = Tranche::new(
        "INVALID",
        90.0,
        105.0, // Over 100%
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.04 },
        maturity_date(),
    );

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_tranche_fixed_coupon() {
    // Arrange & Act
    let tranche = Tranche::new(
        "SENIOR",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(90_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap();

    // Assert
    match tranche.coupon {
        TrancheCoupon::Fixed { rate } => assert_eq!(rate, 0.05),
        _ => panic!("Expected fixed coupon"),
    }
}

#[test]
fn test_tranche_floating_coupon() {
    // Arrange & Act
    let tranche = Tranche::new(
        "FLOATING",
        10.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(90_000_000.0, Currency::USD),
        TrancheCoupon::Floating(finstack_cashflows::builder::FloatingRateSpec {
            index_id: CurveId::new("SOFR-3M".to_string()),
            spread_bp: rust_decimal::Decimal::try_from(150.0).expect("valid"),
            gearing: rust_decimal::Decimal::try_from(1.0).expect("valid"),
            gearing_includes_spread: true,
            index_floor_bp: Some(rust_decimal::Decimal::try_from(0.0).expect("valid")), // 0 bps floor
            all_in_cap_bp: None,
            all_in_floor_bp: None,
            index_cap_bp: None,
            reset_freq: finstack_core::dates::Tenor::quarterly(),
            index_tenor: None,
            reset_lag_days: 2,
            dc: finstack_core::dates::DayCount::Act360,
            bdc: finstack_core::dates::BusinessDayConvention::ModifiedFollowing,
            calendar_id: "weekends_only".to_string(),
            fixing_calendar_id: None,
            end_of_month: false,
            overnight_compounding: None,
            overnight_basis: None,
            fallback: Default::default(),
            payment_lag_days: 0,
        }),
        maturity_date(),
    )
    .unwrap();

    // Assert
    match &tranche.coupon {
        TrancheCoupon::Floating(spec) => {
            assert_eq!(spec.spread_bp, dec!(150.0));
            assert_eq!(spec.index_floor_bp, Some(dec!(0.0)));
            assert_eq!(spec.all_in_cap_bp, None);
        }
        _ => panic!("Expected floating coupon"),
    }
}

// ============================================================================
// Tranche Builder Tests
// ============================================================================

#[test]
fn test_tranche_builder_complete() {
    // Arrange & Act
    let tranche = Tranche::builder()
        .id("MEZZANINE")
        .attachment_detachment(10.0, 15.0)
        .seniority(TrancheSeniority::Mezzanine)
        .balance(Money::new(5_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.08 })
        .maturity(maturity_date())
        .rating(CreditRating::BBB)
        .frequency(Tenor::quarterly())
        .build()
        .unwrap();

    // Assert
    assert_eq!(tranche.id.as_str(), "MEZZANINE");
    assert_eq!(tranche.attachment_point, 10.0);
    assert_eq!(tranche.detachment_point, 15.0);
    assert_eq!(tranche.rating, Some(CreditRating::BBB));
}

#[test]
fn test_tranche_builder_missing_required_field() {
    // Arrange & Act: Missing balance
    let result = Tranche::builder()
        .id("INCOMPLETE")
        .attachment_detachment(0.0, 10.0)
        .seniority(TrancheSeniority::Equity)
        // Missing: .balance()
        .coupon(TrancheCoupon::Fixed { rate: 0.12 })
        .maturity(maturity_date())
        .build();

    // Assert
    assert!(result.is_err());
}

// ============================================================================
// Tranche Characteristics Tests
// ============================================================================

#[test]
fn test_tranche_thickness() {
    // Arrange
    let tranche = Tranche::new(
        "MEZZ",
        10.0,
        15.0,
        TrancheSeniority::Mezzanine,
        Money::new(5_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    // Act
    let thickness = tranche.thickness();

    // Assert
    assert_eq!(thickness, 5.0);
}

#[test]
fn test_tranche_is_first_loss() {
    // Arrange
    let equity = Tranche::new(
        "EQUITY",
        0.0,
        10.0,
        TrancheSeniority::Equity,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.15 },
        maturity_date(),
    )
    .unwrap();

    let mezz = Tranche::new(
        "MEZZ",
        10.0,
        15.0,
        TrancheSeniority::Mezzanine,
        Money::new(5_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    // Assert
    assert!(equity.is_first_loss());
    assert!(!mezz.is_first_loss());
}

#[test]
fn test_tranche_is_impaired() {
    // Arrange
    let tranche = Tranche::new(
        "MEZZ",
        10.0,
        15.0,
        TrancheSeniority::Mezzanine,
        Money::new(5_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    // Act & Assert
    assert!(!tranche.is_impaired(5.0)); // Loss below attachment
    assert!(tranche.is_impaired(12.0)); // Loss in tranche
    assert!(tranche.is_impaired(20.0)); // Loss above detachment
}

// ============================================================================
// Loss Allocation Tests
// ============================================================================

#[test]
fn test_tranche_loss_allocation_no_loss() {
    // Arrange
    let tranche = Tranche::new(
        "MEZZ",
        10.0,
        15.0,
        TrancheSeniority::Mezzanine,
        Money::new(50_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    let pool_balance = Money::new(1_000_000_000.0, Currency::USD);

    // Act: Loss below attachment
    let loss = tranche.loss_allocation(5.0, pool_balance);

    // Assert
    assert_eq!(loss.amount(), 0.0);
}

#[test]
fn test_tranche_loss_allocation_partial_loss() {
    // Arrange
    let tranche = Tranche::new(
        "MEZZ",
        10.0, // 10% attachment
        15.0, // 15% detachment (5% thick)
        TrancheSeniority::Mezzanine,
        Money::new(50_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    let pool_balance = Money::new(1_000_000_000.0, Currency::USD);

    // Act: 12% loss (2% into the tranche)
    let loss = tranche.loss_allocation(12.0, pool_balance);

    // Assert: 2% / 5% = 40% of tranche = $20M
    assert_eq!(loss.amount(), 20_000_000.0);
}

#[test]
fn test_tranche_loss_allocation_full_loss() {
    // Arrange
    let tranche = Tranche::new(
        "MEZZ",
        10.0,
        15.0,
        TrancheSeniority::Mezzanine,
        Money::new(50_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    let pool_balance = Money::new(1_000_000_000.0, Currency::USD);

    // Act: 20% loss (beyond detachment)
    let loss = tranche.loss_allocation(20.0, pool_balance);

    // Assert: Full tranche wiped out
    assert_eq!(loss.amount(), 50_000_000.0);
}

#[test]
fn test_tranche_current_balance_after_losses() {
    // Arrange
    let tranche = Tranche::new(
        "MEZZ",
        10.0,
        15.0,
        TrancheSeniority::Mezzanine,
        Money::new(50_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.08 },
        maturity_date(),
    )
    .unwrap();

    let pool_balance = Money::new(1_000_000_000.0, Currency::USD);

    // Act: 12% loss
    let balance = tranche.current_balance_after_losses(12.0, pool_balance);

    // Assert: Original $50M - $20M loss = $30M
    assert_eq!(balance.amount(), 30_000_000.0);
}

// ============================================================================
// TrancheStructure Tests
// ============================================================================

#[test]
fn test_tranche_structure_creation() {
    // Arrange
    let equity = Tranche::builder()
        .id("EQUITY")
        .attachment_detachment(0.0, 10.0)
        .seniority(TrancheSeniority::Equity)
        .balance(Money::new(10_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.12 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    let senior = Tranche::builder()
        .id("SENIOR")
        .attachment_detachment(10.0, 100.0)
        .seniority(TrancheSeniority::Senior)
        .balance(Money::new(90_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.05 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    // Act
    let structure = TrancheStructure::new(vec![equity, senior]).unwrap();

    // Assert
    assert_eq!(structure.tranches.len(), 2);
    assert_eq!(structure.total_size.amount(), 100_000_000.0);
}

#[test]
fn test_tranche_structure_validates_gaps() {
    // Arrange: Gap between 10% and 20%
    let equity = Tranche::builder()
        .id("EQUITY")
        .attachment_detachment(0.0, 10.0)
        .seniority(TrancheSeniority::Equity)
        .balance(Money::new(10_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.12 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    let senior = Tranche::builder()
        .id("SENIOR")
        .attachment_detachment(20.0, 100.0) // Gap!
        .seniority(TrancheSeniority::Senior)
        .balance(Money::new(80_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.05 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    // Act
    let result = TrancheStructure::new(vec![equity, senior]);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_tranche_structure_validates_overlap() {
    // Arrange: Overlap between tranches
    let tranche1 = Tranche::builder()
        .id("T1")
        .attachment_detachment(0.0, 15.0)
        .seniority(TrancheSeniority::Equity)
        .balance(Money::new(15_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.12 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    let tranche2 = Tranche::builder()
        .id("T2")
        .attachment_detachment(10.0, 100.0) // Overlaps with T1!
        .seniority(TrancheSeniority::Senior)
        .balance(Money::new(85_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.05 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    // Act
    let result = TrancheStructure::new(vec![tranche1, tranche2]);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_tranche_structure_validates_reaches_100() {
    // Arrange: Only goes to 90%
    let tranche = Tranche::builder()
        .id("INCOMPLETE")
        .attachment_detachment(0.0, 90.0)
        .seniority(TrancheSeniority::Senior)
        .balance(Money::new(90_000_000.0, Currency::USD))
        .coupon(TrancheCoupon::Fixed { rate: 0.05 })
        .maturity(maturity_date())
        .build()
        .unwrap();

    // Act
    let result = TrancheStructure::new(vec![tranche]);

    // Assert
    assert!(result.is_err());
}

#[test]
fn test_tranche_structure_by_seniority() {
    // Arrange
    let equity = create_tranche("EQUITY", 0.0, 10.0, TrancheSeniority::Equity);
    let mezz = create_tranche("MEZZ", 10.0, 15.0, TrancheSeniority::Mezzanine);
    let senior = create_tranche("SENIOR", 15.0, 100.0, TrancheSeniority::Senior);

    let structure = TrancheStructure::new(vec![equity, mezz, senior]).unwrap();

    // Act
    let senior_tranches = structure.by_seniority(TrancheSeniority::Senior);

    // Assert
    assert_eq!(senior_tranches.len(), 1);
    assert_eq!(senior_tranches[0].id.as_str(), "SENIOR");
}

#[test]
fn test_tranche_structure_senior_to() {
    // Arrange
    let equity = create_tranche("EQUITY", 0.0, 10.0, TrancheSeniority::Equity);
    let mezz = create_tranche("MEZZ", 10.0, 15.0, TrancheSeniority::Mezzanine);
    let senior = create_tranche("SENIOR", 15.0, 100.0, TrancheSeniority::Senior);

    let structure = TrancheStructure::new(vec![equity, mezz, senior]).unwrap();

    // Act
    let senior_to_mezz = structure.senior_to("MEZZ");

    // Assert
    assert_eq!(senior_to_mezz.len(), 1);
    assert_eq!(senior_to_mezz[0].id.as_str(), "SENIOR");
}

#[test]
fn test_tranche_structure_senior_balance() {
    // Arrange
    let equity = create_tranche("EQUITY", 0.0, 10.0, TrancheSeniority::Equity);
    let mezz =
        create_tranche_with_balance("MEZZ", 10.0, 15.0, TrancheSeniority::Mezzanine, 5_000_000.0);
    let senior = create_tranche_with_balance(
        "SENIOR",
        15.0,
        100.0,
        TrancheSeniority::Senior,
        85_000_000.0,
    );

    let structure = TrancheStructure::new(vec![equity, mezz, senior]).unwrap();

    // Act
    let senior_bal = structure.senior_balance("MEZZ");

    // Assert
    assert_eq!(senior_bal.amount(), 85_000_000.0);
}

#[test]
fn test_tranche_structure_subordination_amount() {
    // Arrange
    let equity =
        create_tranche_with_balance("EQUITY", 0.0, 10.0, TrancheSeniority::Equity, 10_000_000.0);
    let mezz =
        create_tranche_with_balance("MEZZ", 10.0, 15.0, TrancheSeniority::Mezzanine, 5_000_000.0);
    let senior = create_tranche_with_balance(
        "SENIOR",
        15.0,
        100.0,
        TrancheSeniority::Senior,
        85_000_000.0,
    );

    let structure = TrancheStructure::new(vec![equity, mezz, senior]).unwrap();

    // Act
    let sub_amount = structure.subordination_amount("SENIOR");

    // Assert: Equity + Mezz = $15M
    assert_eq!(sub_amount.amount(), 15_000_000.0);
}

// ============================================================================
// Coverage Trigger Tests
// ============================================================================

#[test]
fn test_coverage_trigger_creation() {
    // Arrange & Act
    let trigger =
        CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow).with_cure_level(1.25);

    // Assert
    assert_eq!(trigger.trigger_level, 1.20);
    assert_eq!(trigger.cure_level, Some(1.25));
}

#[test]
fn test_coverage_trigger_is_breached() {
    // Arrange
    let trigger = CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow);

    // Act & Assert
    assert!(trigger.is_breached(1.15)); // Below trigger
    assert!(!trigger.is_breached(1.20)); // At trigger
    assert!(!trigger.is_breached(1.25)); // Above trigger
}

#[test]
fn test_coverage_trigger_is_cured() {
    // Arrange
    let trigger =
        CoverageTrigger::new(1.20, TriggerConsequence::DivertCashFlow).with_cure_level(1.25);

    // Act & Assert
    assert!(!trigger.is_cured(1.22)); // Above trigger but below cure
    assert!(trigger.is_cured(1.25)); // At cure level
    assert!(trigger.is_cured(1.30)); // Above cure level
}

// ============================================================================
// Helper Functions
// ============================================================================

fn create_tranche(id: &str, attach: f64, detach: f64, seniority: TrancheSeniority) -> Tranche {
    Tranche::new(
        id,
        attach,
        detach,
        seniority,
        Money::new(10_000_000.0, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap()
}

fn create_tranche_with_balance(
    id: &str,
    attach: f64,
    detach: f64,
    seniority: TrancheSeniority,
    balance: f64,
) -> Tranche {
    Tranche::new(
        id,
        attach,
        detach,
        seniority,
        Money::new(balance, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        maturity_date(),
    )
    .unwrap()
}
