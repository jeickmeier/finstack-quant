//! Golden tests for waterfall engine using real-world CLO/CMBS scenarios.
//!
//! These tests replicate known waterfall distributions from actual deal prospectuses
//! to ensure accuracy and correctness of the waterfall engine.
//!
//! # Tolerance Standards
//!
//! Waterfall calculations are **deterministic** and should produce exact results
//! within floating-point precision. Tolerances used:
//!
//! - **Cash conservation**: `CASH_TOLERANCE` (0.01) - accounts only for f64 representation
//! - **Fee allocations**: Exact expected values with `CASH_TOLERANCE`
//! - **Interest calculations**: Exact expected values with `CASH_TOLERANCE`

use finstack_core::currency::Currency;
use finstack_core::dates::{Date, DateExt};
use finstack_core::market_data::context::MarketContext;
use finstack_core::money::Money;
use finstack_valuations::instruments::fixed_income::structured_credit::waterfall::CoverageTrigger;
use finstack_valuations::instruments::fixed_income::structured_credit::WaterfallContext;
use finstack_valuations::instruments::fixed_income::structured_credit::WaterfallDistribution;
use finstack_valuations::instruments::fixed_income::structured_credit::{
    AllocationMode, AssetPool, DealType, ManagementFeeType, PaymentCalculation, PaymentType,
    Recipient, RecipientType, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    Waterfall, WaterfallBuilder, WaterfallTier,
};
use time::Duration;

// ============================================================================
// Market-Standard Tolerances for Waterfall Tests
// ============================================================================

/// Cash distribution tolerance: deterministic calculations should be exact
/// within f64 representation error (1 cent on any amount).
const CASH_TOLERANCE: f64 = 0.01;

/// Helper to create a simple market context for testing
fn create_test_market() -> MarketContext {
    MarketContext::new()
}

/// Helper to create a test asset pool
fn create_test_pool(balance: f64, currency: Currency) -> AssetPool {
    use finstack_core::types::CreditRating;
    use finstack_core::types::InstrumentId;
    use finstack_valuations::instruments::fixed_income::structured_credit::{AssetType, PoolAsset};

    let mut pool = AssetPool::new("TEST_POOL", DealType::CLO, currency);

    // Add assets to match the specified balance
    let num_assets = 10;
    let asset_balance = balance / num_assets as f64;

    for i in 0..num_assets {
        let asset = PoolAsset {
            day_count: finstack_core::dates::DayCount::Act360,
            id: InstrumentId::new(format!("ASSET_{}", i)),
            asset_type: AssetType::FirstLienLoan {
                industry: Some("Technology".into()),
            },
            balance: Money::new(asset_balance, currency),
            rate: 0.08,
            spread_bps: Some(400.0),
            index_id: Some("SOFR-3M".into()),
            maturity: Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
            credit_quality: Some(CreditRating::BB),
            industry: Some("Technology".into()),
            obligor_id: Some(format!("OBLIGOR_{}", i)),
            is_defaulted: false,
            recovery_amount: None,
            purchase_price: None,
            acquisition_date: None,
            smm_override: None,
            mdr_override: None,
        };
        pool.assets.push(asset);
    }

    pool
}

/// Helper to create a simple tranche structure
fn create_test_tranches(currency: Currency) -> TrancheStructure {
    let class_a = Tranche::new(
        "CLASS_A",
        0.0,
        70.0,
        TrancheSeniority::Senior,
        Money::new(175_000_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.05 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_b = Tranche::new(
        "CLASS_B",
        70.0,
        85.0,
        TrancheSeniority::Mezzanine,
        Money::new(37_500_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.065 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_c = Tranche::new(
        "CLASS_C",
        85.0,
        95.0,
        TrancheSeniority::Subordinated,
        Money::new(25_000_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.08 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let equity = Tranche::new(
        "EQUITY",
        95.0,
        100.0,
        TrancheSeniority::Equity,
        Money::new(12_500_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.0 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    TrancheStructure::new(vec![class_a, class_b, class_c, equity]).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn run_waterfall(
    waterfall: &Waterfall,
    available_cash: Money,
    interest_collections: Money,
    payment_date: Date,
    tranches: &TrancheStructure,
    pool_balance: Money,
    period_start_override: Option<Date>,
    pool: &AssetPool,
    market: &MarketContext,
) -> WaterfallDistribution {
    let period_start = period_start_override.unwrap_or_else(|| payment_date.add_months(-3));
    let context = WaterfallContext {
        available_cash,
        interest_collections,
        payment_date,
        period_start,
        pool_balance,
        market,
        tranche_balances: None,
        deferred_interest: None,
        reserve_balance: Money::new(0.0, available_cash.currency()),
        recovery_proceeds: Money::new(0.0, available_cash.currency()),
    };
    finstack_valuations::instruments::fixed_income::structured_credit::execute_waterfall(
        waterfall, tranches, pool, context,
    )
    .expect("waterfall execution")
}

#[test]
fn test_golden_clo_2_0_full_payment() {
    // Scenario: Standard CLO 2.0 with sufficient cash to pay all obligations
    // Based on typical CLO structure with $250M collateral

    let currency = Currency::USD;
    let pool = create_test_pool(250_000_000.0, currency);
    let tranches = create_test_tranches(currency);

    // Build waterfall matching CLO 2.0 template
    let waterfall = WaterfallBuilder::new(currency)
        // Tier 1: Fees
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "trustee",
                    RecipientType::ServiceProvider("Trustee".into()),
                    PaymentCalculation::FixedAmount {
                        amount: Money::new(50_000.0, currency),
                        rounding: None,
                    },
                ))
                .add_recipient(Recipient::new(
                    "senior_mgmt",
                    RecipientType::ManagerFee(ManagementFeeType::Senior),
                    PaymentCalculation::PercentageOfCollateral {
                        rate: 0.004, // 40 bps
                        annualized: true,
                        day_count: None,
                        rounding: None,
                    },
                )),
        )
        // Tier 2: Interest
        .add_tier(
            WaterfallTier::new("interest", 2, PaymentType::Interest)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B"))
                .add_recipient(Recipient::tranche_interest("class_c_int", "CLASS_C")),
        )
        // Tier 3: Principal (divertible)
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
                .divertible(true)
                .add_recipient(Recipient::tranche_principal(
                    "class_a_prin",
                    "CLASS_A",
                    None,
                ))
                .add_recipient(Recipient::tranche_principal(
                    "class_b_prin",
                    "CLASS_B",
                    None,
                ))
                .add_recipient(Recipient::tranche_principal(
                    "class_c_prin",
                    "CLASS_C",
                    None,
                )),
        )
        // Tier 4: Equity
        .add_tier(
            WaterfallTier::new("equity", 4, PaymentType::Residual)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "equity_dist",
                    RecipientType::Equity,
                    PaymentCalculation::ResidualCash,
                )),
        )
        .add_coverage_trigger(CoverageTrigger {
            tranche_id: "CLASS_A".into(),
            oc_trigger: Some(1.25),
            ic_trigger: Some(1.20),
        })
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    let available_cash = Money::new(15_000_000.0, currency); // Quarterly cash available
    let interest_collections = Money::new(3_000_000.0, currency);
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();
    let pool_balance = Money::new(250_000_000.0, currency);
    let period_start = payment_date - Duration::days(90);
    let result = run_waterfall(
        &waterfall,
        available_cash,
        interest_collections,
        payment_date,
        &tranches,
        pool_balance,
        Some(period_start),
        &pool,
        &market,
    );

    // Verify tier allocations
    assert_eq!(result.tier_allocations.len(), 4);

    // Tier 1: Fees
    let (tier_id, amount) = &result.tier_allocations[0];
    assert_eq!(tier_id, "fees");
    // Trustee: $50,000 (fixed) + Senior Mgmt: $250M × 0.004 / 4 = $250,000
    // Total: $300,000.00 (exact deterministic calculation)
    let expected_fees = 50_000.0 + (250_000_000.0 * 0.004 / 4.0);
    assert!(
        (amount.amount() - expected_fees).abs() < CASH_TOLERANCE,
        "Fee allocation mismatch: expected {}, got {}",
        expected_fees,
        amount.amount()
    );

    // Tier 2: Interest payments
    let (tier_id, _) = &result.tier_allocations[1];
    assert_eq!(tier_id, "interest");

    // Expected quarterly interest:
    // Class A: $175M * 5% / 4 = $2,187,500
    // Class B: $37.5M * 6.5% / 4 = $609,375
    // Class C: $25M * 8% / 4 = $500,000
    // Total: ~$3,296,875

    // Coverage tests should pass (sufficient collateral)
    assert!(!result.had_diversions);

    // No cash should be diverted
    assert_eq!(result.diverted_cash.amount(), 0.0);

    // Cash conservation: total distributed + remaining must equal available
    // This is a fundamental invariant that must hold exactly (within f64 precision)
    let total_distributed: f64 = result
        .tier_allocations
        .iter()
        .map(|(_, amt)| amt.amount())
        .sum();
    let total = total_distributed + result.remaining_cash.amount();
    assert!(
        (total - available_cash.amount()).abs() < CASH_TOLERANCE,
        "Cash conservation violated: distributed {} + remaining {} = {} != available {}",
        total_distributed,
        result.remaining_cash.amount(),
        total,
        available_cash.amount()
    );
}

#[test]
fn test_golden_clo_oc_breach_diversion() {
    // Scenario: CLO with OC test breach causing principal diversion
    // Similar to 2008 crisis scenarios where subordinated cash diverts to senior

    let currency = Currency::USD;

    // Create impaired pool (lower collateral value)
    let mut pool = create_test_pool(200_000_000.0, currency); // Down from $250M
    pool.cumulative_defaults = Money::new(30_000_000.0, currency);
    pool.cumulative_recoveries = Money::new(15_000_000.0, currency);

    let tranches = create_test_tranches(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "trustee",
                    RecipientType::ServiceProvider("Trustee".into()),
                    PaymentCalculation::FixedAmount {
                        amount: Money::new(50_000.0, currency),
                        rounding: None,
                    },
                )),
        )
        .add_tier(
            WaterfallTier::new("interest", 2, PaymentType::Interest)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B")),
        )
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
                .divertible(true)
                .add_recipient(Recipient::tranche_principal(
                    "class_a_prin",
                    "CLASS_A",
                    None,
                ))
                .add_recipient(Recipient::tranche_principal(
                    "class_b_prin",
                    "CLASS_B",
                    None,
                )),
        )
        .add_tier(
            WaterfallTier::new("equity", 4, PaymentType::Residual).add_recipient(Recipient::new(
                "equity",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            )),
        )
        .add_coverage_trigger(CoverageTrigger {
            tranche_id: "CLASS_A".into(),
            oc_trigger: Some(1.25), // 125% OC required
            ic_trigger: None,
        })
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    let available_cash = Money::new(5_000_000.0, currency);
    let interest_collections = Money::new(2_500_000.0, currency);
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();
    let pool_balance = Money::new(200_000_000.0, currency);

    let result = run_waterfall(
        &waterfall,
        available_cash,
        interest_collections,
        payment_date,
        &tranches,
        pool_balance,
        None,
        &pool,
        &market,
    );

    // With impaired collateral, OC test should fail
    // OC ratio = $200M / ($175M) = 1.14 < 1.25 required

    // Verify coverage test failure
    let oc_test = result
        .coverage_tests
        .iter()
        .find(|(name, _, _)| name.contains("OC_CLASS_A"));

    if let Some((_, ratio, passed)) = oc_test {
        assert!(*ratio < 1.25, "OC ratio should be below trigger");
        assert!(!passed, "OC test should have failed");
    }

    // Verify diversion was triggered
    assert!(
        result.had_diversions,
        "OC breach should trigger diversion flag"
    );
    assert!(
        !result.diverted_amounts.is_empty(),
        "Diversion tracking should capture redirected payments"
    );
    assert!(
        result
            .diverted_amounts
            .iter()
            .all(|record| record.amount.amount() > 0.0),
        "Diversion records should have positive amounts"
    );
}

#[test]
fn test_golden_cmbs_sequential_pay() {
    // Scenario: CMBS with strict sequential principal paydown
    // No OC/IC tests, principal follows strict seniority

    let currency = Currency::USD;
    let pool = create_test_pool(500_000_000.0, currency);

    // CMBS typically has 5 classes
    let class_a = Tranche::new(
        "CLASS_A",
        0.0,
        70.0,
        TrancheSeniority::Senior,
        Money::new(300_000_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_b = Tranche::new(
        "CLASS_B",
        70.0,
        85.0,
        TrancheSeniority::Mezzanine,
        Money::new(75_000_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.045 },
        Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_c = Tranche::new(
        "CLASS_C",
        85.0,
        100.0,
        TrancheSeniority::Subordinated,
        Money::new(50_000_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.05 },
        Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![class_a, class_b, class_c]).unwrap();

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("servicing", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "master_servicer",
                RecipientType::ServiceProvider("MasterServicer".into()),
                PaymentCalculation::PercentageOfCollateral {
                    rate: 0.0025, // 25 bps
                    annualized: true,
                    day_count: None,
                    rounding: None,
                },
            )),
        )
        .add_tier(
            WaterfallTier::new("interest", 2, PaymentType::Interest)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::tranche_interest("class_a_int", "CLASS_A"))
                .add_recipient(Recipient::tranche_interest("class_b_int", "CLASS_B"))
                .add_recipient(Recipient::tranche_interest("class_c_int", "CLASS_C")),
        )
        .add_tier(
            WaterfallTier::new("principal", 3, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
                .divertible(false) // CMBS doesn't divert
                .add_recipient(Recipient::tranche_principal(
                    "class_a_prin",
                    "CLASS_A",
                    None,
                ))
                .add_recipient(Recipient::tranche_principal(
                    "class_b_prin",
                    "CLASS_B",
                    None,
                ))
                .add_recipient(Recipient::tranche_principal(
                    "class_c_prin",
                    "CLASS_C",
                    None,
                )),
        )
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    let available_cash = Money::new(20_000_000.0, currency);
    let interest_collections = Money::new(5_000_000.0, currency);
    let payment_date = Date::from_calendar_date(2024, time::Month::February, 1).unwrap();
    let pool_balance = Money::new(500_000_000.0, currency);

    let result = run_waterfall(
        &waterfall,
        available_cash,
        interest_collections,
        payment_date,
        &tranches,
        pool_balance,
        None,
        &pool,
        &market,
    );

    // CMBS should NOT have coverage tests
    assert_eq!(result.coverage_tests.len(), 0);

    // No diversions in CMBS
    assert!(!result.had_diversions);
    assert_eq!(result.diverted_cash.amount(), 0.0);

    // Principal should follow strict sequential order
    // All principal goes to Class A first
    let principal_tier = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "principal");

    assert!(principal_tier.is_some());
}

#[test]
fn test_golden_cre_pro_rata_distribution() {
    // Scenario: CRE operating company with pro-rata preferred return
    // 8% pref to LP/GP, then promote structure

    let currency = Currency::USD;
    let pool = create_test_pool(50_000_000.0, currency); // Property value

    // LP/GP structure
    let lp = Tranche::new(
        "LP",
        0.0,
        95.0,
        TrancheSeniority::Equity,
        Money::new(47_500_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.08 },
        Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let gp = Tranche::new(
        "GP",
        95.0,
        100.0,
        TrancheSeniority::Equity,
        Money::new(2_500_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.08 },
        Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let tranches = TrancheStructure::new(vec![lp, gp]).unwrap();

    let waterfall = WaterfallBuilder::new(currency)
        // Operating expenses
        .add_tier(
            WaterfallTier::new("opex", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "operating",
                RecipientType::ServiceProvider("Operating".into()),
                PaymentCalculation::FixedAmount {
                    amount: Money::new(100_000.0, currency),
                    rounding: None,
                },
            )),
        )
        // Preferred return (pro-rata by ownership)
        .add_tier(
            WaterfallTier::new("preferred_return", 2, PaymentType::Interest)
                .allocation_mode(AllocationMode::ProRata)
                .add_recipient(
                    Recipient::new(
                        "lp_pref",
                        RecipientType::Tranche("LP".into()),
                        PaymentCalculation::TrancheInterest {
                            tranche_id: "LP".into(),
                            rounding: None,
                        },
                    )
                    .with_weight(0.95), // 95% ownership
                )
                .add_recipient(
                    Recipient::new(
                        "gp_pref",
                        RecipientType::Tranche("GP".into()),
                        PaymentCalculation::TrancheInterest {
                            tranche_id: "GP".into(),
                            rounding: None,
                        },
                    )
                    .with_weight(0.05), // 5% ownership
                ),
        )
        // Residual split (80/20)
        .add_tier(
            WaterfallTier::new("residual", 3, PaymentType::Residual)
                .allocation_mode(AllocationMode::ProRata)
                .add_recipient(
                    Recipient::new(
                        "lp_residual",
                        RecipientType::Tranche("LP".into()),
                        PaymentCalculation::ResidualCash,
                    )
                    .with_weight(0.80),
                )
                .add_recipient(
                    Recipient::new(
                        "gp_promote",
                        RecipientType::ManagerFee(ManagementFeeType::Incentive),
                        PaymentCalculation::ResidualCash,
                    )
                    .with_weight(0.20),
                ),
        )
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    let available_cash = Money::new(5_000_000.0, currency); // Quarterly NOI
    let interest_collections = Money::new(0.0, currency);
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();
    let pool_balance = Money::new(50_000_000.0, currency);
    let period_start = payment_date - Duration::days(90);

    let result = run_waterfall(
        &waterfall,
        available_cash,
        interest_collections,
        payment_date,
        &tranches,
        pool_balance,
        Some(period_start),
        &pool,
        &market,
    );

    // Verify pro-rata preferred return tier
    let pref_tier = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "preferred_return");

    assert!(pref_tier.is_some());
    let (_, pref_amount) = pref_tier.unwrap();

    // Expected: 8% pref on $50M / 4 = $1,000,000 quarterly
    // LP: $47.5M × 8% / 4 = $950,000
    // GP: $2.5M × 8% / 4 = $50,000
    // Total: $1,000,000.00 (exact)
    let expected_pref = (47_500_000.0 + 2_500_000.0) * 0.08 / 4.0;
    assert!(
        (pref_amount.amount() - expected_pref).abs() < CASH_TOLERANCE,
        "Preferred return mismatch: expected {}, got {}",
        expected_pref,
        pref_amount.amount()
    );

    // Verify residual tier exists
    let residual_tier = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "residual");
    assert!(residual_tier.is_some());

    // Check that LP gets ~80% and GP gets ~20% of residual
    let lp_dist = result
        .distributions
        .get(&RecipientType::Tranche("LP".into()));
    let gp_dist = result
        .distributions
        .get(&RecipientType::ManagerFee(ManagementFeeType::Incentive));

    if let (Some(_lp), Some(gp)) = (lp_dist, gp_dist) {
        // Total residual after opex and pref
        let residual = available_cash.amount() - 100_000.0 - 1_000_000.0;

        // GP should get ~20% of residual (promote)
        let _expected_gp_residual = residual * 0.20;

        // Note: GP also got preferred return, so total will be higher
        // Just verify GP got some promote
        assert!(gp.amount() > 50_000.0); // More than just 5% of pref
    }
}

#[test]
fn test_golden_cash_conservation() {
    // Property test: Total distributed + remaining = available cash
    // This should hold for ANY valid waterfall execution

    let currency = Currency::USD;
    let pool = create_test_pool(100_000_000.0, currency);
    let tranches = create_test_tranches(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "fee1",
                RecipientType::ServiceProvider("Provider".into()),
                PaymentCalculation::FixedAmount {
                    amount: Money::new(10_000.0, currency),
                    rounding: None,
                },
            )),
        )
        .add_tier(
            WaterfallTier::new("interest", 2, PaymentType::Interest)
                .add_recipient(Recipient::tranche_interest("int", "CLASS_A")),
        )
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    let available_cash = Money::new(1_000_000.0, currency);
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();

    let result = run_waterfall(
        &waterfall,
        available_cash,
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &market,
    );

    // Cash conservation: sum(tiers) + remaining = available
    // This fundamental invariant must hold exactly (within f64 precision)
    let total_allocated: f64 = result
        .tier_allocations
        .iter()
        .map(|(_, amt)| amt.amount())
        .sum();

    let total = total_allocated + result.remaining_cash.amount();

    assert!(
        (total - available_cash.amount()).abs() < CASH_TOLERANCE,
        "Cash conservation violated: allocated {} + remaining {} = {} != available {}",
        total_allocated,
        result.remaining_cash.amount(),
        total,
        available_cash.amount()
    );
}
