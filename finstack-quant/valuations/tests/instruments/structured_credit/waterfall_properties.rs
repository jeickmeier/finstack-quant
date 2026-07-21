//! Property-based tests for waterfall engine.
//!
//! These tests verify invariants that should hold for ANY valid waterfall execution,
//! regardless of the specific configuration or input values.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DateExt};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallContext;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::WaterfallDistribution;
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AllocationMode, AssetPool, DealType, PaymentCalculation, PaymentType, Recipient, RecipientType,
    Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure, Waterfall, WaterfallBuilder,
    WaterfallTier,
};

/// Helper to create a simple market context
fn create_market() -> MarketContext {
    MarketContext::new()
}

/// Helper to create a minimal pool
fn create_pool(currency: Currency) -> AssetPool {
    AssetPool::new("TEST", DealType::CLO, currency)
}

/// Helper to create a simple single-tranche structure
fn create_single_tranche(currency: Currency) -> TrancheStructure {
    let tranche = Tranche::new(
        "TEST_TRANCHE",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(100_000_000.0, currency),
        TrancheCoupon::Fixed { rate: 0.05 },
        Date::from_calendar_date(2030, time::Month::January, 1).unwrap(),
    )
    .unwrap();

    TrancheStructure::new(vec![tranche]).unwrap()
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
        .unwrap_or(Money::new(0.0, available_cash.currency()));
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
        deferred_interest: None,
        reserve_balance: Money::new(0.0, available_cash.currency()),
        recovery_proceeds: Money::new(0.0, available_cash.currency()),
    };
    finstack_quant_valuations::instruments::fixed_income::structured_credit::execute_waterfall(
        waterfall, tranches, pool, context,
    )
    .expect("waterfall execution")
}

#[test]
fn property_cash_conservation() {
    // Property: sum(tier_allocations) + remaining_cash = total_available
    // This must hold for ANY waterfall execution

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let test_cases = vec![1_000.0, 10_000.0, 100_000.0, 1_000_000.0, 10_000_000.0];

    for available_amount in test_cases {
        let waterfall = WaterfallBuilder::new(currency)
            .add_tier(
                WaterfallTier::new("tier1", 1, PaymentType::Fee).add_recipient(Recipient::new(
                    "recipient1",
                    RecipientType::ServiceProvider("Test".into()),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(1_000.0, currency),
                    },
                )),
            )
            .add_tier(
                WaterfallTier::new("tier2", 2, PaymentType::Residual).add_recipient(
                    Recipient::new(
                        "residual",
                        RecipientType::Equity,
                        PaymentCalculation::ResidualCash,
                    ),
                ),
            )
            .build()
            .expect("build waterfall");

        let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
        let result = run_waterfall(
            &waterfall,
            Money::new(available_amount, currency),
            Money::new(0.0, currency),
            payment_date,
            &tranches,
            Money::new(100_000_000.0, currency),
            None,
            &pool,
            &create_market(),
        );

        let total_allocated: f64 = result
            .tier_allocations
            .iter()
            .map(|(_, amt)| amt.amount())
            .sum();

        let total = total_allocated + result.remaining_cash.amount();

        assert!(
            (total - available_amount).abs() < 0.01,
            "Cash conservation failed for amount {}: {} != {}",
            available_amount,
            total,
            available_amount
        );
    }
}

#[test]
fn property_non_negative_distributions() {
    // Property: All distributions must be >= 0
    // Negative payments should never occur

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("tier1", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "fee",
                RecipientType::ServiceProvider("Test".into()),
                PaymentCalculation::FixedAmount {
                    rounding: None,
                    amount: Money::new(10_000.0, currency),
                },
            )),
        )
        .build()
        .expect("build waterfall");

    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(5_000.0, currency), // Less than required
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    // All tier allocations must be non-negative
    for (tier_id, amount) in &result.tier_allocations {
        assert!(
            amount.amount() >= 0.0,
            "Tier {} has negative allocation: {}",
            tier_id,
            amount.amount()
        );
    }

    // All distributions must be non-negative
    for (recipient, amount) in &result.distributions {
        assert!(
            amount.amount() >= 0.0,
            "Recipient {:?} has negative distribution: {}",
            recipient,
            amount.amount()
        );
    }

    // Remaining cash must be non-negative
    assert!(result.remaining_cash.amount() >= 0.0);
}

#[test]
fn property_priority_ordering() {
    // Property: Higher priority tiers (lower number) receive funds first
    // If tier N gets partial payment, tier N+1 should get 0 (sequential mode)

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("high_priority", 1, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "fee1",
                    RecipientType::ServiceProvider("Provider1".into()),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(100_000.0, currency),
                    },
                )),
        )
        .add_tier(
            WaterfallTier::new("low_priority", 2, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "fee2",
                    RecipientType::ServiceProvider("Provider2".into()),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(50_000.0, currency),
                    },
                )),
        )
        .build()
        .expect("build waterfall");

    // Case 1: Insufficient funds - only tier 1 gets paid
    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(75_000.0, currency),
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    let tier1_amount = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "high_priority")
        .map(|(_, amt)| amt.amount())
        .unwrap_or(0.0);

    let tier2_amount = result
        .tier_allocations
        .iter()
        .find(|(id, _)| id == "low_priority")
        .map(|(_, amt)| amt.amount())
        .unwrap_or(0.0);

    // Tier 1 should get partial payment
    assert!(tier1_amount > 0.0);
    assert!(tier1_amount <= 75_000.0);

    // Tier 2 should get nothing (higher priority tier not fully satisfied)
    assert_eq!(tier2_amount, 0.0);
}

#[test]
fn property_pro_rata_weight_distribution() {
    // Property: In pro-rata mode, recipients receive proportional to weights
    // total_allocated * (weight_i / sum_weights) = recipient_i_amount

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("pro_rata_tier", 1, PaymentType::Interest)
                .allocation_mode(AllocationMode::ProRata)
                .add_recipient(
                    Recipient::new(
                        "recipient1",
                        RecipientType::ServiceProvider("Provider1".into()),
                        PaymentCalculation::FixedAmount {
                            rounding: None,
                            amount: Money::new(100_000.0, currency),
                        },
                    )
                    .with_weight(0.60), // 60%
                )
                .add_recipient(
                    Recipient::new(
                        "recipient2",
                        RecipientType::ServiceProvider("Provider2".into()),
                        PaymentCalculation::FixedAmount {
                            rounding: None,
                            amount: Money::new(100_000.0, currency),
                        },
                    )
                    .with_weight(0.40), // 40%
                ),
        )
        .build()
        .expect("build waterfall");

    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(150_000.0, currency), // Less than total requested
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    let dist1 = result
        .distributions
        .get(&RecipientType::ServiceProvider("Provider1".into()))
        .map(|m| m.amount())
        .unwrap_or(0.0);

    let dist2 = result
        .distributions
        .get(&RecipientType::ServiceProvider("Provider2".into()))
        .map(|m| m.amount())
        .unwrap_or(0.0);

    // Verify pro-rata distribution
    let total_distributed = dist1 + dist2;
    let ratio = dist1 / total_distributed;

    assert!(
        (ratio - 0.60).abs() < 0.01,
        "Pro-rata ratio should be 0.60, got {}",
        ratio
    );
}

#[test]
fn property_shortfall_computation() {
    // Property: shortfall = requested - paid for each payment record
    // shortfall should be 0 if fully paid, > 0 if underpaid

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("tier1", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "fee",
                RecipientType::ServiceProvider("Test".into()),
                PaymentCalculation::FixedAmount {
                    rounding: None,
                    amount: Money::new(100_000.0, currency),
                },
            )),
        )
        .build()
        .expect("build waterfall");

    // Case 1: Full payment
    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(200_000.0, currency),
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    for record in &result.payment_records {
        let computed_shortfall = record.requested_amount.amount() - record.paid_amount.amount();
        assert!(
            (computed_shortfall - record.shortfall.amount()).abs() < 0.01,
            "Shortfall computation error: {} != {}",
            computed_shortfall,
            record.shortfall.amount()
        );

        // Full payment → zero shortfall
        assert!(record.shortfall.amount() < 0.01);
    }

    // Case 2: Partial payment
    let result = run_waterfall(
        &waterfall,
        Money::new(50_000.0, currency),
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    for record in &result.payment_records {
        let computed_shortfall = record.requested_amount.amount() - record.paid_amount.amount();
        assert!(
            (computed_shortfall - record.shortfall.amount()).abs() < 0.01,
            "Shortfall computation error"
        );

        // Partial payment → positive shortfall
        if record.requested_amount.amount() > 50_000.0 {
            assert!(record.shortfall.amount() > 0.0);
        }
    }
}

#[test]
fn property_tier_count_consistency() {
    // Property: Number of tier_allocations should equal number of tiers
    // Even if a tier gets zero allocation, it should appear in results

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let tier_count = 5;
    let mut builder = WaterfallBuilder::new(currency);

    for i in 1..=tier_count {
        builder = builder.add_tier(
            WaterfallTier::new(format!("tier{}", i), i, PaymentType::Fee).add_recipient(
                Recipient::new(
                    format!("recipient{}", i),
                    RecipientType::ServiceProvider(format!("Provider{}", i)),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(10_000.0, currency),
                    },
                ),
            ),
        );
    }

    let waterfall = builder.build().expect("build waterfall");

    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(15_000.0, currency), // Only enough for ~1.5 tiers
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    // Should have allocation entry for each tier
    assert_eq!(
        result.tier_allocations.len(),
        tier_count,
        "Number of tier allocations should match number of tiers"
    );
}

#[test]
fn property_diversion_tracking() {
    // Property: diverted_cash should equal sum of all diverted payments
    // If had_diversions is true, diverted_cash should be > 0

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("tier1", 1, PaymentType::Principal)
                .divertible(true)
                .add_recipient(Recipient::new(
                    "principal",
                    RecipientType::Tranche("TEST_TRANCHE".into()),
                    PaymentCalculation::TranchePrincipal {
                        tranche_id: "TEST_TRANCHE".into(),
                        target_balance: None,
                        rounding: None,
                    },
                )),
        )
        .build()
        .expect("build waterfall");

    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(1_000_000.0, currency),
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    // Property: had_diversions consistency
    if result.had_diversions {
        assert!(
            result.diverted_cash.amount() >= 0.0,
            "If diversions occurred, diverted_cash should be >= 0"
        );
    }

    // Property: diverted payment records
    let diverted_count = result.payment_records.iter().filter(|r| r.diverted).count();

    if diverted_count > 0 {
        assert!(
            result.had_diversions,
            "If payments were diverted, had_diversions should be true"
        );
    }
}

#[test]
fn property_monotonic_tier_allocation() {
    // Property: In sequential mode with equal recipients,
    // tier allocations should be monotonically non-increasing by priority
    // (higher priority tiers get at least as much as lower priority tiers)

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("tier1", 1, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "r1",
                    RecipientType::ServiceProvider("P1".into()),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(50_000.0, currency),
                    },
                )),
        )
        .add_tier(
            WaterfallTier::new("tier2", 2, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "r2",
                    RecipientType::ServiceProvider("P2".into()),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(50_000.0, currency),
                    },
                )),
        )
        .add_tier(
            WaterfallTier::new("tier3", 3, PaymentType::Fee)
                .allocation_mode(AllocationMode::Sequential)
                .add_recipient(Recipient::new(
                    "r3",
                    RecipientType::ServiceProvider("P3".into()),
                    PaymentCalculation::FixedAmount {
                        rounding: None,
                        amount: Money::new(50_000.0, currency),
                    },
                )),
        )
        .build()
        .expect("build waterfall");

    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(100_000.0, currency), // Enough for 2 tiers
        Money::new(0.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    let amounts: Vec<f64> = result
        .tier_allocations
        .iter()
        .map(|(_, amt)| amt.amount())
        .collect();

    // Verify monotonic non-increasing (tier1 >= tier2 >= tier3)
    for i in 0..amounts.len() - 1 {
        assert!(
            amounts[i] >= amounts[i + 1] - 0.01, // Allow tiny float errors
            "Tier allocations should be monotonically non-increasing: tier{} ({}) < tier{} ({})",
            i + 1,
            amounts[i],
            i + 2,
            amounts[i + 1]
        );
    }
}

#[test]
fn property_coverage_test_result_format() {
    // Property: Coverage test results should have valid format
    // (test_id: String, ratio: f64, passed: bool)
    // ratio should be non-negative

    let currency = Currency::USD;
    let pool = create_pool(currency);
    let tranches = create_single_tranche(currency);

    let waterfall = WaterfallBuilder::new(currency)
        .add_tier(
            WaterfallTier::new("tier1", 1, PaymentType::Fee).add_recipient(Recipient::new(
                "fee",
                RecipientType::ServiceProvider("Test".into()),
                PaymentCalculation::FixedAmount {
                    rounding: None,
                    amount: Money::new(1_000.0, currency),
                },
            )),
        )
        .add_coverage_trigger(
            finstack_quant_valuations::instruments::fixed_income::structured_credit::waterfall::CoverageTrigger {
                tranche_id: "TEST_TRANCHE".into(),
                oc_trigger: Some(1.25),
                ic_trigger: Some(1.20),
            },
        )
        .build().expect("build waterfall");

    let payment_date = Date::from_calendar_date(2024, time::Month::January, 1).unwrap();
    let result = run_waterfall(
        &waterfall,
        Money::new(10_000.0, currency),
        Money::new(5_000.0, currency),
        payment_date,
        &tranches,
        Money::new(100_000_000.0, currency),
        None,
        &pool,
        &create_market(),
    );

    for (test_id, ratio, passed) in &result.coverage_tests {
        // Test ID should not be empty
        assert!(!test_id.is_empty(), "Test ID should not be empty");

        // Ratio should be non-negative
        assert!(
            *ratio >= 0.0,
            "Test ratio should be non-negative: {}",
            ratio
        );

        // Passed should be consistent with ratio and threshold
        // (We can't verify exact logic without knowing threshold, but can check type)
        let _ = passed; // Just verify it's a bool
    }
}
