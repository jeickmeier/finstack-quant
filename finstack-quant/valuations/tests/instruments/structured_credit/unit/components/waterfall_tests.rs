//! Unit tests for waterfall engine and payment distribution.
//!
//! Tests cover:
//! - Waterfall builder construction
//! - Tier-based payment ordering
//! - Sequential distribution logic
//! - Pro-rata distribution logic
//! - OC/IC diversion triggers
//! - Payment calculation methods

use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AllocationMode, CoverageTestSpec, FundingSource, ManagementFeeType, PaymentCalculation,
    PaymentType, Recipient, RecipientType, TemplateFees, Waterfall, WaterfallBuilder,
    WaterfallTier,
};

// Waterfall Builder Tests

#[test]
fn test_waterfall_builder_creates_proper_priority_order() {
    // Arrange & Act
    let waterfall = WaterfallBuilder::new(Currency::USD)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "trustee",
                RecipientType::ServiceProvider("Trustee".into()),
                PaymentCalculation::FixedAmount {
                    amount: Money::new(25_000.0, Currency::USD).expect("valid money fixture"),
                    rounding: None,
                },
            )),
        )
        .add_tier(
            WaterfallTier::new("interest", 2, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A")),
        )
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal).add_recipient(
                Recipient::tranche_principal("class_a_prin", "CLASS_A", None),
            ),
        )
        .build()
        .expect("build waterfall");

    // Assert
    assert_eq!(waterfall.tiers.len(), 3);

    // Verify priority order (1, 2, 3)
    for (i, tier) in waterfall.tiers.iter().enumerate() {
        assert_eq!(tier.priority, i + 1);
    }
}

#[test]
fn test_waterfall_builder_tier_types() {
    // Arrange & Act
    let waterfall = WaterfallBuilder::new(Currency::USD)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "mgmt",
                RecipientType::ManagerFee(ManagementFeeType::Senior),
                PaymentCalculation::PercentageOfCollateral {
                    rate: 0.004,
                    annualized: true,
                    day_count: None,
                    rounding: None,
                },
            )),
        )
        .add_tier(
            WaterfallTier::new("interest", 2, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("int", "A")),
        )
        .build()
        .expect("build waterfall");

    // Assert
    assert_eq!(waterfall.tiers.len(), 2);
    assert_eq!(waterfall.tiers[0].payment_type, PaymentType::Fee);
    assert_eq!(waterfall.tiers[1].payment_type, PaymentType::Interest);
}

#[test]
fn test_waterfall_coverage_test_tier() {
    // Arrange & Act: a coverage-test position between interest and principal.
    let waterfall = WaterfallBuilder::new(Currency::USD)
        .add_tier(
            WaterfallTier::new("interest", 1, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("a_int", "A")),
        )
        .add_tier(WaterfallTier::coverage_tests(
            "a_coverage",
            2,
            vec![
                CoverageTestSpec::oc("A", 1.2),
                CoverageTestSpec::ic("A", 1.1),
            ],
        ))
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal)
                .add_recipient(Recipient::tranche_principal("a_prin", "A", None)),
        )
        .build()
        .expect("build waterfall");

    // Assert
    assert_eq!(waterfall.tiers[1].payment_type, PaymentType::CoverageTest);
    assert!(waterfall.tiers[1].recipients.is_empty());
    let ids: Vec<&str> = waterfall.coverage_tests().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["OC_A", "IC_A"]);
}

// Payment Priority Tests

#[test]
fn test_payment_priority_ordering() {
    // Arrange
    let mut waterfall = Waterfall::new(Currency::USD);

    waterfall = waterfall.add_tier(
        WaterfallTier::new("third", 3, PaymentType::Residual).add_recipient(Recipient::new(
            "equity",
            RecipientType::Equity,
            PaymentCalculation::ResidualCash,
        )),
    );

    waterfall = waterfall.add_tier(
        WaterfallTier::new("first", 1, PaymentType::Fee).add_recipient(Recipient::new(
            "fee",
            RecipientType::ServiceProvider("Test".into()),
            PaymentCalculation::FixedAmount {
                amount: Money::new(1000.0, Currency::USD).expect("valid money fixture"),
                rounding: None,
            },
        )),
    );

    waterfall = waterfall.add_tier(
        WaterfallTier::new("second", 2, PaymentType::Interest)
            .add_recipient(Recipient::tranche_interest("int", "A")),
    );

    // Assert - tiers should be auto-sorted by priority
    assert_eq!(waterfall.tiers[0].id, "first");
    assert_eq!(waterfall.tiers[1].id, "second");
    assert_eq!(waterfall.tiers[2].id, "third");
}

// Allocation Mode Tests

#[test]
fn test_allocation_mode_sequential() {
    let tier = WaterfallTier::new("test", 1, PaymentType::Fee)
        .allocation_mode(AllocationMode::Sequential)
        .add_recipient(Recipient::new(
            "r1",
            RecipientType::ServiceProvider("P1".into()),
            PaymentCalculation::FixedAmount {
                amount: Money::new(1000.0, Currency::USD).expect("valid money fixture"),
                rounding: None,
            },
        ));

    assert_eq!(tier.allocation_mode, AllocationMode::Sequential);
}

#[test]
fn test_allocation_mode_pro_rata() {
    let tier = WaterfallTier::new("test", 1, PaymentType::Interest)
        .allocation_mode(AllocationMode::ProRata)
        .add_recipient(
            Recipient::new(
                "r1",
                RecipientType::Tranche("A".into()),
                PaymentCalculation::ResidualCash,
            )
            .with_weight(0.60),
        )
        .add_recipient(
            Recipient::new(
                "r2",
                RecipientType::Tranche("B".into()),
                PaymentCalculation::ResidualCash,
            )
            .with_weight(0.40),
        );

    assert_eq!(tier.allocation_mode, AllocationMode::ProRata);
    assert_eq!(tier.recipients[0].weight, Some(0.60));
    assert_eq!(tier.recipients[1].weight, Some(0.40));
}

// Recipient Helper Tests

#[test]
fn test_recipient_fixed_fee_helper() {
    let recipient = Recipient::fixed_fee(
        "trustee",
        "Trustee",
        Money::new(50_000.0, Currency::USD).expect("valid money fixture"),
    );

    assert_eq!(recipient.id, "trustee");
    match &recipient.recipient_type {
        RecipientType::ServiceProvider(name) => assert_eq!(name, "Trustee"),
        _ => panic!("Expected ServiceProvider"),
    }
}

#[test]
fn test_recipient_tranche_interest_helper() {
    let recipient = Recipient::tranche_interest("class_a_int", "CLASS_A");

    assert_eq!(recipient.id, "class_a_int");
    match &recipient.recipient_type {
        RecipientType::Tranche(id) => assert_eq!(id, "CLASS_A"),
        _ => panic!("Expected Tranche recipient"),
    }
}

#[test]
fn test_recipient_tranche_principal_helper() {
    let recipient = Recipient::tranche_principal("class_a_prin", "CLASS_A", None);

    assert_eq!(recipient.id, "class_a_prin");
    match &recipient.calculation {
        PaymentCalculation::TranchePrincipal {
            tranche_id,
            target_balance,
            rounding,
        } => {
            assert_eq!(tranche_id, "CLASS_A");
            assert_eq!(*target_balance, None);
            assert_eq!(*rounding, None);
        }
        _ => panic!("Expected TranchePrincipal calculation"),
    }
}

// Multi-Recipient Tier Tests

#[test]
fn test_tier_multiple_recipients() {
    let tier = WaterfallTier::new("fees", 1, PaymentType::Fee)
        .add_recipient(Recipient::fixed_fee(
            "trustee",
            "Trustee",
            Money::new(50_000.0, Currency::USD).expect("valid money fixture"),
        ))
        .add_recipient(Recipient::fixed_fee(
            "admin",
            "Admin",
            Money::new(25_000.0, Currency::USD).expect("valid money fixture"),
        ))
        .add_recipient(Recipient::fixed_fee(
            "rating",
            "RatingAgency",
            Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
        ));

    assert_eq!(tier.recipients.len(), 3);
    assert_eq!(tier.recipients[0].id, "trustee");
    assert_eq!(tier.recipients[1].id, "admin");
    assert_eq!(tier.recipients[2].id, "rating");
}

#[test]
fn test_waterfall_engine_creation() {
    let engine = Waterfall::new(Currency::USD);

    assert_eq!(engine.base_currency, Currency::USD);
    assert_eq!(engine.tiers.len(), 0);
    assert_eq!(engine.coverage_tests().count(), 0);
}

#[test]
fn test_waterfall_engine_add_tier() {
    let tier = WaterfallTier::new("test", 1, PaymentType::Fee).add_recipient(Recipient::new(
        "recipient",
        RecipientType::ServiceProvider("Test".into()),
        PaymentCalculation::FixedAmount {
            amount: Money::new(1000.0, Currency::USD).expect("valid money fixture"),
            rounding: None,
        },
    ));

    let engine = Waterfall::new(Currency::USD).add_tier(tier);

    assert_eq!(engine.tiers.len(), 1);
    assert_eq!(engine.tiers[0].id, "test");
}

#[test]
fn abs_template_does_not_trap_junior_coupon() {
    use finstack_quant_core::dates::Date;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    };
    use time::Month;

    let maturity = Date::from_calendar_date(2030, Month::January, 1).expect("date");
    let senior = Tranche::new(
        "A",
        0.0,
        70.0,
        TrancheSeniority::Senior,
        Money::new(70_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.04 },
        maturity,
    )
    .expect("senior");
    let mezz = Tranche::new(
        "B",
        70.0,
        90.0,
        TrancheSeniority::Mezzanine,
        Money::new(20_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.06 },
        maturity,
    )
    .expect("mezz");
    let equity = Tranche::new(
        "EQ",
        90.0,
        100.0,
        TrancheSeniority::Equity,
        Money::new(10_000.0, Currency::USD).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.0 },
        maturity,
    )
    .expect("equity");
    let structure = TrancheStructure::new(vec![senior, mezz, equity]).expect("structure");

    let tests = vec![CoverageTestSpec::oc("B", 1.15)];
    let waterfall =
        Waterfall::standard_sequential(Currency::USD, &structure, TemplateFees::default(), &tests);

    // One interest tier per class, the B test right after B's coupon, so a
    // failing B test can trap only what ranks below B. Every tier draws on
    // its default account until the deal opts senior coupons into principal.
    let ids: Vec<&str> = waterfall
        .tiers
        .iter()
        .map(|tier| tier.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec![
            "A_interest",
            "B_interest",
            "B_coverage",
            "principal",
            "equity"
        ],
        "the template places the test after the tested class's coupon"
    );
    assert!(waterfall
        .tiers
        .iter()
        .all(|tier| tier.effective_funding() == FundingSource::default_for(tier.payment_type)));
    assert_eq!(waterfall.tiers[0].payment_type, PaymentType::Interest);
    assert_eq!(
        waterfall.tiers[0].recipients.len(),
        1,
        "each class has its own interest tier"
    );

    let mut funded = waterfall.clone();
    funded.fund_senior_interest_from_principal(&structure);
    assert_eq!(
        funded.tiers[0].effective_funding(),
        FundingSource::InterestThenPrincipal,
        "the senior coupon may top up from principal"
    );
    assert_eq!(
        funded.tiers[1].effective_funding(),
        FundingSource::Interest,
        "the mezzanine coupon stays on interest proceeds"
    );
}
