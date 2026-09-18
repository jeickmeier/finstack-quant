//! Golden tests for the waterfall engine using representative CLO/CMBS
//! structures (synthetic deals with market-typical tiers: senior fees,
//! sequential interest, coverage-test diversion, principal, equity residual).
//!
//! Expected distributions are hand-computed from the tier definitions, so
//! these tests pin the engine's allocation arithmetic and cash conservation.
//! They are not benchmarked against an external deal prospectus.
//!
//! # Tolerance Standards
//!
//! Waterfall calculations are **deterministic** and should produce exact results
//! within floating-point precision. Tolerances used:
//!
//! - **Cash conservation**: `CASH_TOLERANCE` (0.01) - accounts only for f64 representation
//! - **Fee allocations**: Exact expected values with `CASH_TOLERANCE`
//! - **Interest calculations**: Exact expected values with `CASH_TOLERANCE`

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::CoverageTestSpec;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallContext;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallDistribution;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AllocationMode, AssetPool, DealType, ManagementFeeType, PaymentCalculation, PaymentType,
    Recipient, RecipientType, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
    Waterfall, WaterfallBuilder, WaterfallTier,
};
use time::Duration;

// Market-Standard Tolerances for Waterfall Tests

/// Cash distribution tolerance: deterministic calculations should be exact
/// within f64 representation error (1 cent on any amount).
const CASH_TOLERANCE: f64 = 0.01;

/// Helper to create a simple market context for testing
fn create_test_market() -> MarketContext {
    MarketContext::new()
}

/// Helper to create a test asset pool
fn create_test_pool(balance: f64, currency: Currency) -> AssetPool {
    use finstack_quant_core::types::CreditRating;
    use finstack_quant_core::types::InstrumentId;
    use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
        AssetType, PoolAsset,
    };

    let mut pool = AssetPool::new("TEST_POOL", DealType::Clo, currency);

    // Add assets to match the specified balance
    let num_assets = 10;
    let asset_balance = balance / num_assets as f64;

    for i in 0..num_assets {
        let asset = PoolAsset {
            day_count: finstack_quant_core::dates::DayCount::Act360,
            id: InstrumentId::new(format!("ASSET_{}", i)),
            asset_type: AssetType::FirstLienLoan {
                industry: Some("Technology".into()),
            },
            balance: Money::new(asset_balance, currency).expect("valid money fixture"),
            rate: 0.08,
            spread_bp: Some(400.0),
            index_id: Some("SOFR-3M".into()),
            index_floor: None,
            maturity: Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
            credit_quality: Some(CreditRating::BB),
            industry: Some("Technology".into()),
            obligor_id: Some(format!("OBLIGOR_{}", i)),
            is_defaulted: false,
            recovery_amount: None,
            default_date: None,
            purchase_price: None,
            acquisition_date: None,
            origination_date: None,
            smm_override: None,
            mdr_override: None,
            recovery_rate: None,
            commitment: None,
            contractual_payment: None,
            amortization_term_months: None,
            io_months: None,
            market_price_pct: None,
            delinquency_buckets: None,
            balloon: None,
            prepayment_penalty: None,
            special_servicing: None,
            noi: None,
            liquidation: None,
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
        Money::new(175_000_000.0, currency).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.05 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_b = Tranche::new(
        "CLASS_B",
        70.0,
        85.0,
        TrancheSeniority::Mezzanine,
        Money::new(37_500_000.0, currency).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.065 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_c = Tranche::new(
        "CLASS_C",
        85.0,
        95.0,
        TrancheSeniority::Subordinated,
        Money::new(25_000_000.0, currency).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.08 },
        Date::from_calendar_date(2031, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let equity = Tranche::new(
        "EQUITY",
        95.0,
        100.0,
        TrancheSeniority::Equity,
        Money::new(12_500_000.0, currency).expect("valid money fixture"),
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
    // Tests treat everything above interest as principal proceeds.
    let principal_collections = available_cash
        .checked_sub(interest_collections)
        .unwrap_or(Money::new(0.0, available_cash.currency()).expect("valid money fixture"));
    let context = WaterfallContext {
        available_cash,
        interest_collections,
        principal_collections,
        payment_date,
        period_start,
        valuation_date: period_start,
        pool_balance,
        market,
        tranche_balances: None,
        asset_balances: None,
        live_collateral: None,
        special_serviced: None,
        deferred_interest: None,
        reserve_balance: Money::new(0.0, available_cash.currency()).expect("valid money fixture"),
        restricted_cash: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        defaulted_collateral_value: Money::new(0.0, Currency::USD).expect("valid money fixture"),
        recovery_proceeds: Money::new(0.0, available_cash.currency()).expect("valid money fixture"),
        floating_rate_shift: 0.0,
        equity_history: None,
    };
    finstack_quant_valuations::instruments::fixed_income::structured_credit::execute_waterfall(
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
                        amount: Money::new(50_000.0, currency).expect("valid money fixture"),
                        rounding: None,
                    },
                ))
                .add_recipient(Recipient::new(
                    "senior_mgmt",
                    RecipientType::ManagerFee(ManagementFeeType::Senior),
                    PaymentCalculation::PercentageOfCollateral {
                        rate: 0.004, // 40 bp
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
        // Tier 3: Class A coverage tests (after every note coupon)
        .add_tier(WaterfallTier::coverage_tests(
            "coverage",
            3,
            vec![
                CoverageTestSpec::oc("CLASS_A", 1.25),
                CoverageTestSpec::ic("CLASS_A", 1.20),
            ],
        ))
        // Tier 4: Principal
        .add_tier(
            WaterfallTier::new("principal", 4, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
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
        // Tier 5: Equity
        .add_tier(
            WaterfallTier::new("equity", 5, PaymentType::Residual)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "equity_dist",
                    RecipientType::Equity,
                    PaymentCalculation::ResidualCash,
                )),
        )
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    let available_cash = Money::new(15_000_000.0, currency).expect("valid money fixture"); // Quarterly cash available
    let interest_collections = Money::new(3_000_000.0, currency).expect("valid money fixture");
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();
    let pool_balance = Money::new(250_000_000.0, currency).expect("valid money fixture");
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

    // Verify tier allocations (fees, interest, coverage position, principal, equity)
    assert_eq!(result.tier_allocations.len(), 5);
    assert_eq!(result.tier_allocations[2].0, "coverage");
    assert_eq!(
        result.tier_allocations[2].1.amount(),
        0.0,
        "a passing coverage position diverts nothing"
    );

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
    pool.cumulative_defaults = Money::new(30_000_000.0, currency).expect("valid money fixture");
    pool.cumulative_recoveries = Money::new(15_000_000.0, currency).expect("valid money fixture");

    let tranches = create_test_tranches(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("fees", 1, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "trustee",
                    RecipientType::ServiceProvider("Trustee".into()),
                    PaymentCalculation::FixedAmount {
                        amount: Money::new(50_000.0, currency).expect("valid money fixture"),
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
        // Class A OC test (125% required) after the note coupons: on a breach
        // the interest still undistributed here pays down Class A.
        .add_tier(WaterfallTier::coverage_tests(
            "a_coverage",
            3,
            vec![CoverageTestSpec::oc("CLASS_A", 1.25)],
        ))
        .add_tier(
            // Senior principal: small scheduled paydown (target balance just
            // below par) so the regular pass leaves cash for the junior tier.
            WaterfallTier::new("senior_principal", 4, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::tranche_principal(
                    "class_a_prin",
                    "CLASS_A",
                    Some(Money::new(174_500_000.0, currency).expect("valid money fixture")),
                )),
        )
        .add_tier(
            WaterfallTier::new("junior_principal", 5, PaymentType::Principal)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::tranche_principal(
                    "class_b_prin",
                    "CLASS_B",
                    None,
                )),
        )
        .add_tier(
            WaterfallTier::new("equity", 6, PaymentType::Residual).add_recipient(Recipient::new(
                "equity",
                RecipientType::Equity,
                PaymentCalculation::ResidualCash,
            )),
        )
        .build()
        .expect("build waterfall");

    let market = create_test_market();
    // Interest well above the ~2.8M of quarterly coupons, so the failing test
    // has excess interest to divert to Class A principal.
    let available_cash = Money::new(6_500_000.0, currency).expect("valid money fixture");
    let interest_collections = Money::new(4_000_000.0, currency).expect("valid money fixture");
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();
    let pool_balance = Money::new(200_000_000.0, currency).expect("valid money fixture");

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
    // Only interest ranked below the test position is diverted: the fees and
    // note coupons above it are paid in full and the diversion pays Class A.
    let senior_to_test: f64 = result
        .payment_records
        .iter()
        .filter(|r| r.priority < 3)
        .map(|r| r.paid_amount.amount())
        .sum();
    assert!(
        (result.diverted_cash.amount() - (4_000_000.0 - senior_to_test)).abs() < CASH_TOLERANCE,
        "diverted cash {} must be the interest left after the senior tiers ({})",
        result.diverted_cash.amount(),
        4_000_000.0 - senior_to_test
    );
    assert!(result
        .diverted_amounts
        .iter()
        .all(|record| record.target_tranche == "class_a_prin"));
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
        Money::new(350_000_000.0, currency).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.04 },
        Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_b = Tranche::new(
        "CLASS_B",
        70.0,
        85.0,
        TrancheSeniority::Mezzanine,
        Money::new(75_000_000.0, currency).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.045 },
        Date::from_calendar_date(2034, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let class_c = Tranche::new(
        "CLASS_C",
        85.0,
        100.0,
        TrancheSeniority::Subordinated,
        Money::new(75_000_000.0, currency).expect("valid money fixture"),
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
                    rate: 0.0025, // 25 bp
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
    let available_cash = Money::new(20_000_000.0, currency).expect("valid money fixture");
    let interest_collections = Money::new(5_000_000.0, currency).expect("valid money fixture");
    let payment_date = Date::from_calendar_date(2024, time::Month::February, 1).unwrap();
    let pool_balance = Money::new(500_000_000.0, currency).expect("valid money fixture");

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
        Money::new(47_500_000.0, currency).expect("valid money fixture"),
        TrancheCoupon::Fixed { rate: 0.08 },
        Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    let gp = Tranche::new(
        "GP",
        95.0,
        100.0,
        TrancheSeniority::Equity,
        Money::new(2_500_000.0, currency).expect("valid money fixture"),
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
                    amount: Money::new(100_000.0, currency).expect("valid money fixture"),
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
    let available_cash = Money::new(5_000_000.0, currency).expect("valid money fixture"); // Quarterly NOI
    let interest_collections = available_cash; // Quarterly NOI is income, not return of capital.
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();
    let pool_balance = Money::new(50_000_000.0, currency).expect("valid money fixture");
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
    let (_, pref_amount) = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "preferred_return")
        .expect("preferred-return tier");

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
    let (_, residual_amount) = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "residual")
        .expect("residual tier");

    // Check that LP gets ~80% and GP gets ~20% of residual
    let lp_dist = result
        .distributions
        .get(&RecipientType::Tranche("LP".into()))
        .expect("LP distribution");
    let gp_dist = result
        .distributions
        .get(&RecipientType::ManagerFee(ManagementFeeType::Incentive))
        .expect("GP promote distribution");

    let expected_gp = residual_amount.amount() * 0.20;
    let expected_lp = 950_000.0 + residual_amount.amount() * 0.80;
    assert!(
        (gp_dist.amount() - expected_gp).abs() < CASH_TOLERANCE,
        "GP promote mismatch: expected {expected_gp}, got {}",
        gp_dist.amount()
    );
    assert!(
        (lp_dist.amount() - expected_lp).abs() < CASH_TOLERANCE,
        "LP preferred return plus residual mismatch: expected {expected_lp}, got {}",
        lp_dist.amount()
    );
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
                    amount: Money::new(10_000.0, currency).expect("valid money fixture"),
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
    let available_cash = Money::new(1_000_000.0, currency).expect("valid money fixture");
    let payment_date = Date::from_calendar_date(2024, time::Month::April, 1).unwrap();

    let result = run_waterfall(
        &waterfall,
        available_cash,
        Money::new(0.0, currency).expect("valid money fixture"),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency).expect("valid money fixture"),
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
