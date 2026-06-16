//! Targeted regression tests for deterministic simulation edge cases.
//!
//! Covers:
//! - m-FINAL-1: Mid-period maturity interest cap
//! - is_defaulted skip: Pre-defaulted assets excluded from pool flows
//! - Reinvestment reconciliation: pool_outstanding snaps to actual balances

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::{Date, DayCount};
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::math::interp::InterpStyle;
use finstack_quant_core::money::Money;
use finstack_quant_core::types::{CreditRating, InstrumentId};
use finstack_quant_valuations::instruments::fixed_income::structured_credit::{
    AssetPool, AssetType, DealType, PoolAsset, ReinvestmentCriteria, ReinvestmentPeriod,
    StructuredCredit, Tranche, TrancheCoupon, TrancheSeniority, TrancheStructure,
};
use time::Month;

// ============================================================================
// Test Helpers
// ============================================================================

fn test_date() -> Date {
    Date::from_calendar_date(2025, Month::January, 1).unwrap()
}

fn closing_date() -> Date {
    Date::from_calendar_date(2024, Month::October, 1).unwrap()
}

fn legal_maturity() -> Date {
    Date::from_calendar_date(2030, Month::December, 31).unwrap()
}

/// Create a single fixed-rate bullet asset with configurable maturity.
fn make_asset(id: &str, balance: f64, rate: f64, maturity: Date, is_defaulted: bool) -> PoolAsset {
    PoolAsset {
        id: InstrumentId::new(id.to_string()),
        asset_type: AssetType::FirstLienLoan {
            industry: Some("Test".to_string()),
        },
        balance: Money::new(balance, Currency::USD),
        rate,
        spread_bps: None,
        index_id: None,
        maturity,
        credit_quality: Some(CreditRating::BB),
        industry: Some("Test".to_string()),
        obligor_id: Some(format!("OB_{id}")),
        is_defaulted,
        recovery_amount: None,
        purchase_price: None,
        acquisition_date: Some(closing_date()),
        day_count: DayCount::Act360,
        smm_override: None,
        mdr_override: None,
    }
}

/// Single-tranche structure. High coupon ensures tranche wants more than pool
/// can deliver, so tranche interest_paid is constrained by pool collections.
fn single_tranche_high_coupon(balance: f64) -> TrancheStructure {
    let tranche = Tranche::new(
        "SENIOR_A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(balance, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.20 }, // 20% -- binds against pool
        legal_maturity(),
    )
    .expect("valid tranche");

    TrancheStructure::new(vec![tranche]).expect("valid structure")
}

/// Minimal tranche for deals where we only care about pool behavior.
fn single_tranche(balance: f64) -> TrancheStructure {
    let tranche = Tranche::new(
        "SENIOR_A",
        0.0,
        100.0,
        TrancheSeniority::Senior,
        Money::new(balance, Currency::USD),
        TrancheCoupon::Fixed { rate: 0.05 },
        legal_maturity(),
    )
    .expect("valid tranche");

    TrancheStructure::new(vec![tranche]).expect("valid structure")
}

/// Flat market context with no forward curves (fixed-rate assets only).
fn flat_market() -> MarketContext {
    let discount = DiscountCurve::builder("USD_OIS")
        .base_date(test_date())
        .knots(vec![(0.0, 1.0), (1.0, 0.95), (5.0, 0.78)])
        .interp(InterpStyle::Linear)
        .build()
        .expect("valid discount curve");

    MarketContext::new().insert(discount)
}

/// Run simulation with a shared market context for deterministic comparisons.
fn run_sim(
    instrument: &StructuredCredit,
    market: &MarketContext,
) -> finstack_quant_core::HashMap<
    String,
    finstack_quant_valuations::instruments::fixed_income::structured_credit::TrancheCashflows,
> {
    finstack_quant_valuations::instruments::fixed_income::structured_credit::run_simulation(
        instrument,
        market,
        test_date(),
    )
    .expect("simulation should succeed")
}

// ============================================================================
// Test 1: Mid-period maturity interest cap (m-FINAL-1)
// ============================================================================

#[test]
fn test_mid_period_maturity_caps_interest_accrual() {
    // Arrange: Two deals each with a single asset.
    // - "far": asset matures in 2028 → many periods of full interest
    // - "mid": asset matures mid-Q1-2025 → first period interest is capped,
    //   asset balance returned as balloon principal, deal ends quickly.
    //
    // With a 20% tranche coupon, the tranche always wants more interest
    // than the pool generates (pool rate = 6%). So tranche interest_paid
    // is constrained by available cash from the pool, making pool-level
    // differences visible in tranche results.
    let balance = 10_000_000.0;
    let rate = 0.06; // 6% annual

    let far_maturity = Date::from_calendar_date(2028, Month::January, 1).unwrap();
    let mid_maturity = Date::from_calendar_date(2025, Month::February, 15).unwrap();

    let mut pool_far = AssetPool::new("POOL_FAR", DealType::CLO, Currency::USD);
    pool_far
        .assets
        .push(make_asset("A_FAR", balance, rate, far_maturity, false));

    let mut pool_mid = AssetPool::new("POOL_MID", DealType::CLO, Currency::USD);
    pool_mid
        .assets
        .push(make_asset("A_MID", balance, rate, mid_maturity, false));

    let clo_far = StructuredCredit::new_clo(
        "CLO_FAR",
        pool_far,
        single_tranche_high_coupon(balance),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let clo_mid = StructuredCredit::new_clo(
        "CLO_MID",
        pool_mid,
        single_tranche_high_coupon(balance),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    // Act -- shared market for deterministic comparison
    let market = flat_market();
    let results_far = run_sim(&clo_far, &market);
    let results_mid = run_sim(&clo_mid, &market);

    let tranche_far = &results_far["SENIOR_A"];
    let tranche_mid = &results_mid["SENIOR_A"];

    // Assert 1: The mid-maturity deal should have fewer interest payment periods
    // because the pool is exhausted after the asset's balloon payment.
    assert!(
        tranche_far.interest_flows.len() > tranche_mid.interest_flows.len(),
        "Far-maturity deal should have more interest periods ({}) than mid-maturity ({})",
        tranche_far.interest_flows.len(),
        tranche_mid.interest_flows.len(),
    );

    // Assert 2: Total interest from the far-maturity deal should exceed
    // total interest from the mid-maturity deal (more periods of interest).
    assert!(
        tranche_far.total_interest.amount() > tranche_mid.total_interest.amount(),
        "Far-maturity total interest ({:.2}) should exceed mid-maturity ({:.2})",
        tranche_far.total_interest.amount(),
        tranche_mid.total_interest.amount(),
    );

    // Assert 3: The mid-maturity deal should return principal quickly
    // (balloon payment in the first period), so total_principal should
    // be close to the full balance.
    assert!(
        tranche_mid.total_principal.amount() > balance * 0.90,
        "Mid-maturity deal should return most principal ({:.2}) from balloon",
        tranche_mid.total_principal.amount(),
    );
}

// ============================================================================
// Test 2: Pre-defaulted asset is skipped in pool flows
// ============================================================================

#[test]
fn test_pre_defaulted_asset_generates_zero_pool_interest() {
    // Arrange: A deal with a single pre-defaulted asset (non-zero balance).
    // Since the asset is defaulted, it should generate zero pool interest.
    // With a high tranche coupon, the tranche will record zero interest_paid
    // because there are zero pool collections.
    let balance = 5_000_000.0;
    let rate = 0.08;
    let maturity = legal_maturity();

    let mut pool = AssetPool::new("POOL_DEFAULT", DealType::CLO, Currency::USD);
    pool.assets
        .push(make_asset("DEFAULTED", balance, rate, maturity, true));

    let clo = StructuredCredit::new_clo(
        "CLO_DEFAULT",
        pool,
        single_tranche_high_coupon(balance),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    // Act
    let market = flat_market();
    let results = run_sim(&clo, &market);
    let tranche = &results["SENIOR_A"];

    // Assert: No interest should be collected from a fully-defaulted pool.
    // The tranche should receive zero interest payments.
    assert_eq!(
        tranche.total_interest.amount(),
        0.0,
        "Pre-defaulted asset should generate zero tranche interest, got {:.2}",
        tranche.total_interest.amount(),
    );

    // No cashflows at all (no interest, no principal from defaulted assets)
    assert!(
        tranche.cashflows.is_empty(),
        "No cashflows should be generated from a fully-defaulted pool, got {} flows",
        tranche.cashflows.len(),
    );
}

#[test]
fn test_pre_defaulted_asset_does_not_affect_performing_pool_flows() {
    // Arrange: Compare two deals:
    // - perf_only: one performing asset
    // - mixed: same performing asset + one pre-defaulted asset
    //
    // Both deals use a high tranche coupon so pool collections constrain
    // interest_paid. Since the defaulted asset contributes nothing, both
    // deals should generate identical total interest over all periods
    // (assuming the same performing asset drives both).
    let balance = 5_000_000.0;
    let rate = 0.08;
    let maturity = legal_maturity();

    let mut pool_perf = AssetPool::new("POOL_PERF", DealType::CLO, Currency::USD);
    pool_perf
        .assets
        .push(make_asset("PERFORMING", balance, rate, maturity, false));

    let mut pool_mixed = AssetPool::new("POOL_MIXED", DealType::CLO, Currency::USD);
    pool_mixed
        .assets
        .push(make_asset("PERFORMING", balance, rate, maturity, false));
    pool_mixed
        .assets
        .push(make_asset("DEFAULTED", balance, rate, maturity, true));

    // Both tranches sized to the performing balance so interest dynamics match
    let clo_perf = StructuredCredit::new_clo(
        "CLO_PERF",
        pool_perf,
        single_tranche_high_coupon(balance),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let clo_mixed = StructuredCredit::new_clo(
        "CLO_MIXED",
        pool_mixed,
        single_tranche_high_coupon(balance),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    // Act -- shared market for deterministic comparison
    let market = flat_market();
    let results_perf = run_sim(&clo_perf, &market);
    let results_mixed = run_sim(&clo_mixed, &market);

    let tranche_perf = &results_perf["SENIOR_A"];
    let tranche_mixed = &results_mixed["SENIOR_A"];

    // Assert: Total interest should be identical (both driven by same performing
    // asset). Allow $1 tolerance for floating-point rounding across periods.
    let diff = (tranche_perf.total_interest.amount() - tranche_mixed.total_interest.amount()).abs();
    assert!(
        diff < 1.0,
        "Total interest should match: perf={:.2}, mixed={:.2}, diff={:.2}",
        tranche_perf.total_interest.amount(),
        tranche_mixed.total_interest.amount(),
        diff,
    );

    // Assert: Same number of interest payment periods
    assert_eq!(
        tranche_perf.interest_flows.len(),
        tranche_mixed.interest_flows.len(),
        "Should have same number of interest periods: perf={}, mixed={}",
        tranche_perf.interest_flows.len(),
        tranche_mixed.interest_flows.len(),
    );
}

// ============================================================================
// Test 3: Reinvestment reconciliation snaps pool_outstanding
// ============================================================================

#[test]
fn test_reinvestment_end_reconciles_pool_outstanding() {
    // Arrange: CLO with a short reinvestment period (ends 2025-06-30).
    // Simulation starts at 2025-01-01. After reinvestment ends, the waterfall
    // should see accurate pool balances (no phantom balance from drift).
    let balance = 10_000_000.0;
    let rate = 0.06;
    let maturity = legal_maturity();

    let mut pool = AssetPool::new("POOL_REINVEST", DealType::CLO, Currency::USD);
    pool.assets
        .push(make_asset("LOAN_1", balance, rate, maturity, false));
    pool.assets
        .push(make_asset("LOAN_2", balance, rate, maturity, false));

    // Set reinvestment period ending mid-2025
    let reinvest_end = Date::from_calendar_date(2025, Month::June, 30).unwrap();
    pool.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: reinvest_end,
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
    });

    let clo = StructuredCredit::new_clo(
        "CLO_REINVEST",
        pool,
        single_tranche(balance * 2.0),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    let market = flat_market();

    // Act
    let results = run_sim(&clo, &market);

    // Assert: Simulation should complete without panics or errors.
    // Total interest + principal should be positive (deal produced cashflows).
    let tranche = &results["SENIOR_A"];
    let total_cf: f64 = tranche
        .cashflows
        .iter()
        .map(|(_, m)| m.amount())
        .sum::<f64>();

    assert!(
        total_cf > 0.0,
        "Deal with reinvestment should generate positive cashflows"
    );

    // Verify that cashflows span both the reinvestment and post-reinvestment periods.
    let has_during_reinvest = tranche.cashflows.iter().any(|(d, _)| *d <= reinvest_end);
    let has_after_reinvest = tranche.cashflows.iter().any(|(d, _)| *d > reinvest_end);

    assert!(
        has_during_reinvest,
        "Should have cashflows during reinvestment period"
    );
    assert!(
        has_after_reinvest,
        "Should have cashflows after reinvestment ends"
    );

    // The final balance should be non-negative (no phantom negative balance
    // from double-counting pool reductions).
    assert!(
        tranche.final_balance.amount() >= 0.0,
        "Final tranche balance should be non-negative, got: {:.2}",
        tranche.final_balance.amount(),
    );

    // Total principal paid should not exceed original tranche balance.
    // If pool_outstanding were double-counted (B1 bug), the waterfall could
    // see an inflated pool balance and over-pay principal.
    let original_balance = balance * 2.0;
    assert!(
        tranche.total_principal.amount() <= original_balance + 1.0, // $1 tolerance
        "Total principal ({:.2}) should not exceed original tranche balance ({:.2})",
        tranche.total_principal.amount(),
        original_balance,
    );
}

#[test]
fn test_reinvestment_vs_no_reinvestment_produces_consistent_results() {
    // Arrange: Compare a deal WITH reinvestment period vs. an identical deal
    // WITHOUT. The reinvestment deal recycles principal during the reinvestment
    // window, so it should have more total interest (pool stays larger longer)
    // but the same or less total principal returned early.
    //
    // This baseline comparison catches reconciliation bugs: if pool_outstanding
    // diverges from reality at the reinvestment-end transition, the
    // reinvestment deal would produce anomalous cashflows relative to the
    // no-reinvestment baseline.
    let balance = 10_000_000.0;
    let rate = 0.06;
    let maturity = legal_maturity();

    // Deal WITH reinvestment
    let mut pool_reinvest = AssetPool::new("POOL_REINVEST", DealType::CLO, Currency::USD);
    pool_reinvest
        .assets
        .push(make_asset("LOAN_1", balance, rate, maturity, false));
    pool_reinvest
        .assets
        .push(make_asset("LOAN_2", balance, rate, maturity, false));

    let reinvest_end = Date::from_calendar_date(2026, Month::January, 1).unwrap();
    pool_reinvest.reinvestment_period = Some(ReinvestmentPeriod {
        end_date: reinvest_end,
        is_active: true,
        criteria: ReinvestmentCriteria::default(),
    });

    let clo_reinvest = StructuredCredit::new_clo(
        "CLO_REINVEST",
        pool_reinvest,
        single_tranche(balance * 2.0),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    // Deal WITHOUT reinvestment (same pool, no reinvestment period)
    let mut pool_no_reinvest = AssetPool::new("POOL_NO_REINVEST", DealType::CLO, Currency::USD);
    pool_no_reinvest
        .assets
        .push(make_asset("LOAN_1", balance, rate, maturity, false));
    pool_no_reinvest
        .assets
        .push(make_asset("LOAN_2", balance, rate, maturity, false));
    // No reinvestment_period set

    let clo_no_reinvest = StructuredCredit::new_clo(
        "CLO_NO_REINVEST",
        pool_no_reinvest,
        single_tranche(balance * 2.0),
        closing_date(),
        legal_maturity(),
        "USD_OIS",
    )
    .with_payment_calendar("nyse");

    // Act -- shared market context
    let market = flat_market();
    let results_reinvest = run_sim(&clo_reinvest, &market);
    let results_no_reinvest = run_sim(&clo_no_reinvest, &market);

    let tranche_reinvest = &results_reinvest["SENIOR_A"];
    let tranche_no_reinvest = &results_no_reinvest["SENIOR_A"];

    // Assert 1: Both deals should generate positive cashflows
    assert!(
        tranche_reinvest.total_interest.amount() > 0.0,
        "Reinvestment deal should generate interest"
    );
    assert!(
        tranche_no_reinvest.total_interest.amount() > 0.0,
        "No-reinvestment deal should generate interest"
    );

    // Assert 2: Reinvestment deal should generate MORE total interest than
    // no-reinvestment, because during the reinvestment period principal is
    // recycled keeping the pool larger and generating more interest.
    assert!(
        tranche_reinvest.total_interest.amount() >= tranche_no_reinvest.total_interest.amount(),
        "Reinvestment deal interest ({:.2}) should be >= no-reinvestment ({:.2})",
        tranche_reinvest.total_interest.amount(),
        tranche_no_reinvest.total_interest.amount(),
    );

    // Assert 3: Both deals should have non-negative final balances
    assert!(
        tranche_reinvest.final_balance.amount() >= -1e-9,
        "Reinvestment deal final balance should be non-negative: {:.2}",
        tranche_reinvest.final_balance.amount(),
    );
    assert!(
        tranche_no_reinvest.final_balance.amount() >= -1e-9,
        "No-reinvestment deal final balance should be non-negative: {:.2}",
        tranche_no_reinvest.final_balance.amount(),
    );

    // Assert 4: Total cashflows (interest + principal) should be in the same
    // ballpark -- wildly different totals would indicate a reconciliation bug.
    // The reinvestment deal may have slightly more total cashflows (more
    // interest from larger pool), but not orders of magnitude different.
    let total_cf_reinvest: f64 = tranche_reinvest
        .cashflows
        .iter()
        .map(|(_, m)| m.amount())
        .sum();
    let total_cf_no_reinvest: f64 = tranche_no_reinvest
        .cashflows
        .iter()
        .map(|(_, m)| m.amount())
        .sum();

    let ratio = if total_cf_no_reinvest > 0.0 {
        total_cf_reinvest / total_cf_no_reinvest
    } else {
        1.0
    };

    assert!(
        (0.5..2.0).contains(&ratio),
        "Total cashflow ratio should be reasonable: reinvest={:.2}, no_reinvest={:.2}, ratio={:.4}",
        total_cf_reinvest,
        total_cf_no_reinvest,
        ratio,
    );
}
